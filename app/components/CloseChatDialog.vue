<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { CheckCheck, LoaderCircle, X } from "lucide-vue-next";
import { isThreadSettled } from "~/lib/threads";
import type { T3Thread } from "~/lib/types";

const props = defineProps<{
  thread: T3Thread;
  connected: boolean;
  confirm: (settle: boolean) => Promise<void>;
}>();
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
const settle = ref(true),
  submitting = ref(false),
  error = ref("");
const alreadySettled = computed(
  () => isThreadSettled(props.thread) || !!props.thread.archivedAt,
);
const willSettle = computed(() => settle.value && !alreadySettled.value);
let trigger: HTMLElement | null;
onMounted(() => {
  trigger = document.activeElement as HTMLElement | null;
});
onUnmounted(
  () =>
    void nextTick(() => {
      const target = trigger?.isConnected
        ? trigger
        : document.querySelector<HTMLElement>(
            '.chat-tabs [aria-selected="true"], .chat-tabs button[aria-label="New chat"]',
          );
      target?.focus();
    }),
);
function dismiss() {
  if (!submitting.value) emit("close");
}
async function submit() {
  if (submitting.value) return;
  submitting.value = true;
  error.value = "";
  try {
    await props.confirm(willSettle.value);
    emit("close");
  } catch (e) {
    error.value = String(e);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <AppDialog
    title="Close conversation tab?"
    :dismissible="!submitting"
    @close="dismiss"
    ><AppDialogPanel
      class="modal close-chat-dialog w-[470px] max-w-full max-h-[90vh] overflow-auto p-[22px] m-auto text-[var(--fg)] [&_.settle-choice]:flex [&_.settle-choice]:flex-row [&_.settle-choice]:items-center [&_.settle-choice]:gap-[12px] [&_.settle-choice]:p-[16px] [&_.settle-choice]:mt-[20px] [&_.settle-choice]:mx-0 [&_.settle-choice]:mb-[10px] [&_.settle-choice]:border-[1px] [&_.settle-choice]:border-solid [&_.settle-choice]:border-[var(--line)] [&_.settle-choice]:rounded-[8px] [&_.settle-choice]:bg-[var(--panel)] [&_.settle-choice]:cursor-pointer [&_.settle-choice_input]:w-[18px] [&_.settle-choice_input]:h-[18px] [&_.settle-choice_input]:m-0 [&_.settle-choice_input]:shrink-0"
      aria-labelledby="close-chat-title"
      aria-describedby="close-chat-description"
    >
      <form @submit.prevent="submit">
        <AppDialogHeader class="modal-heading">
          <h2
            class="[:where(&)]:m-0 [:where(&)]:text-[18px] [:where(&)]:font-semibold"
            id="close-chat-title"
          >
            Close conversation tab?
          </h2>
          <AppButton
            size="icon"
            type="button"
            class="icon-button"
            aria-label="Cancel closing tab"
            :disabled="submitting"
            @click="dismiss"
          >
            <X :size="18" />
          </AppButton>
        </AppDialogHeader>
        <p
          class="m-0 text-[12px] leading-[1.7] text-[var(--fg-secondary)] [overflow-wrap:anywhere] [&_strong]:text-[var(--fg)]"
          id="close-chat-description"
        >
          You’re closing <strong>{{ thread.title }}</strong
          >. You can reopen it from Chat history.
        </p>
        <p
          v-if="alreadySettled"
          class="settled-note mt-[20px] mx-0 mb-[10px] text-[12px] leading-[1.7] text-[var(--fg-secondary)] [overflow-wrap:anywhere] [&_strong]:text-[var(--fg)] flex items-center gap-[8px]"
        >
          <CheckCheck :size="18" />This conversation is already
          {{ thread.archivedAt ? "archived" : "settled" }}.
        </p>
        <label
          v-else
          class="settle-choice [&.enabled]:[border-color:var(--line)] [&.enabled]:bg-[var(--selection)] [&:has(input:focus-visible)]:[outline:2px_solid_var(--accent)] [&:has(input:focus-visible)]:outline-offset-[3px] [&>span:first-of-type]:grid [&>span:first-of-type]:gap-[5px] [&>span:first-of-type]:flex-1 [&_strong]:text-[13px] [&_strong]:text-[var(--fg)] [&_small]:leading-[1.6]"
          :class="{ enabled: settle }"
        >
          <AppInput
            v-model="settle"
            type="checkbox"
            :disabled="submitting"
            aria-label="Settle conversation"
            aria-describedby="settle-chat-description"
          />
          <span
            ><strong>Settle conversation</strong
            ><small class="[:where(&)]:text-[11px]" id="settle-chat-description"
              >Mark this conversation as settled in T3 Code when closing the
              tab.</small
            ></span
          >
          <span
            v-if="settle"
            class="settle-badge py-[3px] px-[7px] rounded-[4px] text-[var(--fg)] bg-[var(--selection)] text-[10px]"
            >Enabled</span
          >
        </label>
        <p
          class="close-chat-hint m-0 text-[11px] leading-[1.7] text-[var(--fg-secondary)] [overflow-wrap:anywhere] [&_strong]:text-[var(--fg)]"
        >
          {{
            willSettle
              ? "The conversation will be marked as settled before the tab closes."
              : "Only the tab will close. The conversation’s status will stay the same."
          }}
        </p>
        <p
          v-if="willSettle && !connected"
          class="inline-error text-[var(--fg-secondary)] bg-[var(--danger-bg)] py-[10px] px-[12px] text-[12px] leading-[1.7] whitespace-pre-wrap [overflow-wrap:anywhere] m-0 [&_strong]:text-[var(--fg)]"
        >
          Reconnect T3 Code to settle, or uncheck “Settle conversation” to close
          only the tab.
        </p>
        <div
          v-if="error"
          class="inline-error text-[var(--danger)] bg-[var(--danger-bg)] py-[10px] px-[12px] text-[11px] leading-[1.7] whitespace-pre-wrap [overflow-wrap:anywhere]"
          role="alert"
        >
          {{ error }}
        </div>
        <AppDialogFooter class="modal-footer">
          <AppButton
            type="button"
            autofocus
            :disabled="submitting"
            @click="dismiss"
          >
            Cancel
          </AppButton>
          <AppButton
            variant="primary"
            type="submit"
            class="primary [border-color:var(--line)]"
            :disabled="submitting || (willSettle && !connected)"
          >
            <LoaderCircle
              v-if="submitting"
              :size="14"
              class="spin animate-spin"
            />
            {{
              submitting
                ? "Closing…"
                : willSettle
                  ? "Settle & close tab"
                  : "Close tab"
            }}
          </AppButton>
        </AppDialogFooter>
      </form>
    </AppDialogPanel></AppDialog
  >
</template>
