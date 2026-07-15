<script setup lang="ts">
import { reactive, ref } from "vue";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { t } from "../i18n";
import type { AppSettings } from "../types";
import { errorMessage, passwordStrength } from "../utils/security";
import IconGlyph from "./IconGlyph.vue";

const props = defineProps<{ settings: AppSettings }>();
const emit = defineEmits<{
  close: [];
  saveSettings: [settings: AppSettings];
  imported: [];
  notify: [message: string, tone: "success" | "error"];
}>();

const tab = ref<"general" | "security" | "backup">("general");
const form = reactive<AppSettings>({ ...props.settings });
const currentPassword = ref("");
const newPassword = ref("");
const confirmPassword = ref("");
const backupPassword = ref("");
const backupConfirm = ref("");
const busy = ref(false);

async function run(action: () => Promise<void>) {
  busy.value = true;
  try {
    await action();
  } catch (error) {
    emit("notify", errorMessage(error), "error");
  } finally {
    busy.value = false;
  }
}

function submitSettings() {
  emit("saveSettings", { ...form });
}

async function changePassword() {
  if (!currentPassword.value || !newPassword.value) {
    emit("notify", t("请填写当前主密码和新主密码"), "error");
    return;
  }
  if (newPassword.value !== confirmPassword.value) {
    emit("notify", t("两次输入的新主密码不一致"), "error");
    return;
  }
  const strength = passwordStrength(newPassword.value);
  if (strength.score < 2 && !window.confirm(t("新主密码较弱，仍然继续吗？"))) return;
  await run(async () => {
    await api.changeMasterPassword(currentPassword.value, newPassword.value);
    currentPassword.value = "";
    newPassword.value = "";
    confirmPassword.value = "";
    emit("notify", t("主密码已更换，所有条目已重新加密"), "success");
  });
}

async function exportBackup() {
  if (!backupPassword.value || backupPassword.value !== backupConfirm.value) {
    emit("notify", t("请输入并确认备份密码"), "error");
    return;
  }
  const path = await saveDialog({
    defaultPath: `keychains-${new Date().toISOString().slice(0, 10)}.kcbak`,
    filters: [{ name: t("Keychains 加密备份"), extensions: ["kcbak"] }]
  });
  if (!path) return;
  await run(async () => {
    const result = await api.exportBackup(path, backupPassword.value);
    backupPassword.value = "";
    backupConfirm.value = "";
    emit("notify", t("已加密备份 {count} 个条目", { count: result.entryCount }), "success");
  });
}

