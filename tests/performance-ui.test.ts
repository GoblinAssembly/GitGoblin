// @vitest-environment happy-dom
import { readFileSync } from 'node:fs';
import { compileScript, parse } from '@vue/compiler-sfc';
import { ModuleKind, transpile } from 'typescript';
import * as Vue from 'vue';
import * as Icons from 'lucide-vue-next';
import * as files from '../app/lib/files';
import * as selection from '../app/lib/file-selection';
import * as diff from '../app/lib/diff';
import * as windowing from '../app/lib/diff-window';
import * as statuses from '../app/lib/diff-files';
import * as geometry from '../app/lib/diff-geometry';
import * as session from '../app/lib/diff-session';
import { expect, it, vi } from 'vitest';
const imageGit = vi.fn();

function component(name: string) {
  const { descriptor } = parse(readFileSync(`app/components/${name}.vue`, 'utf8'));
  const compiled = compileScript(descriptor, { id: name, inlineTemplate: true });
  const exports: any = {};
  const modules: Record<string, unknown> = { vue: Vue, 'lucide-vue-next': Icons,
    './FileTypeIcon.vue': { default: Vue.defineComponent(() => () => Vue.h('img')) },
    '~/lib/files': files, '~/lib/file-selection': selection, '~/lib/diff': diff,
    '~/lib/diff-window': windowing, '~/lib/diff-files': statuses, '~/lib/diff-geometry': geometry, '~/lib/diff-session': session,
    '~/lib/native': { git: imageGit } };
  new Function('require', 'exports', transpile(compiled.content.replaceAll('import.meta.url', '"https://test.invalid/app.vue"'), { module: ModuleKind.CommonJS }))(
    (name: string) => modules[name], exports);
  return exports.default;
}
function mount(name: string, props: Record<string, unknown>) {
  const host = document.createElement('div');
  host.className = 'file-list';
  document.body.append(host);
  const app = Vue.createApp(component(name), props);
  for (const name of ['AppButton', 'AppCodeSurface', 'ContextMenu', 'DiffFileNavigator', 'DiffOverview', 'ImageDiff'])
    app.component(name, { setup: (_, { slots, expose }) => { expose({ close: () => {} }); return () => Vue.h(name === 'AppButton' ? 'button' : 'div', slots.default?.()); } });
  app.mount(host);
  return { host, stop: () => { app.unmount(); host.remove(); } };
}

it('only expands and collapses folders from their chevron', async () => {
  const { host, stop } = mount('DiffFileNavigator', { files: [{ path: 'src/main.ts', index: 'M', working: ' ' }], selected: '' });
  try {
    host.querySelector<HTMLElement>('span[title="src"]')!.click();
    await Vue.nextTick();
    expect(host.textContent).toContain('main.ts');
    const toggle = host.querySelector<HTMLButtonElement>('button[aria-label="Toggle folder src"]')!;
    toggle.click();
    await Vue.nextTick();
    expect(host.textContent).not.toContain('main.ts');
    toggle.click();
    await Vue.nextTick();
    expect(host.textContent).toContain('main.ts');
  } finally { stop(); }
});

it('remembers checked files only for the reviewed PR revision', async () => {
  const props = { files: [{ path: 'main.ts', index: 'M', working: ' ' }], selected: '' };
  function open(reviewKey: string) {
    const mounted = mount('DiffFileNavigator', { ...props, reviewKey });
    return { ...mounted, box: mounted.host.querySelector<HTMLInputElement>('input[type=checkbox]')! };
  }
  const first = open('pr-42-revision-a');
  first.box.click();
  await Vue.nextTick();
  expect(first.host.textContent).toContain('1 of 1 files reviewed');
  first.stop();
  const reopened = open('pr-42-revision-a');
  expect(reopened.box.checked).toBe(true);
  reopened.stop();
  const updated = open('pr-42-revision-b');
  expect(updated.box.checked).toBe(false);
  updated.stop();
});

it('previews an added image, handles decode errors, and releases its URL', async () => {
  imageGit.mockResolvedValue({ mime: 'image/png', original: null, modified: [1, 2, 3] });
  const create = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:test-image');
  const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
  const { host, stop } = mount('ImageDiff', { repo: 'repo', path: 'new.png', mode: 'staged', version: '1' });
  try {
    await Vue.nextTick(); await Vue.nextTick();
    expect(imageGit).toHaveBeenCalledWith(expect.objectContaining({ action: 'image-diff', mode: 'staged', path: 'new.png' }));
    expect(host.textContent).toContain('File does not exist in this version.');
    expect(host.querySelector('img')?.alt).toBe('Modified: new.png');
    host.querySelector('img')!.dispatchEvent(new Event('error'));
    await Vue.nextTick();
    expect(host.textContent).toContain('Unable to decode this image.');
  } finally {
    stop();
    expect(revoke).toHaveBeenCalledWith('blob:test-image');
    create.mockRestore(); revoke.mockRestore();
  }
});

