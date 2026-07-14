<script setup lang="ts">
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { passwordStrength } from "../utils/security";

defineProps<{ busy: boolean; error: string }>();
const emit = defineEmits<{
  create: [password: string];
  restore: [path: string, backupPassword: string, masterPassword: string];
}>();

const mode = ref<"create" | "restore">("create");
const password = ref("");
const confirmPassword = ref("");
const backupPassword = ref("");
const backupPath = ref("");
const localError = ref("");
const weakConfirmed = ref(false);
const strength = computed(() => passwordStrength(password.value));

async function chooseBackup() {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: t("Keychains 加密备份"), extensions: ["kcbak"] }]
  });
  if (typeof selected === "string") backupPath.value = selected;
}

function submit() {
  localError.value = "";
  if (!password.value) {
    localError.value = t("主密码不能为空");
    return;
  }
  if (password.value !== confirmPassword.value) {
    localError.value = t("两次输入的主密码不一致");
    return;
  }
  if (strength.value.score < 2 && !weakConfirmed.value) {
    weakConfirmed.value = true;
    localError.value = t("这个主密码较弱。Keychains 不会阻止使用；请再次点击确认。");
    return;
  }
  if (mode.value === "restore") {
    if (!backupPath.value || !backupPassword.value) {
      localError.value = t("请选择备份文件并输入备份密码");
      return;
    }
    emit("restore", backupPath.value, backupPassword.value, password.value);
  } else {
    emit("create", password.value);
  }
}
</script>

<template>
  <main class="auth-page">
    <section class="auth-card setup-card">
      <h1>{{ t("欢迎使用 Keychains") }}</h1>
      <p class="auth-lead">{{ t("账号与密码只保存在这台设备上，并使用你的主密码加密。") }}</p>

      <div class="segment-control">
        <button :class="{ active: mode === 'create' }" @click="mode = 'create'">{{ t("创建新密码库") }}</button>
        <button :class="{ active: mode === 'restore' }" @click="mode = 'restore'">{{ t("从备份恢复") }}</button>
      </div>

      <form @submit.prevent="submit">
        <template v-if="mode === 'restore'">
          <label>{{ t("加密备份") }}</label>
          <button class="file-picker" type="button" @click="chooseBackup">
            <span>{{ backupPath ? backupPath.split(/[\\/]/).pop() : t("选择 .kcbak 文件") }}</span>
            <b>{{ t("浏览") }}</b>
          </button>
          <label for="backup-password">{{ t("备份密码") }}</label>
          <input id="backup-password" v-model="backupPassword" type="password" autocomplete="off" />
        </template>

        <label for="master-password">{{ t(mode === "restore" ? "新的本地主密码" : "主密码") }}</label>
        <input
          id="master-password"
          v-model="password"
          type="password"
          autocomplete="new-password"
          autofocus
          @input="weakConfirmed = false"
        />
        <div class="strength-row">
          <div class="strength-track"><i :style="{ width: `${strength.score * 25}%` }" /></div>
          <span>{{ t("强度：{strength}", { strength: t(strength.label) }) }}</span>
        </div>

        <label for="confirm-password">{{ t("确认主密码") }}</label>
        <input id="confirm-password" v-model="confirmPassword" type="password" autocomplete="new-password" />
        <p class="security-note">{{ t("主密码遗失后无法找回。请选择你能记住、但他人难以猜到的密码。") }}</p>
        <p v-if="localError || error" class="form-error">{{ localError || error }}</p>
        <button class="primary wide" type="submit" :disabled="busy">
          {{ t(busy ? "正在准备密码库…" : weakConfirmed ? "确认使用弱密码" : mode === "restore" ? "恢复密码库" : "创建密码库") }}
        </button>
      </form>
    </section>
  </main>
</template>
