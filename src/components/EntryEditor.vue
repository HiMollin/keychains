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
  <div class="modal-backdrop" @mousedown.self="emit('cancel')">
    <section class="modal editor-modal" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <p class="eyebrow">{{ t(entry ? "编辑条目" : "新建条目") }}</p>
          <h2>{{ entry ? entry.name : t("保存一个账号") }}</h2>
        </div>
        <button class="icon-button" :aria-label="t('关闭')" @click="emit('cancel')"><IconGlyph name="close" :size="18" /></button>
      </header>

      <form class="editor-form" @submit.prevent="submit">
        <div class="form-grid three">
          <div>
            <label>{{ t("类型") }}</label>
            <select v-model="form.typeId">
              <option v-for="item in entryTypes" :key="item.id" :value="item.id">{{ item.name }}</option>
            </select>
          </div>
          <div class="span-2">
            <label>{{ t("名称 *") }}</label>
            <input v-model="form.name" autofocus :placeholder="t('例如 GitHub')" />
          </div>
        </div>

        <label>{{ t("网址") }}</label>
        <input v-model="form.url" inputmode="url" :placeholder="t('https://example.com（可选）')" />
        <label>{{ t("用户名或邮箱") }}</label>
        <input v-model="form.username" autocomplete="off" placeholder="name@example.com" />
        <label>{{ t("密码") }}</label>
        <div class="input-actions">
          <input v-model="form.password" :type="showPassword ? 'text' : 'password'" autocomplete="new-password" />
          <button type="button" @click="showPassword = !showPassword">{{ t(showPassword ? "隐藏" : "显示") }}</button>
          <button type="button" @click="showGenerator = !showGenerator">{{ t("生成") }}</button>
        </div>
        <div class="strength-row compact">
          <div class="strength-track"><i :style="{ width: `${strength.score * 25}%` }" /></div>
          <span>{{ t(strength.label) }}</span>
        </div>

        <section v-if="showGenerator" class="generator-panel">
          <div class="generator-top">
            <label>{{ t("长度") }} <b>{{ generator.length }}</b></label>
            <input v-model.number="generator.length" type="range" min="12" max="64" />
          </div>
          <div class="check-row">
            <label><input v-model="generator.uppercase" type="checkbox" /> {{ t("大写") }}</label>
            <label><input v-model="generator.lowercase" type="checkbox" /> {{ t("小写") }}</label>
            <label><input v-model="generator.digits" type="checkbox" /> {{ t("数字") }}</label>
            <label><input v-model="generator.symbols" type="checkbox" /> {{ t("符号") }}</label>
            <button class="secondary small" type="button" :disabled="generating" @click="generate">{{ t("生成密码") }}</button>
          </div>
        </section>

        <label>{{ t("标签") }}</label>
        <input v-model="tagsText" :placeholder="t('工作, 邮箱（使用逗号分隔）')" />

        <div class="section-title-row">
          <label>{{ t("自定义字段") }}</label>
          <button class="text-button" type="button" @click="addCustomField">{{ t("＋ 添加字段") }}</button>
        </div>
        <div v-for="(field, index) in form.customFields" :key="field.id" class="custom-field-row">
          <input v-model="field.label" :placeholder="t('字段名称')" />
          <input v-model="field.value" :type="field.secret ? 'password' : 'text'" :placeholder="t('字段内容')" />
          <label class="secret-toggle"><input v-model="field.secret" type="checkbox" /> {{ t("敏感") }}</label>
          <button class="icon-button danger-quiet" type="button" :aria-label="t('删除字段')" @click="form.customFields.splice(index, 1)"><IconGlyph name="close" :size="16" /></button>
        </div>

        <label>{{ t("备注") }}</label>
        <textarea v-model="form.notes" rows="4" :placeholder="t('仅在解锁后可见')" />
        <label class="favorite-check"><input v-model="form.favorite" type="checkbox" /> {{ t("添加到收藏") }}</label>

        <p v-if="localError" class="form-error">{{ localError }}</p>
        <footer class="modal-footer">
          <button class="secondary" type="button" @click="emit('cancel')">{{ t("取消") }}</button>
          <button class="primary" type="submit">{{ t(entry ? "保存更改" : "保存条目") }}</button>
        </footer>
      </form>
    </section>
  </div>
</template>
