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
  <main class="grid size-full place-items-center overflow-hidden bg-window p-5">
    <section class="max-h-[calc(100vh-28px)] w-[min(500px,100%)] overflow-y-auto rounded-ui-panel border border-line bg-surface px-[38px] pt-8 pb-[30px] shadow-ui-card [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <h1 class="mb-2 text-center text-2xl font-[680]">{{ t("欢迎使用 Keychains") }}</h1>
      <p class="mx-auto mt-0 mb-5 text-center text-[12.5px] leading-[1.55] text-muted">{{ t("账号与密码只保存在这台设备上，并使用你的主密码加密。") }}</p>

      <div class="mb-[18px] grid grid-cols-2 gap-0.5 rounded-[10px] bg-control p-[3px]">
        <button class="min-h-[31px] rounded-[7px] border-0 bg-transparent px-2 text-xs font-semibold text-muted" :class="{ 'bg-surface text-foreground shadow-[0_1px_3px_rgba(24,24,21,0.12)]': mode === 'create' }" @click="mode = 'create'">{{ t("创建新密码库") }}</button>
        <button class="min-h-[31px] rounded-[7px] border-0 bg-transparent px-2 text-xs font-semibold text-muted" :class="{ 'bg-surface text-foreground shadow-[0_1px_3px_rgba(24,24,21,0.12)]': mode === 'restore' }" @click="mode = 'restore'">{{ t("从备份恢复") }}</button>
      </div>

      <form @submit.prevent="submit">
        <template v-if="mode === 'restore'">
          <label>{{ t("加密备份") }}</label>
          <button class="flex min-h-[39px] w-full items-center justify-between gap-3 rounded-[9px] border border-line-strong bg-surface px-[11px] text-muted" type="button" @click="chooseBackup">
            <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{{ backupPath ? backupPath.split(/[\\/]/).pop() : t("选择 .kcbak 文件") }}</span>
            <b class="flex-none text-foreground">{{ t("浏览") }}</b>
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
        <div class="mt-2 flex items-center gap-[9px] text-[10.5px] text-muted">
          <div class="h-[3px] flex-1 overflow-hidden rounded-[99px] bg-line"><i class="block h-full rounded-[inherit] bg-[linear-gradient(90deg,#d85f66,#dbac49_48%,#35a978)]" :style="{ width: `${strength.score * 25}%` }" /></div>
          <span>{{ t("强度：{strength}", { strength: t(strength.label) }) }}</span>
        </div>

        <label for="confirm-password">{{ t("确认主密码") }}</label>
        <input id="confirm-password" v-model="confirmPassword" type="password" autocomplete="new-password" />
        <p class="my-4 rounded-lg bg-control px-[11px] py-2.5 text-[11.5px] leading-[1.55] text-muted">{{ t("主密码遗失后无法找回。请选择你能记住、但他人难以猜到的密码。") }}</p>
        <p v-if="localError || error" class="my-3 rounded-lg bg-danger-soft px-[11px] py-[9px] text-[11.5px] leading-[1.45] text-danger">{{ localError || error }}</p>
        <button class="mt-[13px] min-h-[var(--control-height)] w-full rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" type="submit" :disabled="busy">
          {{ t(busy ? "正在准备密码库…" : weakConfirmed ? "确认使用弱密码" : mode === "restore" ? "恢复密码库" : "创建密码库") }}
        </button>
      </form>
    </section>
  </main>
</template>
