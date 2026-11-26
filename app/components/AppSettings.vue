<script setup lang="ts">
import { Check, Keyboard, Link2, LoaderCircle, Palette, X } from 'lucide-vue-next';
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from 'reka-ui';
import { themes, type Theme } from '~/composables/useTheme';
import { ref } from 'vue';
const hideSnoozed = defineModel<boolean>("hideSnoozed");
const autoCloseSettled = defineModel<boolean>("autoCloseSettled");
const props = defineProps<{ syncEnabled?: boolean; syncError?: string; theme: Theme; error: string; t3Connected?: boolean; t3Origin?: string; t3Error?: string; pairT3?: (pairing: string, server: string) => Promise<void>; category?: string }>();
defineEmits<{ sync: [enabled: boolean]; close: []; theme: [value: Theme] }>();
const server = ref(props.t3Origin || 'http://127.0.0.1:3773');
const pairing = ref('');
const pairingError = ref('');
const pairingBusy = ref(false);
async function pair() {
  if (pairingBusy.value || !pairing.value.trim() || !props.pairT3) return;
  pairingBusy.value = true;
  pairingError.value = '';
  try {
    await props.pairT3(pairing.value, server.value);
    pairing.value = '';
  } catch (error) {
    pairingError.value = String(error);
  } finally {
    pairingBusy.value = false;
  }
}
</script>

