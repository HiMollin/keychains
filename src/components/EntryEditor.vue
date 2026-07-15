<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { api } from "../api";
import { t } from "../i18n";
import type { Entry, EntryInput, EntryType, PasswordOptions } from "../types";
import { errorMessage, passwordStrength } from "../utils/security";
import IconGlyph from "./IconGlyph.vue";

const props = defineProps<{ entry: Entry | null; entryTypes: EntryType[] }>();
const emit = defineEmits<{ save: [input: EntryInput, id?: string]; cancel: [] }>();

const blank = (): EntryInput => ({
  typeId: props.entryTypes[0]?.id ?? "",
  name: "",
  url: "",
  username: "",
  password: "",
  notes: "",
  tags: [],
  favorite: false,
  customFields: []
});

const form = reactive<EntryInput>(blank());
const tagsText = ref("");
const showPassword = ref(false);
const showGenerator = ref(false);
const localError = ref("");
const generating = ref(false);
const generator = reactive<PasswordOptions>({
  length: 20,
  uppercase: true,
  lowercase: true,
  digits: true,
  symbols: true
});
const strength = computed(() => passwordStrength(form.password));

watch(
  () => props.entry,
  (entry) => {
    Object.assign(form, entry ? {
      typeId: entry.typeId,
      name: entry.name,
      url: entry.url,
      username: entry.username,
      password: entry.password,
      notes: entry.notes,
      tags: [...entry.tags],
      favorite: entry.favorite,
      customFields: entry.customFields.map((field) => ({ ...field }))
    } : blank());
    tagsText.value = entry?.tags.join(", ") ?? "";
  },
  { immediate: true }
);

function addCustomField() {
  form.customFields.push({ id: crypto.randomUUID(), label: "", value: "", secret: false });
}

async function generate() {
  generating.value = true;
  localError.value = "";
  try {
    form.password = await api.generatePassword({ ...generator });
    showPassword.value = true;
  } catch (error) {
    localError.value = errorMessage(error);
  } finally {
    generating.value = false;
  }
}

function submit() {
  localError.value = "";
  if (!form.typeId || !props.entryTypes.some((item) => item.id === form.typeId)) {
    localError.value = t("请先选择一个有效类型");
    return;
  }
  if (!form.name.trim()) {
    localError.value = t("请输入名称");
    return;
  }
  form.tags = tagsText.value.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean);
  emit("save", {
    ...form,
    tags: [...form.tags],
    customFields: form.customFields.map((field) => ({ ...field }))
  }, props.entry?.id);
}
</script>

