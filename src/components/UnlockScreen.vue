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
  <main class="locked-workspace">
    <aside class="locked-sidebar-surface" aria-hidden="true" />
    <div class="locked-workspace-divider" aria-hidden="true" />
    <section class="locked-main-surface" aria-hidden="true" />

    <div class="unlock-overlay">
      <section class="unlock-dialog" role="dialog" aria-modal="true" aria-labelledby="unlock-title">
        <header class="unlock-dialog-header">
          <div>
            <p class="eyebrow">KEYCHAINS</p>
            <h1 id="unlock-title">{{ t("解锁密码库") }}</h1>
          </div>
        </header>
        <p class="unlock-lead">{{ t("输入主密码以继续，密码不会离开这台设备。") }}</p>
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
          <p v-if="error" class="form-error">{{ error }}</p>
          <button class="primary wide" type="submit" :disabled="busy || !password">
            {{ t(busy ? "正在解锁…" : "解锁密码库") }}
          </button>
        </form>
        <p class="unlock-hint">{{ t("闲置一段时间后会自动重新锁定。") }}</p>
      </section>
    </div>
  </main>
</template>
