<script setup lang="ts">
import { ref } from "vue";
import { t } from "../i18n";

defineProps<{ busy: boolean; error: string }>();
const emit = defineEmits<{ unlock: [password: string] }>();
const password = ref("");

function submit() {
  if (password.value) emit("unlock", password.value);
}
</script>

<template>
  <main class="locked-workspace relative grid size-full overflow-hidden bg-window">
    <aside class="bg-sidebar" aria-hidden="true" />
    <div class="locked-workspace-divider relative bg-sidebar" aria-hidden="true" />
    <section class="bg-window" aria-hidden="true" />

    <div class="absolute inset-0 grid place-items-center bg-[color-mix(in_srgb,var(--window)_42%,transparent)] p-6">
      <section class="w-[min(390px,100%)] rounded-ui-panel border border-line-strong bg-surface px-[27px] pt-[25px] pb-[23px] shadow-dialog" role="dialog" aria-modal="true" aria-labelledby="unlock-title">
        <header class="flex items-center">
          <div>
            <p class="mb-[3px] text-[9px] font-semibold tracking-[0.02em] text-muted">KEYCHAINS</p>
            <h1 id="unlock-title" class="m-0 text-[19px] font-[680]">{{ t("解锁密码库") }}</h1>
          </div>
        </header>
        <p class="mt-[18px] mb-1 text-xs leading-[1.55] text-muted">{{ t("输入主密码以继续，密码不会离开这台设备。") }}</p>
        <form @submit.prevent="submit">
          <label for="unlock-password">{{ t("主密码") }}</label>
          <input
            id="unlock-password"
            v-model="password"
            type="password"
            autocomplete="current-password"
            autofocus
            :placeholder="t('输入主密码')"
          />
          <p v-if="error" class="my-3 rounded-lg bg-danger-soft px-[11px] py-[9px] text-[11.5px] leading-[1.45] text-danger">{{ error }}</p>
          <button class="mt-[13px] min-h-[var(--control-height)] w-full rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" type="submit" :disabled="busy || !password">
            {{ t(busy ? "正在解锁…" : "解锁密码库") }}
          </button>
        </form>
        <p class="mt-[15px] mb-0 text-center text-[10.5px] text-faint">{{ t("闲置一段时间后会自动重新锁定。") }}</p>
      </section>
    </div>
  </main>
</template>