<template>
  <div class="fixed inset-0 z-60 grid place-items-center bg-[rgba(12,12,11,0.38)] p-6 backdrop-blur-[3px]" @mousedown.self="emit('cancel')">
    <section class="max-h-[calc(100vh-48px)] w-[min(680px,100%)] overflow-hidden rounded-ui-panel border border-line-strong bg-surface shadow-dialog" role="dialog" aria-modal="true">
      <header class="flex items-center justify-between border-b border-line bg-surface px-[21px] pt-[18px] pb-[15px]">
        <div>
          <p class="mb-[5px] text-[10px] font-semibold tracking-[0.02em] text-muted">{{ t(entry ? "编辑条目" : "新建条目") }}</p>
          <h2 class="m-0 text-[17px] font-[660]">{{ entry ? entry.name : t("保存一个账号") }}</h2>
        </div>
        <button class="grid size-8 place-items-center rounded-ui-control border-0 bg-transparent p-0 text-[22px] text-muted hover:bg-hover hover:text-foreground" :aria-label="t('关闭')" @click="emit('cancel')"><IconGlyph name="close" :size="18" /></button>
      </header>

      <form class="max-h-[calc(100vh-150px)] overflow-y-auto px-[22px] pt-1 pb-[22px] [scrollbar-color:color-mix(in_srgb,var(--muted)_40%,transparent)_transparent] [scrollbar-width:thin]" @submit.prevent="submit">
        <div class="grid grid-cols-[145px_1fr_1fr] gap-3 max-[760px]:grid-cols-1 [&_label]:mt-2">
          <div>
            <label>{{ t("类型") }}</label>
            <select v-model="form.typeId">
              <option v-for="item in entryTypes" :key="item.id" :value="item.id">{{ item.name }}</option>
            </select>
          </div>
          <div class="col-span-2 max-[760px]:col-auto">
            <label>{{ t("名称 *") }}</label>
            <input v-model="form.name" autofocus :placeholder="t('例如 GitHub')" />
          </div>
        </div>

        <label>{{ t("网址") }}</label>
        <input v-model="form.url" inputmode="url" :placeholder="t('https://example.com（可选）')" />
        <label>{{ t("用户名或邮箱") }}</label>
        <input v-model="form.username" autocomplete="off" placeholder="name@example.com" />
        <label>{{ t("密码") }}</label>
        <div class="grid grid-cols-[minmax(0,1fr)_auto_auto] items-stretch gap-[7px] [&_button]:min-w-[58px] [&_button]:rounded-ui-control [&_button]:border [&_button]:border-line-strong [&_button]:bg-control [&_button]:px-2.5 [&_button]:text-[11px] [&_button]:font-semibold [&_button]:whitespace-nowrap [&_button:hover]:bg-hover">
          <input v-model="form.password" :type="showPassword ? 'text' : 'password'" autocomplete="new-password" />
          <button type="button" @click="showPassword = !showPassword">{{ t(showPassword ? "隐藏" : "显示") }}</button>
          <button type="button" @click="showGenerator = !showGenerator">{{ t("生成") }}</button>
        </div>
        <div class="mt-[7px] flex items-center gap-[9px] text-[10.5px] text-muted">
          <div class="h-[3px] flex-1 overflow-hidden rounded-[99px] bg-line"><i class="block h-full rounded-[inherit] bg-[linear-gradient(90deg,#d85f66,#dbac49_48%,#35a978)]" :style="{ width: `${strength.score * 25}%` }" /></div>
          <span>{{ t(strength.label) }}</span>
        </div>

        <section v-if="showGenerator" class="mt-3 rounded-[10px] border border-line bg-control p-3">
          <div class="grid grid-cols-[80px_1fr] items-center gap-3">
            <label class="m-0">{{ t("长度") }} <b>{{ generator.length }}</b></label>
            <input v-model.number="generator.length" class="p-0" type="range" min="12" max="64" />
          </div>
          <div class="mt-2.5 flex flex-wrap items-center gap-[13px] [&_label]:m-0 [&_label]:flex [&_label]:items-center [&_label]:gap-1.5 [&_label]:whitespace-nowrap">
            <label><input v-model="generator.uppercase" type="checkbox" /> {{ t("大写") }}</label>
            <label><input v-model="generator.lowercase" type="checkbox" /> {{ t("小写") }}</label>
            <label><input v-model="generator.digits" type="checkbox" /> {{ t("数字") }}</label>
            <label><input v-model="generator.symbols" type="checkbox" /> {{ t("符号") }}</label>
            <button class="ml-auto min-h-[30px] rounded-ui-control border border-line-strong bg-surface px-2.5 text-xs font-[620] disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:bg-control" type="button" :disabled="generating" @click="generate">{{ t("生成密码") }}</button>
          </div>
        </section>

        <label>{{ t("标签") }}</label>
        <input v-model="tagsText" :placeholder="t('工作, 邮箱（使用逗号分隔）')" />

        <div class="mt-[17px] flex items-center justify-between">
          <label class="m-0">{{ t("自定义字段") }}</label>
          <button class="border-0 bg-transparent p-[5px] font-semibold text-foreground" type="button" @click="addCustomField">{{ t("＋ 添加字段") }}</button>
        </div>
        <div v-for="(field, index) in form.customFields" :key="field.id" class="mt-[7px] grid grid-cols-[0.75fr_1fr_auto_34px] items-center gap-[7px] [&_input]:min-w-0">
          <input v-model="field.label" :placeholder="t('字段名称')" />
          <input v-model="field.value" :type="field.secret ? 'password' : 'text'" :placeholder="t('字段内容')" />
          <label class="m-0 flex items-center gap-1.5 whitespace-nowrap"><input v-model="field.secret" type="checkbox" /> {{ t("敏感") }}</label>
          <button class="grid size-8 place-items-center rounded-ui-control border-0 bg-transparent p-0 text-[22px] text-danger hover:bg-hover" type="button" :aria-label="t('删除字段')" @click="form.customFields.splice(index, 1)"><IconGlyph name="close" :size="16" /></button>
        </div>

        <label>{{ t("备注") }}</label>
        <textarea v-model="form.notes" rows="4" :placeholder="t('仅在解锁后可见')" />
        <label class="mt-4 mb-0 flex items-center gap-1.5 whitespace-nowrap"><input v-model="form.favorite" type="checkbox" /> {{ t("添加到收藏") }}</label>

        <p v-if="localError" class="my-3 rounded-lg bg-danger-soft px-[11px] py-[9px] text-[11.5px] leading-[1.45] text-danger">{{ localError }}</p>
        <footer class="mt-[22px] flex justify-end gap-[9px] border-t border-line pt-[18px]">
          <button class="min-h-[var(--button-height)] rounded-ui-control border border-line-strong bg-surface px-3.5 text-xs font-[620] disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:bg-control" type="button" @click="emit('cancel')">{{ t("取消") }}</button>
          <button class="min-h-[var(--button-height)] rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" type="submit">{{ t(entry ? "保存更改" : "保存条目") }}</button>
        </footer>
      </form>
    </section>
  </div>
</template>