it('shows an empty diff state without hiding loading or file metadata', async () => {
  for (const props of [
    { text: '', loading: false },
    { text: '', loading: true },
    { text: 'diff --git a/image.png b/image.png\nBinary files a/image.png and b/image.png differ', loading: false },
  ]) {
    const { host, stop } = mount('DiffViewer', { ...props, mode: 'working' });
    try {
      await Vue.nextTick();
      expect(Boolean(host.querySelector('.diff-empty'))).toBe(!props.text && !props.loading);
      if (props.loading) expect(host.textContent).toContain('Loading changes');
      if (props.text) expect(host.textContent).toContain('Binary files');
    } finally { stop(); }
  }
});

it('windows 5000 files and keeps the last file reachable by keyboard', async () => {
  const { host, stop } = mount('FileList', { files: Array.from({ length: 5000 }, (_, i) => ({ path: `file-${i}`, index: ' ', worktree: 'M', conflict: false })), tree: false, selected: '', mode: 'working' });
  try {
    Object.defineProperty(host, 'clientHeight', { value: 400 });
    const list = host.querySelector('ul')!;
    list.getBoundingClientRect = () => new DOMRect(0, -host.scrollTop, 300, 125000);
    expect(host.querySelectorAll('[data-path]').length).toBeLessThan(60);
    host.querySelector('button')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
    await Vue.nextTick(); await Vue.nextTick();
    expect(document.activeElement?.closest<HTMLElement>('[data-path]')?.dataset.path).toBe('file-4999');
    expect(host.querySelectorAll('[data-path]').length).toBeLessThan(60);
    document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true }));
    await Vue.nextTick(); await Vue.nextTick();
    expect(document.activeElement?.closest<HTMLElement>('[data-path]')?.dataset.path).toBe('file-0');
  } finally { stop(); }
});

it('cancels obsolete large-diff workers and renders only the latest result', async () => {
  const workers: FakeWorker[] = [];
  class FakeWorker {
    onmessage?: (event: { data: unknown }) => void;
    terminate = vi.fn();
    postMessage = vi.fn();
    constructor(public url: URL) { workers.push(this); }
  }
  vi.stubGlobal('Worker', FakeWorker);
  const props = Vue.reactive({ text: '+'.repeat(300001), mode: 'working', path: 'large.txt' });
  const host = document.createElement('div');
  const app = Vue.createApp({ setup: () => () => Vue.h(componentValue, props) });
  const componentValue = component('DiffViewer');
  for (const name of ['AppButton', 'AppCodeSurface', 'ContextMenu', 'DiffFileNavigator', 'DiffOverview', 'ImageDiff'])
    app.component(name, { setup: (_, { slots, expose }) => { expose({ close: () => {} }); return () => Vue.h(name === 'AppButton' ? 'button' : 'div', slots.default?.()); } });
  app.mount(host);
  try {
    const old = workers.find(worker => worker.url.pathname.includes('geometry'))!;
    expect(old.postMessage).toHaveBeenCalled();
    expect(host.textContent).toContain('Loading changes');
    const receive = session.createDiffSession();
    old.onmessage!({ data: receive(old.postMessage.mock.calls[0]![0]) });
    await Vue.nextTick();
    (host.querySelector('[aria-label="Increase diff font size"]') as HTMLButtonElement).click();
    await Vue.nextTick();
    expect(workers.filter(worker => worker.url.pathname.includes('geometry'))).toHaveLength(1);
    expect(old.postMessage.mock.lastCall![0].text).toBeUndefined();
    old.onmessage!({ data: receive(old.postMessage.mock.lastCall![0]) });
    await Vue.nextTick();
    props.text = 'latest file';
    await Vue.nextTick();
    expect(old.terminate).toHaveBeenCalled();
    old.onmessage!({ data: geometry.diffGeometry('obsolete file', 'split', 23, 11) });
    await Vue.nextTick();
    expect(host.textContent).toContain('latest file');
    expect(host.textContent).not.toContain('obsolete file');
  } finally { app.unmount(); vi.unstubAllGlobals(); }
});

it('toggles JSON formatting on both sides and in unified view', async () => {
  const previous = '{"id":9007199254740993,"id":0}';
  const next = '{"items":[1,2]}';
  const text = ['diff --git a/a.json b/a.json', '--- a/a.json', '+++ b/a.json', '@@ -1 +1 @@', '-' + previous, '+' + next].join('\n');
  const { host, stop } = mount('DiffViewer', { text, mode: 'working' });
  const click = (label: string) => Array.from(host.querySelectorAll('button')).find(button => button.textContent.trim() === label)!.click();
  try {
    await Vue.nextTick();
    click('Format JSON');
    for (const layout of ['Side by side', 'Unified']) {
      click(layout);
      await Vue.nextTick();
      expect(host.querySelector('.delete code')?.textContent).toBe('{\n  "id": 9007199254740993,\n  "id": 0\n}');
      expect(host.querySelector('.add code')?.textContent).toBe(JSON.stringify(JSON.parse(next), null, 2));
    }
    click('Format JSON');
    await Vue.nextTick();
    expect(host.querySelector('.delete code')?.textContent).toBe(previous);
  } finally { stop(); }
});
