<script setup lang="ts">
import { formatJsonLine, parseDiffFiles } from "~/lib/diff";
import { git } from "~/lib/native";
import type { ContextMenuHandle } from "~/lib/context-menu";
import {
  computed,
  ref,
  shallowRef,
  watch,
  onMounted,
  onUnmounted,
  nextTick,
} from "vue";
import { Check, Eye, FileCode2, LoaderCircle } from "lucide-vue-next";
import { diffFileStatus } from "~/lib/diff-files";
import { diffGeometry, diffViewport } from "~/lib/diff-geometry";
import { createDiffSession, type DiffViewRequest } from "~/lib/diff-session";
import type { DiffSyntax } from "~/lib/syntax";
const props = defineProps<{
  text: string;
  mode: string;
  path?: string;
  editable?: boolean;
  reviewKey?: string;
  loading?: boolean;
  disabled?: boolean;
  wholeFile?: boolean;
  repo?: string;
  target?: string;
  version?: string;
}>();
const syntax = shallowRef<DiffSyntax>({});
const scroller = ref<HTMLElement>();
let worker: Worker | undefined;
let syntaxRequest = 0;
function highlight() {
  syntax.value = {};
  const id = ++syntaxRequest;
  if (props.text.length <= 300_000) worker?.postMessage({ id, text: props.text, path: props.path || "" });
}
onMounted(() => {
  try {
    worker = new Worker(new URL("../lib/syntax.worker.ts", import.meta.url), {
      type: "module",
    });
    worker.onmessage = ({ data }) => {
      if (data.id === syntaxRequest) syntax.value = data.tokens;
    };
    highlight();
  } catch {
    /* Plain text remains usable when workers are unavailable. */
  }
});
onUnmounted(() => worker?.terminate());
watch(() => [props.text, props.path], highlight);
const emit = defineEmits<{
  patch: [text: string];
  editContext: [text: string];
  "update:wholeFile": [value: boolean];
}>();
const selectionMenu = ref<ContextMenuHandle>();
let editSelection = "";
async function openSelectionMenu(event: MouseEvent) {
  const selection = window.getSelection();
  if (!selection?.rangeCount || selection.isCollapsed || !scroller.value)
    return;
  const selectedRange = selection.getRangeAt(0);
  if (
    !scroller.value.contains(selectedRange.startContainer) ||
    !scroller.value.contains(selectedRange.endContainer)
  )
    return;
  const snippets: string[] = [];
  let location = "";
  for (const code of scroller.value.querySelectorAll(".diff-line code")) {
    if (!selectedRange.intersectsNode(code)) continue;
    const range = document.createRange();
    range.selectNodeContents(code);
    if (selectedRange.compareBoundaryPoints(Range.START_TO_START, range) > 0)
      range.setStart(selectedRange.startContainer, selectedRange.startOffset);
    if (selectedRange.compareBoundaryPoints(Range.END_TO_END, range) < 0)
      range.setEnd(selectedRange.endContainer, selectedRange.endOffset);
    const text = range.toString();
    if (!text) continue;
    const row = code.closest(".diff-line")!;
    const source =
      row.classList.contains("delete") ||
      (row.classList.contains("split-cell") &&
        row === row.parentElement?.firstElementChild)
        ? "Original"
        : "Modified";
    const file =
      code.closest(".hunk")?.getAttribute("data-file") || props.path || "";
    const next = `${file} (${source}, ${props.mode} diff)`;
    if (next !== location) {
      snippets.push(`\n${next}:`);
      location = next;
    }
    snippets.push(text);
  }
  if (!snippets.length) return;
  event.preventDefault();
  editSelection = `Selected code for the requested edit. Original/deleted lines may no longer exist in the current file.\n${snippets.join("\n")}`;
  selectionMenu.value?.open(event, [
    { label: "Ask T3 to edit selection", run: editInChat },
  ]);
}
function editInChat() {
  selectionMenu.value?.close();
  emit("editContext", editSelection);
}
const scroll = ref(0),
  viewportHeight = ref(600);
