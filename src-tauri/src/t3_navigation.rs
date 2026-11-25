#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    },
};

#[cfg(windows)]
unsafe extern "system" fn collect_title(window: HWND, data: LPARAM) -> i32 {
    if IsWindowVisible(window) == 0 || GetWindowTextLengthW(window) == 0 {
        return 1;
    }
    let mut pid = 0;
    GetWindowThreadProcessId(window, &mut pid);
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return 1;
    }
    let mut path = [0u16; 1024];
    let mut length = path.len() as u32;
    let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
    CloseHandle(process);
    if ok == 0 {
        return 1;
    }
    let path = String::from_utf16_lossy(&path[..length as usize]).to_ascii_lowercase();
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|v| v.to_str());
    if matches!(name, Some("t3 code.exe" | "t3 code (nightly).exe")) {
        let mut title = vec![0u16; GetWindowTextLengthW(window) as usize + 1];
        let n = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
        if n > 0 {
            // EnumWindows calls synchronously with the titles vector from t3_window_titles.
            (*(data as *mut Vec<String>)).push(String::from_utf16_lossy(&title[..n as usize]));
        }
    }
    1
}

#[tauri::command]
pub fn t3_window_titles() -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        let mut titles = Vec::new();
        unsafe {
            EnumWindows(
                Some(collect_title),
                &mut titles as *mut Vec<String> as LPARAM,
            );
        }
        Ok(titles)
    }
    #[cfg(not(windows))]
    Err("Active thread sync requires Windows.".into())
}

#[tauri::command]
pub fn t3_open_thread(environment_id: String, thread_id: String) -> Result<(), String> {
    let mut url = reqwest::Url::parse("t3code://threads").unwrap();
    for value in [&environment_id, &thread_id] {
        if value.is_empty() || value == "." || value == ".." || value.contains(['/', '\\']) {
            return Err("Invalid T3 thread link.".into());
        }
        url.path_segments_mut().unwrap().push(value);
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let url: Vec<u16> = url.as_str().encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize > 32 {
            Ok(())
        } else {
            Err(format!(
                "Windows could not open the T3 Code thread link ({}).",
                result as isize
            ))
        }
    }
    #[cfg(not(windows))]
    Err("Active thread sync requires Windows.".into())
}
