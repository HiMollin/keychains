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
  <div class="fixed inset-0 z-60 grid place-items-center bg-[rgba(12,12,11,0.38)] p-6 backdrop-blur-[3px]" @mousedown.self="emit('close')">
    <section class="grid h-[min(600px,calc(100vh-48px))] max-h-[calc(100vh-48px)] w-[min(800px,100%)] grid-cols-[190px_minmax(0,1fr)] overflow-hidden rounded-ui-panel border border-line-strong bg-surface shadow-dialog max-[760px]:grid-cols-[160px_minmax(0,1fr)]" role="dialog" aria-modal="true">
      <aside class="border-r border-line bg-sidebar px-2.5 py-5">
        <div class="px-[11px] pb-5">
          <h2 class="m-0">{{ t("设置") }}</h2>
        </div>
        <button class="min-h-9 w-full rounded-lg border-0 bg-transparent px-2.5 text-left text-xs text-muted hover:bg-hover hover:text-foreground" :class="{ 'bg-selected font-[620] text-foreground': tab === 'general' }" @click="tab = 'general'">{{ t("常规与外观") }}</button>
        <button class="min-h-9 w-full rounded-lg border-0 bg-transparent px-2.5 text-left text-xs text-muted hover:bg-hover hover:text-foreground" :class="{ 'bg-selected font-[620] text-foreground': tab === 'security' }" @click="tab = 'security'">{{ t("安全") }}</button>
        <button class="min-h-9 w-full rounded-lg border-0 bg-transparent px-2.5 text-left text-xs text-muted hover:bg-hover hover:text-foreground" :class="{ 'bg-selected font-[620] text-foreground': tab === 'backup' }" @click="tab = 'backup'">{{ t("备份与恢复") }}</button>
      </aside>

      <div class="relative min-w-0 overflow-y-auto bg-surface [scrollbar-color:color-mix(in_srgb,var(--muted)_40%,transparent)_transparent] [scrollbar-width:thin]">
        <header class="sticky top-0 z-2 flex items-center justify-between border-b border-line bg-surface px-[21px] pt-[18px] pb-[15px]">
          <div>
            <h2 class="m-0 text-[17px] font-[660]">{{ t(tab === "general" ? "常规与外观" : tab === "security" ? "安全" : "备份与恢复") }}</h2>
          </div>
          <button class="grid size-8 place-items-center rounded-ui-control border-0 bg-transparent p-0 text-[22px] text-muted hover:bg-hover hover:text-foreground" :aria-label="t('关闭')" @click="emit('close')"><IconGlyph name="close" :size="18" /></button>
        </header>

        <section v-if="tab === 'general'" class="px-[25px] pt-2 pb-[70px]">
          <div class="border-b border-line py-[18px]">
            <div><b class="text-[13px]">{{ t("界面主题") }}</b><p class="mt-[5px] mb-0 text-[11px] text-muted">{{ t("默认跟随操作系统，也可以固定为浅色或深色。") }}</p></div>
            <select v-model="form.theme" class="mt-[15px] mb-1 min-h-10 w-[min(260px,100%)]" :aria-label="t('界面主题')">
              <option value="system">{{ t("跟随系统") }}</option>
              <option value="light">{{ t("浅色") }}</option>
              <option value="dark">{{ t("深色") }}</option>
            </select>
          </div>
          <div class="border-b border-line py-[18px]">
            <div><b class="text-[13px]">{{ t("显示语言") }}</b><p class="mt-[5px] mb-0 text-[11px] text-muted">{{ t("默认跟随系统语言，也可以固定为中文或英文。") }}</p></div>
            <select v-model="form.language" class="mt-[15px] mb-1 min-h-10 w-[min(260px,100%)]" :aria-label="t('显示语言')">
              <option value="system">{{ t("系统语言") }}</option>
              <option value="zh-CN">{{ t("中文") }}</option>
              <option value="en">English</option>
            </select>
          </div>
          <button class="mt-5 inline-flex min-h-[var(--button-height)] items-center justify-center rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" @click="submitSettings">{{ t("保存设置") }}</button>
        </section>

        <section v-else-if="tab === 'security'" class="px-[25px] pt-2 pb-[70px]">
          <div class="flex items-center justify-between gap-[22px] border-b border-line py-[18px]">
            <div><b class="text-[13px]">{{ t("自动锁定") }}</b><p class="mt-[5px] mb-0 text-[11px] text-muted">{{ t("没有键盘或鼠标活动时重新要求主密码。") }}</p></div>
            <div class="flex flex-none items-center gap-[7px]"><input v-model.number="form.autoLockMinutes" class="w-[74px]" type="number" min="1" max="120" /><span class="text-[11px] text-muted">{{ t("分钟") }}</span></div>
          </div>
          <div class="flex items-center justify-between gap-[22px] border-b border-line py-[18px]">
            <div><b class="text-[13px]">{{ t("清除剪贴板") }}</b><p class="mt-[5px] mb-0 text-[11px] text-muted">{{ t("仅在内容仍是本应用复制的密码时清除。") }}</p></div>
            <div class="flex flex-none items-center gap-[7px]"><input v-model.number="form.clipboardClearSeconds" class="w-[74px]" type="number" min="5" max="300" /><span class="text-[11px] text-muted">{{ t("秒") }}</span></div>
          </div>
          <button class="mt-5 inline-flex min-h-[var(--button-height)] items-center justify-center rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" @click="submitSettings">{{ t("保存安全设置") }}</button>

          <div class="mt-[34px] mb-7 h-px bg-line" />
          <h3>{{ t("更换主密码") }}</h3>
          <p class="leading-[1.6] text-muted">{{ t("更换时会在事务中重新加密所有条目。主密码仍然无法找回。") }}</p>
          <div class="grid gap-3 [&_label]:mt-2 [&_label_input]:mt-[7px]">
            <label>{{ t("当前主密码") }}<input v-model="currentPassword" type="password" autocomplete="current-password" /></label>
            <label>{{ t("新主密码") }}<input v-model="newPassword" type="password" autocomplete="new-password" /></label>
            <label>{{ t("确认新主密码") }}<input v-model="confirmPassword" type="password" autocomplete="new-password" /></label>
          </div>
          <button class="mt-[18px] min-h-[var(--button-height)] rounded-ui-control border border-line-strong bg-surface px-3.5 text-xs font-[620] disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:bg-control" :disabled="busy" @click="changePassword">{{ t("更换主密码") }}</button>
        </section>

        <section v-else class="px-[25px] pt-2 pb-[70px]">
          <div class="rounded-[10px] border border-line bg-control p-3.5">
            <b>{{ t("备份文件始终加密") }}</b>
            <p class="mt-1.5 mb-0 text-[11px] leading-normal text-muted">{{ t("备份密码独立于本地主密码。忘记备份密码同样无法恢复内容。") }}</p>
          </div>
          <label>{{ t("备份密码") }}</label>
          <input v-model="backupPassword" type="password" autocomplete="new-password" />
          <label>{{ t("确认备份密码（导出时需要）") }}</label>
          <input v-model="backupConfirm" type="password" autocomplete="new-password" />
          <div class="mt-5 flex flex-wrap gap-2">
            <button class="min-h-[var(--button-height)] rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" :disabled="busy" @click="exportBackup">{{ t("导出加密备份") }}</button>
            <button class="min-h-[var(--button-height)] rounded-ui-control border border-line-strong bg-surface px-3.5 text-xs font-[620] disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:bg-control" :disabled="busy" @click="importBackup('merge')">{{ t("合并导入") }}</button>
            <button class="min-h-[var(--button-height)] rounded-ui-control border bg-danger-soft px-3.5 text-xs font-[620] text-danger disabled:cursor-not-allowed disabled:opacity-[0.52]" style="border-color: color-mix(in srgb, var(--danger) 22%, var(--border))" :disabled="busy" @click="importBackup('replace')">{{ t("整体恢复") }}</button>
          </div>
        </section>
      </div>
    </section>
  </div>
</template>
