#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod git;
mod devops;
mod preferences;
mod shell;
mod t3_images;
mod t3_navigation;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock, Weak},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tauri::{Manager, State};

#[derive(Default)]
struct AppState {
    git_locks: Arc<Mutex<HashMap<PathBuf, Weak<RwLock<()>>>>>,
    diff_requests: Mutex<HashMap<String, Arc<AtomicU64>>>,
    t3: Mutex<Option<(String, String)>>,
}

#[tauri::command]
async fn devops_review_vote(repo: String, repository_id: String, id: u64, vote: Option<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || devops::review_vote(&repo, &repository_id, id, vote.as_deref()))
        .await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn devops_pr_diff(repo: String, base: String, head: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || devops::pull_request_diff(&repo, &base, &head))
        .await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn devops_pull_requests(repo: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || devops::pull_requests(&repo))
        .await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn devops_sign_in() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || devops::sign_in())
        .await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn devops_tickets(repo: String, query: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || devops::tickets(&repo, &query))
        .await.map_err(|error| error.to_string())?
}

#[tauri::command]
async fn git_request(request: git::Request, state: State<'_, AppState>) -> Result<Value, String> {
    let locks = state.git_locks.clone();
    let latest = if request.action == "diff" && !request.client_id.is_empty() {
        let mut requests = state.diff_requests.lock().map_err(|error| error.to_string())?;
        let latest = requests.entry(request.client_id.clone()).or_default().clone();
        latest.fetch_max(request.request_id, Ordering::SeqCst);
        Some(latest)
    } else { None };
    let request_id = request.request_id;
    tauri::async_runtime::spawn_blocking(move || {
        let check = || if latest.as_ref().is_some_and(|latest| latest.load(Ordering::SeqCst) != request_id) {
            Err("Diff request superseded".to_string())
        } else { Ok(()) };
        check()?;
        let lock = repository_lock(&locks, &request.repo)?;
        let action = request.action.clone();
        with_git_access(&lock, &action, || git::execute_checked(request, check))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn repository_lock(locks: &Mutex<HashMap<PathBuf, Weak<RwLock<()>>>>, repo: &str) -> Result<Arc<RwLock<()>>, String> {
    let path = std::fs::canonicalize(repo).map_err(|error| error.to_string())?;
    let mut common = None;
    for root in path.ancestors() {
        let marker = root.join(".git");
        let gitdir = if marker.is_dir() { marker }
        else if marker.is_file() {
            let contents = std::fs::read_to_string(&marker).map_err(|error| error.to_string())?;
            root.join(contents.trim().strip_prefix("gitdir: ").ok_or("Invalid Git directory marker")?)
        } else { continue; };
        let shared = gitdir.join("commondir");
        let directory = if shared.exists() {
            gitdir.join(std::fs::read_to_string(shared).map_err(|error| error.to_string())?.trim())
        } else { gitdir };
        common = Some(std::fs::canonicalize(directory).map_err(|error| error.to_string())?);
        break;
    }
    let key = common.ok_or("Choose a Git worktree")?;
    #[cfg(windows)]
    let key = PathBuf::from(key.to_string_lossy().to_lowercase());
    let mut locks = locks.lock().map_err(|error| error.to_string())?;
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) { return Ok(lock); }
    let lock = Arc::new(RwLock::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    Ok(lock)
}

fn with_git_access<T>(lock: &RwLock<()>, action: &str, run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    if git::is_read_only(action) {
        let _guard = lock.read().map_err(|e| e.to_string())?;
        run()
    } else {
        let _guard = lock.write().map_err(|e| e.to_string())?;
        run()
    }
}

#[cfg(test)]
mod access_tests {
    use super::*;
    #[test]
    fn independent_repositories_do_not_block_but_linked_worktrees_share_access() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let linked = tempfile::tempdir().unwrap();
        std::fs::create_dir(one.path().join(".git")).unwrap();
        std::fs::create_dir(two.path().join(".git")).unwrap();
        let metadata = one.path().join(".git/worktrees/linked");
        std::fs::create_dir_all(&metadata).unwrap();
        std::fs::write(metadata.join("commondir"), "../..").unwrap();
        std::fs::write(linked.path().join(".git"), format!("gitdir: {}", metadata.display())).unwrap();
        let locks = Mutex::new(HashMap::new());
        let first = repository_lock(&locks, one.path().to_str().unwrap()).unwrap();
        let same = repository_lock(&locks, linked.path().to_str().unwrap()).unwrap();
        let other = repository_lock(&locks, two.path().to_str().unwrap()).unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        let _reading = first.read().unwrap();
        assert!(same.try_write().is_err());
        assert!(other.try_write().is_ok());
    }
    #[test]
    fn diffs_do_not_queue_behind_reads_but_mutations_stay_exclusive() {
        let lock = Arc::new(RwLock::new(()));
        let background = lock.read().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let other = lock.clone();
        let reader = std::thread::spawn(move || with_git_access(&other, "diff", || {
            sender.send(()).unwrap(); Ok(())
        }));
        let concurrent = receiver.recv_timeout(Duration::from_secs(1));
        drop(background);
        reader.join().unwrap().unwrap();
        assert!(concurrent.is_ok(), "File diff waited behind a background read");

        let background = lock.read().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let other = lock.clone();
        let writer = std::thread::spawn(move || with_git_access(&other, "stage", || {
            sender.send(()).unwrap(); Ok(())
        }));
        let premature = receiver.recv_timeout(Duration::from_millis(50));
        drop(background);
        assert!(premature.is_err(), "Mutation overlapped a read");
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        writer.join().unwrap().unwrap();
        assert!(!git::is_read_only("unknown-action"));
    }
}

fn local_origin(value: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Enter a valid local T3 address.")?;
    if !["http", "https"].contains(&url.scheme())
        || !["localhost", "127.0.0.1", "[::1]"].contains(&url.host_str().unwrap_or(""))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only a T3 server on this machine is supported.".into());
    }
    Ok(url.origin().ascii_serialization())
}
fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}
async fn response_json(response: reqwest::Response) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(format!(
            "T3 returned HTTP {}. Check the pairing code, server version, and granted permissions.",
            response.status()
        ));
    }
    response
        .json()
        .await
        .map_err(|_| "T3 returned an unexpected response.".into())
}