function onScroll() {
  if (scroller.value) {
    scroll.value = scroller.value.scrollTop;
    viewportHeight.value = scroller.value.clientHeight;
  }
}
let resize: ResizeObserver | undefined;
onMounted(() => {
  resize = new ResizeObserver(onScroll);
  if (scroller.value) resize.observe(scroller.value);
});
onUnmounted(() => resize?.disconnect());
const geometry = shallowRef(diffViewport(diffGeometry('', 'split', 23, 11), 0, 600, 23));
const selectedFile = ref("");
const navigationFiles = computed(() => geometry.value.navigationFiles);
const markdownPath = computed(() => props.path || selectedFile.value || (navigationFiles.value.length === 1 ? navigationFiles.value[0]?.path : ""));
const canPreview = computed(() => props.repo && /\.(md|markdown)$/i.test(markdownPath.value || ""));
const preview = ref(false);
type MarkdownVersions = { original: string | null; modified: string | null };
const previewVersions = shallowRef<MarkdownVersions>({ original: null, modified: null });
const previewFile = computed(() => {
  if (!preview.value || !canPreview.value) return;
  const files = parseDiffFiles(props.text);
  return files.find(file => diffFileStatus(file.header)?.path === markdownPath.value)
    || (props.path && files.length === 1 ? files[0] : undefined);
});
const previewLines = computed(() => {
  const original = new Set<number>(), modified = new Set<number>();
  for (const hunk of previewFile.value?.hunks || []) {
    for (const line of hunk.lines) {
      if (line.kind === "delete" && line.old !== null) original.add(line.old);
      if (line.kind === "add" && line.next !== null) modified.add(line.next);
    }
  }
  return { original, modified };
});
const previewSides = ["original", "modified"] as const;
const previewStatus = ref("");
watch([preview, markdownPath, () => props.repo, () => props.mode, () => props.target, () => props.version, () => props.text], async (_, __, cleanup) => {
  if (!preview.value || !canPreview.value) return;
  let active = true;
  cleanup(() => { active = false; });
  previewStatus.value = "Loading preview…";
  try {
    const versions = await git<MarkdownVersions>({ repo: props.repo!, path: markdownPath.value!, action: "markdown-preview", text: diffFileStatus(previewFile.value?.header || "")?.oldPath, mode: props.mode, target: props.target });
    if (active) {
      previewVersions.value = versions;
      previewStatus.value = "";
    }
  } catch (error) {
    if (active) previewStatus.value = String(error);
  }
});
watch(() => [props.path, props.mode], () => { selectedFile.value = ""; });
watch(navigationFiles, (files) => {
  if (!files.some(file => file.path === selectedFile.value)) selectedFile.value = "";
});
watch(selectedFile, async () => {
  selected.value = new Set();
  await nextTick();
  if (scroller.value) scroller.value.scrollTop = 0;
  onScroll();
});
const formatJson = ref(false);
const wrapLines = ref(false);
const fontSize = ref(11);
const rowHeight = computed(() => fontSize.value + 12);
onMounted(() => {
  try {
    const saved = Number(localStorage.getItem("gitgoblin.diffFontSize"));
    if (Number.isInteger(saved) && saved >= 9 && saved <= 20)
      fontSize.value = saved;
  } catch {
    /* Keep the default when storage is unavailable. */
  }
});
watch(fontSize, async (size) => {
  try {
    localStorage.setItem("gitgoblin.diffFontSize", String(size));
  } catch {
    /* Keep the size in memory. */
  }
  await nextTick();
  onScroll();
});
const layout = ref("split");
const horizontalScroll = ref({ left: 0, right: 0 });
const horizontalWidth = ref({ left: 0, right: 0 });
function scrollSide(side: "left" | "right", event: Event) {
  horizontalScroll.value[side] = (
    event.currentTarget as HTMLElement
  ).scrollLeft;
}
let horizontalResize: ResizeObserver | undefined;
watch(
  () => [props.text, selectedFile.value, layout.value, wrapLines.value, formatJson.value, fontSize.value],
  async () => {
    await nextTick();
    horizontalResize?.disconnect();
    horizontalResize = new ResizeObserver(() => {
      for (const side of ["left", "right"] as const) {
        const bar = scroller.value?.parentElement?.querySelector<HTMLElement>(
          `[data-scroll-side="${side}"]`,
        );
        horizontalWidth.value[side] = bar?.scrollWidth || 0;
      }
    });
    for (const content of scroller.value?.parentElement?.querySelectorAll(
      ".side-scrollbar code",
    ) || [])
      horizontalResize.observe(content);
  },
  { immediate: true, flush: "post" },
);
onUnmounted(() => horizontalResize?.disconnect());
// The worker retains full lines and returns only the current viewport.
const large = computed(() => geometry.value.large);
let geometryWorker: Worker | undefined;
let workerText = '';
let localSession = createDiffSession();
let viewRequest = 0, requestId = 0;
let pendingPatch: { id: number; text: string; path?: string; mode: string } | undefined;
const preparing = ref(false), patching = ref(false);
function receiveGeometry(data: ReturnType<ReturnType<typeof createDiffSession>>) {
  if (pendingPatch?.id === data.id) {
    const current = pendingPatch;
    pendingPatch = undefined;
    patching.value = false;
    if (current.text !== props.text || current.path !== props.path || current.mode !== props.mode) return;
    if (data.error) error.value = data.error;
    else if (data.patch) emit('patch', data.patch);
    return;
  }
  if (data.id !== viewRequest) return;
  if (data.view) {
    geometry.value = data.view;
    if (data.view.large) wrapLines.value = false;
  }
  if (data.error) error.value = data.error;
  preparing.value = false;
}
function requestGeometry(rebuild = false) {
  const text = props.text;
  const request: DiffViewRequest = { id: ++requestId, layout: layout.value, rowHeight: rowHeight.value,
    fontSize: fontSize.value, selectedFile: selectedFile.value, scroll: scroll.value, height: viewportHeight.value };
  viewRequest = request.id;
  if (rebuild) preparing.value = true;
  if (text.length <= 300_000 || typeof Worker === 'undefined') {
    if (geometryWorker) { geometryWorker.terminate(); geometryWorker = undefined; pendingPatch = undefined; patching.value = false; }
    receiveGeometry(localSession({ ...request, text }));
    return;
  }
  if (!geometryWorker || workerText !== text) {
    geometryWorker?.terminate();
    pendingPatch = undefined; patching.value = false;
    localSession = createDiffSession();
    workerText = text;
    try {
      const worker = new Worker(new URL('../lib/diff-geometry.worker.ts', import.meta.url), { type: 'module' });
      geometryWorker = worker;
      worker.onmessage = ({ data }) => { if (geometryWorker === worker) receiveGeometry(data); };
      worker.onerror = () => {
        if (geometryWorker !== worker) return;
        worker.terminate(); geometryWorker = undefined;
        pendingPatch = undefined; patching.value = false;
        receiveGeometry(localSession({ ...request, id: viewRequest, text, layout: layout.value, rowHeight: rowHeight.value, fontSize: fontSize.value, selectedFile: selectedFile.value, scroll: scroll.value, height: viewportHeight.value }));
      };
      request.text = text;
    } catch { geometryWorker = undefined; receiveGeometry(localSession({ ...request, text })); return; }
  }
  geometryWorker.postMessage(request);
}
watch(() => [props.text, layout.value, rowHeight.value, fontSize.value, selectedFile.value], () => requestGeometry(true), { immediate: true });
watch([scroll, viewportHeight], () => requestGeometry());
onUnmounted(() => geometryWorker?.terminate());
const files = computed(() => geometry.value.files);
const formattedJson = computed(() => {
  const formatted = new Map<number, string>();
  if (!formatJson.value || large.value) return formatted;
  for (const file of files.value) for (const hunk of file.hunks) {
    const lines = layout.value === "split"
      ? hunk.visibleRows.flatMap(row => [row.left, row.right])
      : hunk.visibleLines;
    for (const line of lines) {
      if (!line || formatted.has(line.index) || line.kind === "meta") continue;
      const pretty = formatJsonLine(line.text.slice(1));
      if (pretty !== undefined) formatted.set(line.index, pretty);
    }
  }
  return formatted;
});
let jumpToChangePending = false;
watch(
  () =>
    [
      props.path,
      props.mode,
      props.wholeFile,
      props.text,
      props.loading,
      preparing.value,
    ] as const,
  async (current, previous) => {
    if (
      !previous ||
      current.slice(0, 3).some((value, i) => value !== previous[i])
    )
      jumpToChangePending = !!props.wholeFile;
    if (!jumpToChangePending || props.loading || preparing.value || !props.text) return;
    await nextTick();
    const el = scroller.value;
    if (!jumpToChangePending || !props.wholeFile || props.loading || !el)
      return;
    jumpToChangePending = false;
    // Virtualized changes may be outside the DOM; wrapped rows need measured positions.
    const row = el.querySelector<HTMLElement>(
      ".diff-line.add, .diff-line.delete",
    );
    const offset = large.value
      ? geometry.value.changes[0]?.offset
      : row
        ? row.getBoundingClientRect().top -
          el.getBoundingClientRect().top +
          el.scrollTop
        : undefined;
    if (offset === undefined) return;
    el.scrollTop = Math.max(0, offset - el.clientHeight / 3);
    onScroll();
  },
  { immediate: true, flush: "post" },
);
const selected = ref(new Set<number>());
const error = ref("");
watch(
  () => props.text,
  () => {
    selectionMenu.value?.close();
    selected.value = new Set();
    error.value = "";
    horizontalScroll.value = { left: 0, right: 0 };
  },
);
function toggle(index: number) {
  const next = new Set(selected.value);
  next.has(index) ? next.delete(index) : next.add(index);
  selected.value = next;
}
function stage(indices?: number[], hunkStart?: number) {
  if (preparing.value || patching.value) return;
  error.value = '';
  const request = { id: ++requestId, patch: true as const, indices: indices || [...selected.value], hunkStart, reverse: props.mode === 'staged' };
  pendingPatch = { id: request.id, text: props.text, mode: props.mode, path: props.path };
  patching.value = true;
  if (geometryWorker) geometryWorker.postMessage(request);
  else receiveGeometry(localSession(request));
}
</script>
<template>
  <AppCodeSurface
    class="diff-view flex flex-col min-h-0 flex-1"
    :style="{
      '--diff-font-size': `${fontSize}px`,
      '--diff-row-height': `${rowHeight}px`,
    }"
    :class="{
      'side-by-side': layout === 'split',
      'large-diff': large,
      'wrap-lines': wrapLines && !large,
    }"
  >
    <div
      class="diff-actions flex items-center py-[6px] px-[12px] [&>span]:flex-1 [&_.small]:[border-color:transparent] [@media(max-width:1250px)]:[&>span]:hidden [@media(max-width:1100px)]:flex-wrap flex-wrap gap-[6px] min-h-[42px] border-b border-solid border-[var(--line)] text-[var(--muted)] text-[11px] [&_.small]:min-h-[28px] [&_.small]:text-[11px] [&_.small]:text-[var(--fg-secondary)] [&_button[aria-pressed='true']]:bg-[var(--selection)]! [&_button[aria-pressed='true']]:text-[var(--accent)]!"
    >
      <div
        class="diff-layout flex shrink-0 border-solid border-[var(--line)] p-[2px] [&_button]:py-[3px] [&_button]:px-[7px] [&_button]:rounded-[3px] [&_button[aria-pressed='true']]:bg-[var(--raised)] [&_button[aria-pressed='true']]:text-[var(--fg)] bg-[var(--chrome)] border-0 rounded-[6px] [&_button]:min-h-[28px] [&_button]:text-[11px] [&_button]:text-[var(--fg-secondary)]"
        role="group"
        aria-label="Diff layout"
      >
        <AppButton
          :aria-pressed="layout === 'unified'"
          @click="layout = 'unified'"
        >
          Unified
        </AppButton>
        <AppButton :aria-pressed="layout === 'split'" @click="layout = 'split'">
          Side by side
        </AppButton>
      </div>
      <AppButton
        class="small wrap-toggle text-[11px] py-[4px] px-[7px] [border-color:var(--line)] rounded-[4px] [&[aria-pressed='true']]:bg-[var(--raised)] [&[aria-pressed='true']]:text-[var(--fg)]"
        :aria-pressed="wrapLines && !large"
        :disabled="large"
        :title="large ? 'Wrapping is unavailable for large diffs' : undefined"
        @click="wrapLines = !wrapLines"
      >
        Wrap lines
      </AppButton>
      <AppButton
        class="small"
        :aria-pressed="formatJson && !large"
        :disabled="large"
        :title="large ? 'JSON formatting is unavailable for large diffs' : 'Expand single-line JSON objects and arrays in this view'"
        @click="formatJson = !formatJson"
      >
        Format JSON
      </AppButton>
      <AppButton
        v-if="wholeFile !== undefined"
        class="small text-[11px] py-[4px] px-[7px] [border-color:var(--line)] rounded-[4px]"
        :aria-pressed="wholeFile"
        @click="emit('update:wholeFile', !wholeFile)"
      >
        Whole file
      </AppButton>
      <div
        class="diff-font-controls flex items-center shrink-0 gap-[3px] [&_output]:min-w-[28px] [&_output]:text-center [&_output]:text-[10px]"
        role="group"
        aria-label="Diff font size"
      >
        <AppButton
          class="small text-[11px] py-[4px] px-[7px] [border-color:var(--line)] rounded-[4px]"
          aria-label="Decrease diff font size"
          title="Decrease diff font size"
          :disabled="fontSize <= 9"
          @click="fontSize--"
        >
          A−
        </AppButton>
        <output aria-label="Diff font size" aria-live="polite"
          >{{ fontSize }}px</output
        >
        <AppButton
          class="small text-[11px] py-[4px] px-[7px] [border-color:var(--line)] rounded-[4px]"
          aria-label="Increase diff font size"
          title="Increase diff font size"
          :disabled="fontSize >= 20"
          @click="fontSize++"
        >
          A+
        </AppButton>
      </div>
      <LoaderCircle
        class="spin diff-loading animate-spin [visibility:hidden] shrink-0 [&.visible]:[visibility:visible]"
        :class="{ visible: loading || patching }"
        :size="13"
        :aria-hidden="!loading && !patching"
        :aria-label="patching ? 'Preparing selected changes' : 'Refreshing diff'"
      />
      <span>{{
        selected.size
          ? `${selected.size} lines selected`
          : "Select changed lines to stage or discuss"
      }}</span
      ><AppButton
        v-if="editable && selected.size"
        class="small text-[11px] py-[4px] px-[7px] [border-color:var(--line)] rounded-[4px]"
        :disabled="disabled || loading || patching"
        @click="stage()"
      >
        <Check :size="13" />{{
          mode === "staged" ? "Unstage" : "Stage"
        }}
        selection</AppButton>
      <AppButton
        v-if="canPreview"
        class="small ml-auto shrink-0 gap-1.5 whitespace-nowrap"
        :aria-pressed="preview"
        @click="preview = !preview"
      >
        <Eye :size="14" aria-hidden="true" />Markdown preview
      </AppButton>
    </div>
    <div
      v-if="error"
      class="inline-error text-[var(--danger)] bg-[var(--danger-bg)] py-[10px] px-[12px] text-[11px] leading-[1.7] whitespace-pre-wrap [overflow-wrap:anywhere]"
    >
      {{ error }}
    </div>
    <div class="diff-body flex min-h-0 flex-1 [&_.diff-scroll]:min-w-0">
      <DiffFileNavigator v-if="navigationFiles.length > 1 || (reviewKey && navigationFiles.length)" :review-key="reviewKey" :files="navigationFiles" :selected="selectedFile" @select="selectedFile = $event" />
      <div v-if="preview && canPreview" class="min-w-0 flex-1 overflow-auto p-4">
        <p v-if="previewStatus" role="status">{{ previewStatus }}</p>
        <div v-else class="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(min(100%,32ch),1fr))]">
          <template v-for="side in previewSides" :key="side">
            <section v-if="previewVersions[side] !== null" class="min-w-0"
              :class="side === 'original' ? '[&_.markdown-change]:bg-[var(--danger-bg)]' : '[&_.markdown-change]:bg-[var(--success-bg)]'">
              <h3 class="mb-3 text-xs text-[var(--fg-secondary)]">{{ side === 'original' ? 'Original · removals' : 'Modified · additions' }}</h3>
              <ChatMarkdown class="mx-auto max-w-[80ch]" :text="previewVersions[side]!" :changed-lines="previewLines[side]" />
            </section>
          </template>
        </div>
      </div>
      <div v-show="!(preview && canPreview)" class="diff-main flex flex-col flex-1 min-w-0 min-h-0 relative">
        <div
          v-if="preparing || (loading && !text)"
          class="diff-pending absolute inset-0 z-[1] flex flex-col items-center justify-center gap-[10px] bg-[var(--panel)] text-[var(--muted)] pointer-events-none"
          role="status"
        >
          <LoaderCircle :size="22" class="spin animate-spin" /><span
            >Loading changes...</span
          >
        </div>
        <div
          ref="scroller"
          class="diff-scroll overflow-auto flex-1 min-h-0 pb-[30px] [overflow-anchor:none] [.side-by-side_&]:overflow-x-hidden has-[.diff-empty]:flex has-[.diff-empty]:flex-col"
          @contextmenu="openSelectionMenu"
          @scroll="onScroll"
        >
          <div v-if="!text && !loading && !preparing" class="diff-empty m-auto py-[40px] px-[24px] text-center text-[var(--muted)] [&>svg]:box-content! [&>svg]:p-[16px] [&>svg]:mx-auto [&>svg]:mt-0 [&>svg]:mb-[16px] [&>svg]:border [&>svg]:border-solid [&>svg]:border-[var(--line)] [&>svg]:rounded-[14px] [&>svg]:bg-[var(--panel)] [&>svg]:text-[var(--accent)] [&_h3]:mt-0 [&_h3]:mx-0 [&_h3]:mb-[8px] [&_h3]:text-[16px] [&_h3]:font-medium [&_h3]:text-[var(--fg)] [&_h3]:tracking-[-.3px] [&_p]:m-0 [&_p]:text-[12px] [&_p]:leading-[1.6]" role="status">
            <FileCode2 :size="26" aria-hidden="true" />
            <h3>No text changes to display</h3>
            <p>Select another file to review its changes.</p>
          </div>
          <div
            v-else-if="!preparing"
            class="diff-content"
            :style="
              large && layout !== 'split'
                ? { minWidth: `${geometry.width}px` }
                : undefined
            "
          >
            <template v-for="(file, fileIndex) in files" :key="fileIndex">
              <div
                v-if="files.length > 1"
                class="diff-file-label [.large-diff_&]:h-[32px] [.large-diff_&]:whitespace-nowrap py-[9px] px-[12px] text-[var(--fg-secondary)] text-[11px] bg-[var(--panel)] [overflow-wrap:anywhere]"
              >
                {{ diffFileStatus(file.header)?.path || file.header.split("\n")[0] }}
              </div>
              <div
                v-if="layout === 'split' && file.hunks.length"
                class="split-labels [.large-diff_&]:h-[29px] grid grid-cols-[minmax(0,_1fr)_minmax(0,_1fr)] bg-[var(--panel)] border-b-[1px] border-solid border-b-[var(--line)] text-[var(--muted)] text-[10px] [&_span]:py-[7px] [&_span]:px-[15px] [&_span:first-child]:border-r-[1px] [&_span:first-child]:border-solid [&_span:first-child]:border-r-[var(--line)] [.wrap-lines_&]:min-w-0"
              >
                <span>Original</span><span>Modified</span>
              </div>
              <section
                v-for="hunk in file.hunks"
                :key="hunk.start"
                class="hunk min-w-full w-max [.side-by-side_&]:w-full [.side-by-side_&]:min-w-0 [.wrap-lines_&]:w-full [.wrap-lines_&]:min-w-0"
                :data-file="props.path || diffFileStatus(file.header)?.path || file.header.split('\n')[0]"
              >
                <div
                  class="hunk-header min-h-[42px] flex items-center justify-between gap-[15px] py-[7px] px-[12px] bg-[var(--panel)] text-[var(--muted)] text-[10px] border-t-[1px] border-solid border-t-[var(--line)] border-b-[1px] border-b-[var(--line)] mt-[8px] mx-0 mb-0 [&_button]:text-[9px] [&_button]:bg-[var(--selection)] [&_button]:text-[var(--fg-secondary)] [&_button]:[border-color:var(--line)] [.large-diff_&]:h-[42px]"
                >
                  <code
                    class="[:where(&)]:font-['Cascadia_Code',_'JetBrains_Mono',_Consolas,_monospace]"
                    >{{ hunk.header }}</code
                  ><AppButton
                    v-if="editable"
                    class="small text-[11px] py-[4px] px-[7px] [border-color:var(--line)] rounded-[4px]"
                    :disabled="disabled || loading || patching"
                    @click="stage(undefined, hunk.start)"
                  >
                    {{ mode === "staged" ? "Unstage" : "Stage" }} hunk
                  </AppButton>
                </div>
                <div
                  v-if="hunk.window.before"
                  :style="{ height: `${hunk.window.before}px` }"
                  aria-hidden="true"
                />
                <div v-if="layout === 'split'" class="split-lines">
                  <div
                    v-for="(row, rowIndex) in hunk.visibleRows"
                    :key="rowIndex + hunk.window.start"
                    class="split-row grid grid-cols-[minmax(0,_1fr)_minmax(0,_1fr)]"
                  >
                    <div
                      v-for="side in ['left', 'right'] as const"
                      :key="side"
                      class="diff-line split-cell block items-stretch min-h-[var(--diff-row-height)] text-[length:var(--diff-font-size)] leading-[var(--diff-row-height)] whitespace-pre min-w-0 w-full [&_code]:pr-[12px] [&_code]:text-[var(--fg-secondary)] [&_code]:[tab-size:2] [&.add]:bg-[var(--success-bg)] [&.add_code]:text-[var(--success)] [&.add_.line-sign]:text-[var(--success)] [&.add_.line-number]:bg-[var(--success-bg)] [&.add_.line-number]:text-[var(--success)] [&.delete]:bg-[var(--danger-bg)] [&.delete_code]:text-[var(--danger)] [&.delete_.line-sign]:text-[var(--danger)] [&.delete_.line-number]:bg-[var(--danger-bg)] [&.delete_.line-number]:text-[var(--danger)] [&.selected]:bg-[var(--selection)] [&.selected]:shadow-[inset_2px_0_#8ea6e6] [&.meta]:text-[var(--muted)] [.large-diff_&]:h-[var(--diff-row-height)] [.large-diff_&]:min-h-[var(--diff-row-height)] [.large-diff_&_code]:whitespace-pre [.large-diff_&_code]:[overflow-wrap:normal] [&.delete_.diff-inline-change]:text-[var(--fg)] [&.delete_.diff-inline-change]:bg-[color-mix(in_srgb,var(--danger)_30%,var(--danger-bg))] [&.delete_.diff-inline-change]:rounded-[2px] [&.delete_.diff-inline-change]:shadow-[inset_0_-1px_#ff8c9d] [&.add_.diff-inline-change]:text-[var(--fg)] [&.add_.diff-inline-change]:bg-[color-mix(in_srgb,var(--success)_30%,var(--success-bg))] [&.add_.diff-inline-change]:rounded-[2px] [&.add_.diff-inline-change]:shadow-[inset_0_-1px_#79dfa0] overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden [&:first-child]:border-r-[1px] [&:first-child]:border-solid [&:first-child]:border-r-[var(--line)] [&_code]:min-w-0 [&_code]:flex-1 [&_code]:whitespace-pre [.wrap-lines_&]:w-full [.wrap-lines_&]:min-w-0 [.wrap-lines_&_code]:min-w-0 [.wrap-lines_&_code]:flex-1 [.wrap-lines_&_code]:whitespace-pre-wrap [.wrap-lines_&_code]:[overflow-wrap:anywhere] [&.empty-cell]:bg-[var(--raised)]"
                      :class="[
                        row[side]?.kind,
                        {
                          selected: row[side] && selected.has(row[side]!.index),
                          'empty-cell': !row[side],
                        },
                      ]"
                      :scrollLeft="wrapLines ? 0 : horizontalScroll[side]"
                      @scroll="scrollSide(side, $event)"
                    >
                      <div
                        class="split-line-content flex min-h-[var(--diff-row-height)] min-w-full w-max [.wrap-lines_&]:w-full [.wrap-lines_&]:min-w-0"
                        :style="
                          !wrapLines
                            ? { minWidth: `${horizontalWidth[side]}px` }
                            : undefined
                        "
                      >
                        <template v-if="row[side]">
                          <AppButton
                            v-if="
                              row[side]!.kind === 'add' ||
                              row[side]!.kind === 'delete'
                            "
                            class="line-select flex justify-center items-center w-[20px] min-w-[20px] max-w-[20px] min-h-[var(--diff-row-height)] rounded-none p-0 text-[15px] opacity-[0.5] text-[var(--muted)] leading-[1] hover:opacity-[1]"
                            :aria-label="`Select ${side === 'left' ? 'old' : 'new'} line ${row[side]!.old || row[side]!.next}`"
                            :aria-pressed="selected.has(row[side]!.index)"
                            @click="toggle(row[side]!.index)"
                          >
                            <Check
                              v-if="selected.has(row[side]!.index)"
                              :size="11"
                            /><span v-else>&middot;</span></AppButton
                          ><span
                            v-else
                            class="line-select flex justify-center items-center w-[20px] min-w-[20px] max-w-[20px] min-h-[var(--diff-row-height)] rounded-none p-0 text-[15px] opacity-[0.5] text-[var(--muted)] leading-[1] hover:opacity-[1]"
                          />
                          <span
                            class="line-number w-[34px] shrink-0 text-right pr-[8px] text-[var(--muted)] select-none text-[10px]"
                            >{{
                              side === "left" ? row[side]!.old : row[side]!.next
                            }}</span
                          >
                          <span
                            class="line-sign w-[20px] shrink-0 text-center text-[var(--muted)] select-none"
                            >{{
                              row[side]!.kind === "add"
                                ? "+"
                                : row[side]!.kind === "delete"
                                  ? "\u2212"
                                  : ""
                            }}</span
                          >
                          <code
                            class="[:where(&)]:font-['Cascadia_Code',_'JetBrains_Mono',_Consolas,_monospace]"
                            ><template v-if="formattedJson.has(row[side]!.index)">{{ formattedJson.get(row[side]!.index) }}</template

                              ><template
                              v-else-if="
                                syntax[row[side]!.index]?.[
                                  side === 'left' ? 'old' : 'next'
                                ]
                              "
                              ><span
                                v-for="(token, i) in syntax[row[side]!.index]![
                                  side === 'left' ? 'old' : 'next'
                                ]"
                                :key="i"
                                :class="token.classes"
                                :style="token.color ? { '--syntax-color': token.color, '--syntax-light-color': token.lightColor } : undefined"
                                >{{ token.text }}</span
                              ></template
                            ><template v-else>{{
                              row[side]!.kind === "meta"
                                ? row[side]!.text
                                : row[side]!.text.slice(1)
                            }}</template></code
                          >
                        </template>
                      </div>
                    </div>
                  </div>
                </div>
                <template v-else>
                  <div
                    v-for="line in hunk.visibleLines"
                    :key="line.index"
                    class="diff-line flex items-stretch min-h-[var(--diff-row-height)] text-[length:var(--diff-font-size)] leading-[var(--diff-row-height)] whitespace-pre min-w-full w-max [&_code]:pr-[25px] [&_code]:text-[var(--fg-secondary)] [&_code]:[tab-size:2] [&.add]:bg-[var(--success-bg)] [&.add_code]:text-[var(--success)] [&.add_.line-sign]:text-[var(--success)] [&.add_.line-number]:bg-[var(--success-bg)] [&.add_.line-number]:text-[var(--success)] [&.delete]:bg-[var(--danger-bg)] [&.delete_code]:text-[var(--danger)] [&.delete_.line-sign]:text-[var(--danger)] [&.delete_.line-number]:bg-[var(--danger-bg)] [&.delete_.line-number]:text-[var(--danger)] [&.selected]:bg-[var(--selection)] [&.selected]:shadow-[inset_2px_0_#8ea6e6] [&.meta]:text-[var(--muted)] [.large-diff_&]:h-[var(--diff-row-height)] [.large-diff_&]:min-h-[var(--diff-row-height)] [&.delete_.diff-inline-change]:text-[var(--fg)] [&.delete_.diff-inline-change]:bg-[color-mix(in_srgb,var(--danger)_30%,var(--danger-bg))] [&.delete_.diff-inline-change]:rounded-[2px] [&.delete_.diff-inline-change]:shadow-[inset_0_-1px_#ff8c9d] [&.add_.diff-inline-change]:text-[var(--fg)] [&.add_.diff-inline-change]:bg-[color-mix(in_srgb,var(--success)_30%,var(--success-bg))] [&.add_.diff-inline-change]:rounded-[2px] [&.add_.diff-inline-change]:shadow-[inset_0_-1px_#79dfa0] [.wrap-lines_&]:w-full [.wrap-lines_&]:min-w-0 [.wrap-lines_&_code]:min-w-0 [.wrap-lines_&_code]:flex-1 [.wrap-lines_&_code]:whitespace-pre-wrap [.wrap-lines_&_code]:[overflow-wrap:anywhere]"
                    :class="[line.kind, { selected: selected.has(line.index) }]"
                  >
                    <AppButton
                      v-if="line.kind === 'add' || line.kind === 'delete'"
                      class="line-select flex justify-center items-center w-[20px] min-w-[20px] max-w-[20px] min-h-[var(--diff-row-height)] rounded-none p-0 text-[15px] opacity-[0.5] text-[var(--muted)] leading-[1] hover:opacity-[1]"
                      :aria-label="`Select changed line ${line.next || line.old}`"
                      :aria-pressed="selected.has(line.index)"
                      @click="toggle(line.index)"
                    >
                      <Check v-if="selected.has(line.index)" :size="11" /><span
                        v-else
                        >·</span
                      ></AppButton
                    ><span
                      v-else
                      class="line-select flex justify-center items-center w-[20px] min-w-[20px] max-w-[20px] min-h-[var(--diff-row-height)] rounded-none p-0 text-[15px] opacity-[0.5] text-[var(--muted)] leading-[1] hover:opacity-[1]"
                    />
                    <span
                      class="line-number w-[34px] shrink-0 text-right pr-[8px] text-[var(--muted)] select-none text-[10px]"
                      >{{ line.old }}</span
                    ><span
                      class="line-number w-[34px] shrink-0 text-right pr-[8px] text-[var(--muted)] select-none text-[10px]"
                      >{{ line.next }}</span
                    ><span
                      class="line-sign w-[20px] shrink-0 text-center text-[var(--muted)] select-none"
                      >{{
                        line.kind === "add"
                          ? "+"
                          : line.kind === "delete"
                            ? "−"
                            : ""
                      }}</span
                    ><code
                      class="[:where(&)]:font-['Cascadia_Code',_'JetBrains_Mono',_Consolas,_monospace]"
                      ><template v-if="formattedJson.has(line.index)">{{ formattedJson.get(line.index) }}</template

                        ><template
                        v-else-if="
                          syntax[line.index]?.[
                            line.kind === 'delete' ? 'old' : 'next'
                          ]
                        "
                        ><span
                          v-for="(token, i) in syntax[line.index]![
                            line.kind === 'delete' ? 'old' : 'next'
                          ]"
                          :key="i"
                          :class="token.classes"
                          :style="token.color ? { '--syntax-color': token.color, '--syntax-light-color': token.lightColor } : undefined"
                          >{{ token.text }}</span
                        ></template
                      ><template v-else>{{
                        line.kind === "meta" ? line.text : line.text.slice(1)
                      }}</template></code
                    >
                  </div>
                </template>
                <div
                  v-if="hunk.window.after"
                  :style="{ height: `${hunk.window.after}px` }"
                  aria-hidden="true"
                />
              </section>
              <ImageDiff
                v-if="repo && /\.(png|jpe?g|gif|webp|bmp|ico|svg|avif)$/i.test(path || diffFileStatus(file.header)?.path || '')"
                :repo="repo" :path="path || diffFileStatus(file.header)?.path || ''"
                :old-path="diffFileStatus(file.header)?.oldPath"
                :mode="mode" :target="target" :version="version || text"
              />
              <pre
                v-else-if="!file.hunks.length"
                class="plain-output text-[11px] py-[15px] px-[18px] text-[var(--fg-secondary)] [.large-diff_&]:m-0 [.large-diff_&]:p-[12px] [.large-diff_&]:leading-[20px] [.large-diff_&]:whitespace-pre [:where(&)]:font-['Cascadia_Code',_'JetBrains_Mono',_Consolas,_monospace] [:where(&)]:whitespace-pre-wrap [:where(&)]:[word-break:break-word] [:where(&)]:leading-[1.7]"
                :style="large ? { height: `${file.plainHeight}px` } : undefined"
                >{{
                  file.header ||
                  (loading
                    ? "Loading changes..."
                    : "No text changes to display.")
                }}</pre>
            </template>
          </div>
        </div>
        <div
          v-if="layout === 'split' && !wrapLines"
          class="split-scrollbars grid grid-cols-[minmax(0,_1fr)_minmax(0,_1fr)] shrink-0"
        >
          <div
            v-for="side in ['left', 'right'] as const"
            :key="side"
            class="side-scrollbar [overflow-x:scroll] overflow-y-hidden min-w-0 h-[16px] [&:first-child]:border-r-[1px] [&:first-child]:border-solid [&:first-child]:border-r-[var(--line)] [&_code]:block [&_code]:[box-sizing:content-box] [&_code]:w-max [&_code]:[min-width:calc(100%_-_86px)] [&_code]:pl-[74px] [&_code]:pr-[12px] [&_code]:h-[1px] [&_code]:text-[length:var(--diff-font-size)] [&_code]:whitespace-pre [&_code]:[visibility:hidden]"
            tabindex="0"
            role="region"
            :data-scroll-side="side"
            :aria-label="`Scroll ${side === 'left' ? 'original' : 'modified'} horizontally`"
            :scrollLeft="horizontalScroll[side]"
            @scroll="scrollSide(side, $event)"
          >
            <code
              class="[:where(&)]:font-['Cascadia_Code',_'JetBrains_Mono',_Consolas,_monospace]"
              aria-hidden="true"
              >{{ geometry.longest[side] }}</code
            >
          </div>
        </div>
      </div>
      <DiffOverview
        v-if="wholeFile"
        :scroller="scroller"
        :revision="[text, layout, wrapLines, formatJson, fontSize]"
        :changes="large ? geometry.changes : undefined"
      />
    </div>
    <ContextMenu ref="selectionMenu" />
  </AppCodeSurface>
</template>