<template>
  <AppDialog title="Settings" :dismissible="!pairingBusy" @close="$emit('close')">
    <AppDialogPanel class="settings-modal w-[920px] max-w-[calc(100vw-32px)] h-[600px] max-h-[calc(100dvh-32px)] flex flex-col overflow-hidden">
      <header class="settings-heading flex items-center justify-between py-[16px] px-[20px] border-b border-[var(--line)] [&_h2]:m-0 [&_h2]:text-[18px] [&_h2]:font-semibold [&_h2]:tracking-[-.3px]">
        <h2>Settings</h2>
        <AppButton size="icon" aria-label="Close settings" :disabled="pairingBusy" @click="$emit('close')"><X :size="18" /></AppButton>
      </header>
      <TabsRoot :default-value="category || 'appearance'" orientation="vertical" class="settings-layout flex flex-1 min-h-0">
        <TabsList class="settings-nav flex flex-col gap-[4px] basis-[170px] shrink-0 grow-0 py-[12px] px-[8px] bg-[var(--raised)] border-r border-[var(--line)] [&>button]:justify-start [&>button]:p-[10px] [&>button]:text-[var(--fg-secondary)] [&>button[data-state=active]]:bg-[var(--selection)] [&>button[data-state=active]]:text-[var(--fg)] [@media(max-width:650px)]:basis-[54px] [@media(max-width:650px)]:[&>button]:justify-center" aria-label="Settings categories">
          <TabsTrigger value="appearance" as-child><AppButton><Palette :size="16" /><span class="[@media(max-width:650px)]:sr-only">Appearance</span></AppButton></TabsTrigger>
          <TabsTrigger value="shortcuts" as-child><AppButton><Keyboard :size="16" /><span class="[@media(max-width:650px)]:sr-only">Shortcuts</span></AppButton></TabsTrigger>
          <TabsTrigger value="t3" as-child><AppButton><Link2 :size="16" /><span class="[@media(max-width:650px)]:sr-only">T3 Code</span></AppButton></TabsTrigger>
        </TabsList>
        <div class="settings-content flex-1 min-w-0 p-[24px] overflow-auto [&_h3]:mt-0 [&_h3]:mx-0 [&_h3]:mb-[12px] [&_h3]:text-[16px] [&_h3]:font-medium [&_p]:text-[var(--muted)] [&_p]:text-[12px] [&_p]:leading-[1.7] [&_p]:mt-0 [&_p]:mx-0 [&_p]:mb-[18px] [&_[role=alert]]:text-[var(--danger)] [@media(max-width:650px)]:p-[16px]">
          <TabsContent value="appearance">
            <h3>Appearance</h3>
            <p>Choose how GitGoblin looks on this device.</p>
            <div class="theme-options grid grid-cols-3 gap-[8px] mb-[18px] [@media(max-width:650px)]:grid-cols-1" role="group" aria-label="Theme">
              <AppButton v-for="option in themes" :key="option.id" class="theme-option flex items-stretch flex-col p-[10px] border border-[var(--line)] text-left gap-[6px] bg-[var(--bg)] whitespace-normal min-w-0 aria-pressed:border-[var(--accent)] aria-pressed:shadow-[inset_0_0_0_1px_var(--accent)] [&_strong]:text-[12px] [&_strong]:font-semibold [&_small]:text-[var(--muted)] [&_small]:text-[10px] [&_small]:leading-[1.5]"
                :aria-label="option.name" :aria-pressed="theme === option.id" @click="$emit('theme', option.id)">
                <span class="theme-preview flex h-[64px] gap-[8px] rounded-[4px] bg-[image:var(--preview-bg)] mb-[4px] overflow-hidden [&_i]:w-[24%] [&_i]:bg-[var(--preview-panel)] [&>span]:flex [&>span]:flex-col [&>span]:justify-center [&>span]:gap-[6px] [&>span]:flex-1 [&_b]:w-[65%] [&_b]:h-[3px] [&_b]:bg-[var(--preview-line)] [&_b]:rounded-[2px] [&_b:first-child]:w-[45%] [&_b:first-child]:bg-[var(--preview-accent)] data-[palette=daylight]:[--preview-bg:linear-gradient(#e9eaee,#e9eaee)] data-[palette=daylight]:[--preview-panel:#fff] data-[palette=daylight]:[--preview-line:#6c707e] data-[palette=daylight]:[--preview-accent:#3574f0] data-[palette=midnight]:[--preview-bg:linear-gradient(#191c22,#191c22)] data-[palette=midnight]:[--preview-panel:#282e38] data-[palette=midnight]:[--preview-line:#95a1b4] data-[palette=midnight]:[--preview-accent:#8ab4ff] data-[palette=system]:[--preview-bg:linear-gradient(90deg,#e9eaee_50%,#191c22_50%)] data-[palette=system]:[--preview-panel:#bac4d3] data-[palette=system]:[--preview-line:#8b97ab] data-[palette=system]:[--preview-accent:#3574f0]" :data-palette="option.id" aria-hidden="true"><i /><span><b /><b /><b /></span></span>
                <strong>{{ option.name }}</strong><small>{{ option.description }}</small>
              </AppButton>
            </div>
            <p>Applied immediately across the workspace and remembered on this device.</p>
            <p v-if="error" role="alert">{{ error }}</p>
          </TabsContent>
          <TabsContent value="t3" class="t3-settings grid gap-6">
            <header class="flex flex-wrap items-center justify-between gap-3">
              <h3 class="mb-0!">T3 Code</h3>
              <span role="status" class="inline-flex items-center gap-1.5 text-xs font-medium"
                :class="t3Connected ? 'text-[var(--success)]' : 'text-[var(--muted)]'">
                <Check v-if="t3Connected" :size="14" aria-hidden="true" />{{ t3Connected ? 'Connected to T3 Code.' : 'Not connected.' }}
              </span>
            </header>
            <fieldset class="min-w-0">
              <legend class="mb-2 text-[13px] font-semibold">Chat preferences</legend>
              <div class="preference-list overflow-hidden rounded-md border border-[var(--line)]">
                <label>Sync active thread with T3 Code
                  <input role="switch" type="checkbox" :checked="syncEnabled" :disabled="!t3Connected && !syncEnabled"
                    @change="$emit('sync', ($event.target as HTMLInputElement).checked)" />
                </label>
                <label>Hide snoozed chats until they wake
                  <input v-model="hideSnoozed" role="switch" type="checkbox" />
                </label>
                <label>Automatically close settled chat tabs
                  <input v-model="autoCloseSettled" role="switch" type="checkbox" />
                </label>
              </div>
              <p v-if="syncError" role="alert">{{ syncError }}</p>
            </fieldset>
            <form class="grid gap-4 border-t border-[var(--line)] pt-5" @submit.prevent="pair">
              <div>
                <h4 class="mb-1 text-[13px] font-semibold">{{ t3Connected ? 'Update connection' : 'Connect your workspace' }}</h4>
                <p id="t3-pairing-help">Paste the pairing link or code from T3 Code’s connection settings.</p>
              </div>
              <div class="grid gap-3 min-[760px]:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
                <label>Local server<AppInput v-model="server" :disabled="pairingBusy" placeholder="http://127.0.0.1:3773" spellcheck="false" /></label>
                <label>Pairing link or code<AppInput v-model="pairing" :disabled="pairingBusy" type="password" autocomplete="off" placeholder="Paste pairing link or code" aria-describedby="t3-pairing-help" /></label>
              </div>
              <p v-if="pairingError || t3Error" role="alert" class="rounded-[6px] border border-[var(--danger)] bg-[var(--danger-bg)] px-[10px] py-[8px] break-words">{{ pairingError || t3Error }}</p>
              <div class="flex justify-end pt-[2px]">
                <AppButton type="submit" variant="primary" class="text-[12px] px-[14px] min-h-[34px]" :disabled="pairingBusy || !pairing.trim() || !pairT3"><LoaderCircle v-if="pairingBusy" :size="14" class="animate-spin" aria-hidden="true" /><Link2 v-else :size="14" aria-hidden="true" />{{ pairingBusy ? 'Connecting…' : 'Pair with T3 Code' }}</AppButton>
              </div>
            </form>
          </TabsContent>
          <TabsContent value="shortcuts">
            <h3>Keyboard shortcuts</h3>
            <dl class="settings-shortcuts m-0 [&>div]:flex [&>div]:justify-between [&>div]:items-center [&>div]:gap-[12px] [&>div]:py-[12px] [&>div]:border-b [&>div]:border-[var(--line)] [&>div]:text-[12px] [&_dd]:m-0 [&_dd]:shrink-0 [&_kbd]:py-[4px] [&_kbd]:px-[6px] [&_kbd]:border [&_kbd]:border-[var(--line)] [&_kbd]:rounded-[4px] [&_kbd]:bg-[var(--raised)] [&_kbd]:text-[var(--fg-secondary)] [&_kbd]:text-[11px] [@media(max-width:650px)]:[&>div]:flex-wrap">
              <div><dt>Open repository</dt><dd><kbd>Ctrl + O</kbd></dd></div>
              <div><dt>Refresh repository</dt><dd><kbd>Ctrl + R</kbd></dd></div>
              <div><dt>Settings</dt><dd><kbd>Ctrl / ⌘ + ,</kbd></dd></div>
              <div><dt>Commit from the message field</dt><dd><kbd>Ctrl / ⌘ + Enter</kbd></dd></div>
              <div><dt>Close dialog</dt><dd><kbd>Esc</kbd></dd></div>
            </dl>
          </TabsContent>
        </div>
      </TabsRoot>
    </AppDialogPanel>
  </AppDialog>