#[tauri::command]
fn t3_discover() -> Option<String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let value: Value = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(&home).join(".t3/userdata/server-runtime.json"))
            .ok()?,
    )
    .ok()?;
    let port = value.get("port")?.as_u64()?;
    if port == 0 || port > 65535 {
        return None;
    }
    Some(format!("http://127.0.0.1:{port}"))
}

#[tauri::command]
async fn t3_pair(
    origin: String,
    credential: String,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let origin = local_origin(&origin)?;
    let body = [
        (
            "grant_type",
            "urn:ietf:params:oauth:grant-type:token-exchange",
        ),
        ("subject_token", credential.as_str()),
        (
            "subject_token_type",
            "urn:t3:params:oauth:token-type:environment-bootstrap",
        ),
        (
            "requested_token_type",
            "urn:ietf:params:oauth:token-type:access_token",
        ),
        ("scope", "orchestration:read orchestration:operate"),
        ("client_label", "GitGoblin"),
        ("client_device_type", "desktop"),
    ];
    let result = response_json(
        http()?
            .post(format!("{origin}/oauth/token"))
            .form(&body)
            .send()
            .await
            .map_err(|_| "Could not reach the local T3 server.")?,
    )
    .await?;
    let token = result
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("T3 did not return an access token.")?
        .to_string();
    let mut session = state.t3.lock().map_err(|e| e.to_string())?;
    preferences::save_pairing(&(origin.clone(), token.clone()))?;
    *session = Some((origin, token));
    Ok(json!({"connected":true,"expiresIn":result.get("expires_in"),"scope":result.get("scope")}))
}

#[tauri::command]
async fn t3_ticket(state: State<'_, AppState>) -> Result<String, String> {
    let (origin, token) = state
        .t3
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("Pair with T3 Code first.")?;
    let result = response_json(
        http()?
            .post(format!("{origin}/api/auth/websocket-ticket"))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|_| "Could not reach the local T3 server.")?,
    )
    .await?;
    let ticket = result
        .get("ticket")
        .and_then(Value::as_str)
        .ok_or("T3 did not return a connection ticket.")?;
    let mut url = reqwest::Url::parse(&origin).map_err(|e| e.to_string())?;
    url.set_scheme(if origin.starts_with("https:") {
        "wss"
    } else {
        "ws"
    })
    .map_err(|_| "Invalid socket scheme")?;
    url.set_path("/ws");
    url.query_pairs_mut()
        .append_pair("wsTicket", ticket)
        .append_pair("orchestrationProtocol", "2")
        .append_pair("clientSurface", "web")
        .append_pair("clientDeviceType", "desktop")
        .append_pair("connectionMethod", "direct");
    Ok(url.into())
}

#[tauri::command]
fn t3_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let mut session = state.t3.lock().map_err(|e| e.to_string())?;
    preferences::forget_pairing()?;
    *session = None;
    Ok(())
}

#[tauri::command]
async fn t3_image_upload(relative_url: String, mime_type: String, bytes: Vec<u8>, state: State<'_, AppState>) -> Result<(), String> {
    let (origin, _) = state.t3.lock().map_err(|e| e.to_string())?.clone().ok_or("Pair with T3 Code first.")?;
    t3_images::upload(&origin, &relative_url, &mime_type, bytes).await
}

