<script setup lang="ts">
import { ref } from "vue";
import { t } from "../i18n";
import type { EntryTypeInput } from "../types";
import IconGlyph from "./IconGlyph.vue";

defineProps<{ busy: boolean }>();
const emit = defineEmits<{ save: [input: EntryTypeInput]; cancel: [] }>();

const name = ref("");
const localError = ref("");

function submit() {
  localError.value = "";
  const value = name.value.trim();
  if (!value) {
    localError.value = t("请输入类型名称");
    return;
  }
  emit("save", { name: value });
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="emit('cancel')">
    <section class="modal type-modal" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <p class="eyebrow">{{ t("新建类型") }}</p>
          <h2>{{ t("定义条目类型") }}</h2>
        </div>
        <button class="icon-button" :aria-label="t('关闭')" @click="emit('cancel')"><IconGlyph name="close" :size="18" /></button>
      </header>
      <form class="type-form" @submit.prevent="submit">
        <label>{{ t("类型名称 *") }}</label>
        <input v-model="name" autofocus maxlength="30" :placeholder="t('例如：银行卡、服务器、邮箱')" />

        <p v-if="localError" class="form-error">{{ localError }}</p>
        <footer class="modal-footer">
          <button class="secondary" type="button" @click="emit('cancel')">{{ t("取消") }}</button>
          <button class="primary" type="submit" :disabled="busy">{{ t("创建类型") }}</button>
        </footer>
      </form>
    </section>
  </div>
</template>
