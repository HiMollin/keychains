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
  <div class="fixed inset-0 z-60 grid place-items-center bg-[rgba(12,12,11,0.38)] p-6 backdrop-blur-[3px]" @mousedown.self="emit('cancel')">
    <section class="max-h-[calc(100vh-48px)] w-[min(440px,100%)] overflow-hidden rounded-ui-panel border border-line-strong bg-surface shadow-dialog" role="dialog" aria-modal="true">
      <header class="flex items-center justify-between border-b border-line bg-surface px-[21px] pt-[18px] pb-[15px]">
        <div>
          <p class="mb-[5px] text-[10px] font-semibold tracking-[0.02em] text-muted">{{ t("新建类型") }}</p>
          <h2 class="m-0 text-[17px] font-[660]">{{ t("定义条目类型") }}</h2>
        </div>
        <button class="grid size-8 place-items-center rounded-ui-control border-0 bg-transparent p-0 text-[22px] text-muted hover:bg-hover hover:text-foreground" :aria-label="t('关闭')" @click="emit('cancel')"><IconGlyph name="close" :size="18" /></button>
      </header>
      <form class="px-[22px] pt-[5px] pb-[22px]" @submit.prevent="submit">
        <label>{{ t("类型名称 *") }}</label>
        <input v-model="name" autofocus maxlength="30" :placeholder="t('例如：银行卡、服务器、邮箱')" />

        <p v-if="localError" class="my-3 rounded-lg bg-danger-soft px-[11px] py-[9px] text-[11.5px] leading-[1.45] text-danger">{{ localError }}</p>
        <footer class="mt-[22px] flex justify-end gap-[9px] border-t border-line pt-[18px]">
          <button class="min-h-[var(--button-height)] rounded-ui-control border border-line-strong bg-surface px-3.5 text-xs font-[620] disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:bg-control" type="button" @click="emit('cancel')">{{ t("取消") }}</button>
          <button class="min-h-[var(--button-height)] rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground disabled:cursor-not-allowed disabled:opacity-[0.52] enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" type="submit" :disabled="busy">{{ t("创建类型") }}</button>
        </footer>
      </form>
    </section>
  </div>
</template>