async function importBackup(mode: "merge" | "replace") {
  if (!backupPassword.value) {
    emit("notify", t("请输入备份密码"), "error");
    return;
  }
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: t("Keychains 加密备份"), extensions: ["kcbak"] }]
  });
  if (typeof path !== "string") return;
  await run(async () => {
    const preview = await api.inspectBackup(path, backupPassword.value);
    const message = mode === "replace"
      ? t("备份包含 {count} 个条目。整体恢复会清除当前密码库，确定继续吗？", { count: preview.entryCount })
      : t("备份包含 {count} 个条目。确定合并到当前密码库吗？", { count: preview.entryCount });
    if (!window.confirm(message)) return;
    const result = await api.importBackup(path, backupPassword.value, mode);
    backupPassword.value = "";
    backupConfirm.value = "";
    emit("notify", t("已导入 {imported} 个条目，跳过 {skipped} 个", {
      imported: result.imported,
      skipped: result.skipped
    }), "success");
    emit("imported");
  });
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="emit('close')">
    <section class="modal settings-modal" role="dialog" aria-modal="true">
      <aside class="settings-nav">
        <div class="settings-title">
          <h2>{{ t("设置") }}</h2>
        </div>
        <button :class="{ active: tab === 'general' }" @click="tab = 'general'">{{ t("常规与外观") }}</button>
        <button :class="{ active: tab === 'security' }" @click="tab = 'security'">{{ t("安全") }}</button>
        <button :class="{ active: tab === 'backup' }" @click="tab = 'backup'">{{ t("备份与恢复") }}</button>
      </aside>

      <div class="settings-content">
        <header class="modal-header compact-header">
          <div>
            <h2>{{ t(tab === "general" ? "常规与外观" : tab === "security" ? "安全" : "备份与恢复") }}</h2>
          </div>
          <button class="icon-button" :aria-label="t('关闭')" @click="emit('close')"><IconGlyph name="close" :size="18" /></button>
        </header>

        <section v-if="tab === 'general'" class="settings-section">
          <div class="setting-row vertical">
            <div><b>{{ t("界面主题") }}</b><p>{{ t("默认跟随操作系统，也可以固定为浅色或深色。") }}</p></div>
            <select v-model="form.theme" class="theme-select" :aria-label="t('界面主题')">
              <option value="system">{{ t("跟随系统") }}</option>
              <option value="light">{{ t("浅色") }}</option>
              <option value="dark">{{ t("深色") }}</option>
            </select>
          </div>
          <div class="setting-row vertical">
            <div><b>{{ t("显示语言") }}</b><p>{{ t("默认跟随系统语言，也可以固定为中文或英文。") }}</p></div>
            <select v-model="form.language" class="theme-select" :aria-label="t('显示语言')">
              <option value="system">{{ t("系统语言") }}</option>
              <option value="zh-CN">{{ t("中文") }}</option>
              <option value="en">English</option>
            </select>
          </div>
          <button class="primary settings-save" @click="submitSettings">{{ t("保存设置") }}</button>
        </section>

        <section v-else-if="tab === 'security'" class="settings-section">
          <div class="setting-row">
            <div><b>{{ t("自动锁定") }}</b><p>{{ t("没有键盘或鼠标活动时重新要求主密码。") }}</p></div>
            <div class="number-input"><input v-model.number="form.autoLockMinutes" type="number" min="1" max="120" /><span>{{ t("分钟") }}</span></div>
          </div>
          <div class="setting-row">
            <div><b>{{ t("清除剪贴板") }}</b><p>{{ t("仅在内容仍是本应用复制的密码时清除。") }}</p></div>
            <div class="number-input"><input v-model.number="form.clipboardClearSeconds" type="number" min="5" max="300" /><span>{{ t("秒") }}</span></div>
          </div>
          <button class="primary settings-save" @click="submitSettings">{{ t("保存安全设置") }}</button>

          <div class="divider" />
          <h3>{{ t("更换主密码") }}</h3>
          <p class="muted">{{ t("更换时会在事务中重新加密所有条目。主密码仍然无法找回。") }}</p>
          <div class="form-grid">
            <label>{{ t("当前主密码") }}<input v-model="currentPassword" type="password" autocomplete="current-password" /></label>
            <label>{{ t("新主密码") }}<input v-model="newPassword" type="password" autocomplete="new-password" /></label>
            <label>{{ t("确认新主密码") }}<input v-model="confirmPassword" type="password" autocomplete="new-password" /></label>
          </div>
          <button class="secondary change-password-button" :disabled="busy" @click="changePassword">{{ t("更换主密码") }}</button>
        </section>

        <section v-else class="settings-section">
          <div class="backup-callout">
            <b>{{ t("备份文件始终加密") }}</b>
            <p>{{ t("备份密码独立于本地主密码。忘记备份密码同样无法恢复内容。") }}</p>
          </div>
          <label>{{ t("备份密码") }}</label>
          <input v-model="backupPassword" type="password" autocomplete="new-password" />
          <label>{{ t("确认备份密码（导出时需要）") }}</label>
          <input v-model="backupConfirm" type="password" autocomplete="new-password" />
          <div class="backup-actions">
            <button class="primary" :disabled="busy" @click="exportBackup">{{ t("导出加密备份") }}</button>
            <button class="secondary" :disabled="busy" @click="importBackup('merge')">{{ t("合并导入") }}</button>
            <button class="danger-button" :disabled="busy" @click="importBackup('replace')">{{ t("整体恢复") }}</button>
          </div>
        </section>
      </div>
    </section>
  </div>
</template>