#[tauri::command]
async fn t3_image_load(relative_url: String, state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let (origin, _) = state.t3.lock().map_err(|e| e.to_string())?.clone().ok_or("Pair with T3 Code first.")?;
    t3_images::load(&origin, &relative_url).await
}

#[tauri::command]
fn t3_restore(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let mut session = state.t3.lock().map_err(|e| e.to_string())?;
    if session.is_none() {
        if let Some((origin, token)) = preferences::load_pairing()? {
            *session = Some((local_origin(&origin)?, token));
        }
    }
    Ok(session.as_ref().map(|(origin, _)| origin.clone()))
}

#[tauri::command]
fn repositories_load(app: tauri::AppHandle) -> Result<Option<Vec<String>>, String> {
    preferences::load_repositories(&app.path().app_data_dir().map_err(|e| e.to_string())?)
}

#[tauri::command]
fn repositories_save(app: tauri::AppHandle, repositories: Vec<String>) -> Result<(), String> {
    preferences::save_repositories(&app.path().app_data_dir().map_err(|e| e.to_string())?, &repositories)
}

#[tauri::command]
fn favorites_load(app: tauri::AppHandle) -> Result<Option<Vec<String>>, String> {
    preferences::load_favorites(&app.path().app_data_dir().map_err(|e| e.to_string())?)
}

#[tauri::command]
fn favorites_save(app: tauri::AppHandle, repositories: Vec<String>) -> Result<(), String> {
    preferences::save_favorites(&app.path().app_data_dir().map_err(|e| e.to_string())?, &repositories)
}

#[tauri::command]
fn repository_names_load(app: tauri::AppHandle) -> Result<std::collections::HashMap<String, String>, String> {
    preferences::load_repository_names(&app.path().app_data_dir().map_err(|e| e.to_string())?)
}

#[tauri::command]
fn repository_names_save(app: tauri::AppHandle, names: std::collections::HashMap<String, String>) -> Result<(), String> {
    preferences::save_repository_names(&app.path().app_data_dir().map_err(|e| e.to_string())?, &names)
}

#[tauri::command]
fn full_path(repo: String, path: String) -> Result<String, String> {
    Ok(shell::worktree_path(&repo, &path)?.to_string_lossy().trim_start_matches("\\\\?\\").to_string())
}

#[tauri::command]
fn reveal_path(repo: String, path: String) -> Result<(), String> {
    shell::reveal(&shell::worktree_path(&repo, &path)?)
}

#[tauri::command]
fn open_editor(repo: String, path: String) -> Result<(), String> {
    let target = std::fs::canonicalize(shell::worktree_path(&repo, &path)?).map_err(|e| e.to_string())?;
    // Launch an editor executable directly, never a user-controlled shell command.
    #[cfg(windows)]
    let editor =
        std::path::PathBuf::from(std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?)
            .join("Programs/Microsoft VS Code/Code.exe");
    #[cfg(not(windows))]
    let editor = std::path::PathBuf::from("code");
    let mut command = std::process::Command::new(editor);
    command.arg("--reuse-window").arg(target);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.spawn().map_err(|e| {
        format!("Could not open VS Code. Ensure its 'code' launcher is on PATH: {e}")
    })?;
    Ok(())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    shell::open_external_url(&url)
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(debug_assertions)]
    if args.get(1).map(String::as_str) == Some("--git-request") {
        // Exercise the exact native command boundary on disposable repositories.
        use std::io::Read;
        let mut input = String::new();
        let result = std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| e.to_string())
            .and_then(|_| serde_json::from_str::<git::Request>(&input).map_err(|e| e.to_string()))
            .and_then(git::execute);
        match result {
            Ok(value) => println!("{}", json!({"ok":value})),
            Err(error) => {
                println!("{}", json!({"error":error}));
                std::process::exit(1)
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--sequence-editor") {
        let result = args
            .get(2)
            .zip(args.get(3))
            .ok_or("Missing sequence editor paths")
            .and_then(|(source, destination)| {
                std::fs::copy(source, destination)
                    .map(|_| ())
                    .map_err(|_| "Cannot write rebase plan")
            });
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            git_request,
            devops_sign_in,
            devops_pull_requests,
            devops_pr_diff,
            devops_review_vote,
            devops_tickets,
            t3_navigation::t3_window_titles,
            t3_navigation::t3_open_thread,
            t3_discover,
            t3_pair,
            t3_ticket,
            t3_image_upload,
            t3_image_load,
            t3_disconnect,
            t3_restore,
            repositories_load,
            repositories_save,
            favorites_load,
            favorites_save,
            repository_names_load,
            repository_names_save,
            open_editor,
            open_external_url,
            full_path,
            reveal_path
        ])
        .run(tauri::generate_context!())
        .expect("Could not start GitGoblin");
}