</template>

<style scoped>
.t3-settings h3,
.t3-settings p,
.t3-settings label {
  margin: 0;
}
.t3-settings label {
  gap: 6px;
  font-size: 12px;
}
.preference-list label {
  flex-direction: row;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  min-height: 42px;
  color: var(--fg);
  cursor: pointer;
}
.preference-list label + label { border-top: 1px solid var(--line); }
.preference-list label:hover { background: var(--hover); }
.preference-list label:has(:disabled) { color: var(--muted); cursor: default; }
.preference-list input[role="switch"] {
  appearance: none;
  flex: none;
  width: 34px;
  height: 20px;
  margin-inline-start: auto;
  padding: 2px;
  border: 1px solid var(--muted);
  border-radius: 20px;
  background: var(--bg);
  color: var(--fg-secondary);
  cursor: inherit;
}
.preference-list input::before {
  content: "";
  display: block;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: currentColor;
}
.preference-list input[role="switch"]:checked {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--on-accent);
}
.preference-list input:checked::before { transform: translateX(14px); }
.preference-list input:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
.preference-list input:disabled { opacity: .5; }
@media (prefers-reduced-motion: no-preference) {
  .preference-list input::before { transition: transform 120ms ease-out; }
}
@media (forced-colors: active) {
  .preference-list input[role="switch"] { appearance: auto; }
  .preference-list input::before { content: none; }
}
</style>
