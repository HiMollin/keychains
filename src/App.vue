<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import EntryEditor from "./components/EntryEditor.vue";
import IconGlyph from "./components/IconGlyph.vue";
import TypeEditor from "./components/TypeEditor.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import SetupScreen from "./components/SetupScreen.vue";
import UnlockScreen from "./components/UnlockScreen.vue";
import { api } from "./api";
import { refreshSystemLanguage, setLanguage, t } from "./i18n";
import { useVaultStore } from "./stores/vault";
import type { AppSettings, Entry, EntryInput, EntryTypeInput, SortOrder } from "./types";
import { errorMessage, formatDate, initials } from "./utils/security";

const store = useVaultStore();
const showEditor = ref(false);
const editingEntry = ref<Entry | null>(null);
const showTypeEditor = ref(false);
const openEntryAfterType = ref(false);
const showSettings = ref(false);
const query = ref("");
const searchInput = ref<HTMLInputElement | null>(null);
const contentGrid = ref<HTMLElement | null>(null);
const revealed = ref<Set<string>>(new Set());
const listPanePercent = ref(42);
const resizingPanels = ref(false);
const sidebarCollapsed = ref(localStorage.getItem("keychains-sidebar-collapsed") === "true");
const savedSidebarWidth = Number(localStorage.getItem("keychains-sidebar-width")) || 242;
const sidebarWidth = ref(Math.min(340, Math.max(186, savedSidebarWidth)));
const resizingSidebar = ref(false);
let queryTimer: number | undefined;
let idleTimer: number | undefined;
let toastTimer: number | undefined;
const revealTimers = new Map<string, number>();
type ToastTone = "default" | "success" | "error";
interface ToastMessage {
  id: number;
  message: string;
  tone: ToastTone;
}
const currentToast = ref<ToastMessage | null>(null);
let nextToastId = 0;

const viewState = computed(() => {
  if (!store.vaultState) return "loading";
  if (!store.vaultState.exists) return "setup";
  if (store.vaultState.locked) return "locked";
  return "vault";
});

const pageTitle = computed(() => {
  if (store.options.favoriteOnly) return t("收藏");
  if (store.options.tag) return t("标签：{tag}", { tag: store.options.tag });
  if (store.options.typeId) return store.entryTypes.find((item) => item.id === store.options.typeId)?.name ?? t("类型");
  return t("全部条目");
});

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value;
  localStorage.setItem("keychains-sidebar-collapsed", String(sidebarCollapsed.value));
}

function prepareDetailLeave(element: Element) {
  const panel = element as HTMLElement;
  panel.style.width = `${panel.getBoundingClientRect().width}px`;
}

function cleanupDetailLeave(element: Element) {
  (element as HTMLElement).style.removeProperty("width");
}

function resizeSidebar(clientX: number) {
  const minimum = 186;
  const workspaceMinimum = store.selected ? 610 : 430;
  const maximum = Math.max(minimum, Math.min(340, window.innerWidth - workspaceMinimum));
  sidebarWidth.value = Math.min(Math.max(clientX, minimum), maximum);
}

function handleSidebarResize(event: PointerEvent) {
  resizeSidebar(event.clientX);
}

function stopSidebarResize() {
  if (!resizingSidebar.value) return;
  resizingSidebar.value = false;
  localStorage.setItem("keychains-sidebar-width", String(Math.round(sidebarWidth.value)));
  window.removeEventListener("pointermove", handleSidebarResize);
  window.removeEventListener("pointerup", stopSidebarResize);
  window.removeEventListener("pointercancel", stopSidebarResize);
}

function startSidebarResize(event: PointerEvent) {
  if (event.button !== 0) return;
  event.preventDefault();
  resizingSidebar.value = true;
  resizeSidebar(event.clientX);
  window.addEventListener("pointermove", handleSidebarResize);
  window.addEventListener("pointerup", stopSidebarResize);
  window.addEventListener("pointercancel", stopSidebarResize);
}

function adjustSidebarSize(event: KeyboardEvent) {
  if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
  event.preventDefault();
  resizeSidebar(sidebarWidth.value + (event.key === "ArrowLeft" ? -8 : 8));
  localStorage.setItem("keychains-sidebar-width", String(Math.round(sidebarWidth.value)));
}

function resetSidebarSize() {
  sidebarWidth.value = 242;
  localStorage.setItem("keychains-sidebar-width", "242");
}

async function runWindowAction(action: (window: ReturnType<typeof getCurrentWindow>) => Promise<void>) {
  try {
    await action(getCurrentWindow());
  } catch {
    // Window controls are only available in the desktop runtime.
  }
}

function minimizeWindow() {
  return runWindowAction((window) => window.minimize());
}

function toggleMaximizeWindow() {
  return runWindowAction((window) => window.toggleMaximize());
}

function closeWindow() {
  return runWindowAction((window) => window.close());
}

async function safely(action: () => Promise<unknown>) {
  try {
    await action();
  } catch {
    // The store exposes a localized error message.
  }
}

async function createVault(password: string) {
  await safely(() => store.initialize(password));
}

async function restoreVault(path: string, backupPassword: string, masterPassword: string) {
  await safely(() => store.restoreNew(path, backupPassword, masterPassword));
}

async function unlock(password: string) {
  await safely(() => store.unlock(password));
}

async function lock() {
  showEditor.value = false;
  showTypeEditor.value = false;
  showSettings.value = false;
  clearSecrets();
  await safely(() => store.lock());
}

function newEntry() {
  if (!store.entryTypes.length) {
    openEntryAfterType.value = true;
    showTypeEditor.value = true;
    return;
  }
  editingEntry.value = null;
  showEditor.value = true;
}

function newType(openEntry = false) {
  openEntryAfterType.value = openEntry;
  showTypeEditor.value = true;
}

function closeTypeEditor() {
  showTypeEditor.value = false;
  openEntryAfterType.value = false;
}

async function saveType(input: EntryTypeInput) {
  await safely(async () => {
    const created = await store.createType(input);
    if (!created) return;
    showTypeEditor.value = false;
    if (openEntryAfterType.value) {
      editingEntry.value = null;
      showEditor.value = true;
    }
    openEntryAfterType.value = false;
  });
}

async function deleteType(id: string, name: string) {
  if (!window.confirm(t("确定删除类型“{name}”吗？", { name }))) return;
  await safely(() => store.removeType(id));
}

function editEntry() {
  if (!store.selected) return;
  editingEntry.value = store.selected;
  showEditor.value = true;
}

async function saveEntry(input: EntryInput, id?: string) {
  await safely(async () => {
    await store.save(input, id);
    showEditor.value = false;
    editingEntry.value = null;
  });
}

async function removeSelected() {
  const selected = store.selected;
  if (!selected) return;
  if (!window.confirm(t("确定永久删除“{name}”吗？此操作无法撤销。", { name: selected.name }))) return;
  await safely(() => store.remove(selected.id));
}

async function selectEntry(id: string) {
  clearSecrets();
  await safely(() => store.toggleSelection(id));
  if (!sidebarCollapsed.value) resizeSidebar(sidebarWidth.value);
}

function reveal(key: string) {
  const next = new Set(revealed.value);
  if (next.has(key)) {
    next.delete(key);
    window.clearTimeout(revealTimers.get(key));
    revealTimers.delete(key);
  } else {
    next.add(key);
    const timer = window.setTimeout(() => {
      const hidden = new Set(revealed.value);
      hidden.delete(key);
      revealed.value = hidden;
      revealTimers.delete(key);
    }, 15_000);
    revealTimers.set(key, timer);
  }
  revealed.value = next;
}

function clearSecrets() {
  revealTimers.forEach((timer) => window.clearTimeout(timer));
  revealTimers.clear();
  revealed.value = new Set();
}

function dismissToast(id = currentToast.value?.id) {
  if (id === undefined || currentToast.value?.id !== id) return;
  window.clearTimeout(toastTimer);
  currentToast.value = null;
}

function showToast(message: string, tone: ToastTone = "default", duration = 2_800) {
  window.clearTimeout(toastTimer);
  const toast: ToastMessage = { id: ++nextToastId, message, tone };
  currentToast.value = toast;
  toastTimer = window.setTimeout(() => dismissToast(toast.id), duration);
}

function handleSettingsNotification(message: string, tone: "success" | "error") {
  showToast(message, tone, tone === "error" ? 4_000 : 2_800);
}

async function copyField(field: string, label: string) {
  if (!store.selected) return;
  try {
    await api.copyField(store.selected.id, field);
    showToast(t("{label}已复制，将按设置自动清除", { label }), "success");
  } catch (error) {
    store.error = errorMessage(error);
  }
}

async function chooseSection(typeId: string | null, favorite = false) {
  query.value = "";
  if (favorite) await store.setFavoriteOnly(true);
  else await store.setType(typeId);
}

async function chooseTag(tag: string) {
  query.value = "";
  await store.setTag(tag);
}

async function updateSort(event: Event) {
  await store.setSort((event.target as HTMLSelectElement).value as SortOrder);
}

function resizePanels(clientX: number) {
  const grid = contentGrid.value;
  if (!grid) return;

  const bounds = grid.getBoundingClientRect();
  const minimumListWidth = 300;
  const minimumDetailWidth = 320;
  const separatorWidth = 7;
  const maximumListWidth = bounds.width - minimumDetailWidth - separatorWidth;
  const nextWidth = Math.min(Math.max(clientX - bounds.left, minimumListWidth), maximumListWidth);
  listPanePercent.value = (nextWidth / bounds.width) * 100;
}

function handlePanelResize(event: PointerEvent) {
  resizePanels(event.clientX);
}

function stopPanelResize() {
  resizingPanels.value = false;
  window.removeEventListener("pointermove", handlePanelResize);
  window.removeEventListener("pointerup", stopPanelResize);
  window.removeEventListener("pointercancel", stopPanelResize);
}

function startPanelResize(event: PointerEvent) {
  if (event.button !== 0) return;
  event.preventDefault();
  resizingPanels.value = true;
  resizePanels(event.clientX);
  window.addEventListener("pointermove", handlePanelResize);
  window.addEventListener("pointerup", stopPanelResize);
  window.addEventListener("pointercancel", stopPanelResize);
}

function adjustPanelSize(event: KeyboardEvent) {
  if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
  event.preventDefault();
  const direction = event.key === "ArrowLeft" ? -1 : 1;
  listPanePercent.value = Math.min(Math.max(listPanePercent.value + direction * 2, 30), 70);
}

function resetPanelSize() {
  listPanePercent.value = 42;
}

async function saveSettings(settings: AppSettings) {
  await safely(async () => {
    await store.saveSettings(settings);
    applyTheme(settings.theme);
    setLanguage(settings.language);
    resetIdleTimer();
  });
}

function applyTheme(theme: AppSettings["theme"]) {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem("keychains-theme", theme);
}

function handleLanguageChange() {
  refreshSystemLanguage();
}

function resetIdleTimer() {
  window.clearTimeout(idleTimer);
  if (!store.unlocked || !store.settings) return;
  idleTimer = window.setTimeout(() => void lock(), store.settings.autoLockMinutes * 60_000);
}

function handleKeydown(event: KeyboardEvent) {
  resetIdleTimer();
  if (!store.unlocked) return;
  if (event.ctrlKey && event.key.toLowerCase() === "k") {
    event.preventDefault();
    nextTick(() => searchInput.value?.focus());
  }
  if (event.ctrlKey && event.key.toLowerCase() === "l") {
    event.preventDefault();
    void lock();
  }
  if (event.ctrlKey && event.key.toLowerCase() === "n") {
    event.preventDefault();
    newEntry();
  }
  if (event.key === "Escape") {
    showEditor.value = false;
    showTypeEditor.value = false;
    showSettings.value = false;
  }
}

function typeLabel(typeId: string) {
  return store.entryTypes.find((item) => item.id === typeId)?.name ?? t("未知类型");
}

watch(query, (value) => {
  window.clearTimeout(queryTimer);
  queryTimer = window.setTimeout(() => void store.setQuery(value), 180);
});

watch(
  () => store.settings?.theme,
  (theme) => theme && applyTheme(theme),
  { immediate: true }
);

watch(
  () => store.settings?.language,
  (language) => language && setLanguage(language),
  { immediate: true }
);

watch(
  () => store.unlocked,
  () => resetIdleTimer()
);

watch(
  () => store.notice,
  (notice) => {
    if (!notice) return;
    showToast(notice, "success");
    store.notice = "";
  }
);

watch(
  [() => store.error, viewState],
  ([error, state]) => {
    if (!error || state !== "vault") return;
    showToast(error, "error", 4_000);
    store.error = "";
  }
);

onMounted(async () => {
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("pointerdown", resetIdleTimer);
  window.addEventListener("languagechange", handleLanguageChange);
  await safely(() => store.refreshState());
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("pointerdown", resetIdleTimer);
  window.removeEventListener("languagechange", handleLanguageChange);
  window.clearTimeout(queryTimer);
  window.clearTimeout(idleTimer);
  window.clearTimeout(toastTimer);
  stopPanelResize();
  stopSidebarResize();
  clearSecrets();
});
</script>

<template>
  <div
    class="relative size-full overflow-hidden bg-window"
    :class="{ 'sidebar-is-resizing': resizingSidebar }"
    :style="{ '--sidebar-live-width': `${sidebarWidth}px` }"
  >
    <header class="pointer-events-none absolute inset-x-0 top-0 z-40 h-[var(--titlebar-height)] select-none">
      <div v-if="viewState !== 'vault'" class="pointer-events-auto absolute top-0 right-[138px] left-0 h-[var(--titlebar-height)]" data-tauri-drag-region @dblclick="toggleMaximizeWindow" />
      <div class="pointer-events-auto absolute top-0 right-0 grid grid-cols-[repeat(3,46px)]">
        <button class="grid h-[calc(var(--titlebar-height)-1px)] w-[46px] place-items-center rounded-none border-0 bg-transparent p-0 text-muted hover:bg-hover hover:text-foreground" :aria-label="t('最小化')" :title="t('最小化')" @dblclick.stop @click.stop="minimizeWindow"><IconGlyph name="minimize" :size="17" /></button>
        <button class="grid h-[calc(var(--titlebar-height)-1px)] w-[46px] place-items-center rounded-none border-0 bg-transparent p-0 text-muted hover:bg-hover hover:text-foreground" :aria-label="t('最大化')" :title="t('最大化')" @dblclick.stop @click.stop="toggleMaximizeWindow"><IconGlyph name="maximize" :size="16" /></button>
        <button class="grid h-[calc(var(--titlebar-height)-1px)] w-[46px] place-items-center rounded-none border-0 bg-transparent p-0 text-muted hover:bg-[#c42b1c] hover:text-white" :aria-label="t('关闭')" :title="t('关闭')" @dblclick.stop @click.stop="closeWindow"><IconGlyph name="close" :size="18" /></button>
      </div>
    </header>

    <div class="size-full min-h-0 min-w-0 overflow-hidden">
  <div v-if="viewState === 'loading'" class="grid size-full content-center place-items-center gap-3 overflow-hidden bg-window p-5 text-muted">
    <p>{{ t("正在打开 Keychains…") }}</p>
  </div>

  <SetupScreen
    v-else-if="viewState === 'setup'"
    :busy="store.busy"
    :error="store.error"
    @create="createVault"
    @restore="restoreVault"
  />

  <UnlockScreen
    v-else-if="viewState === 'locked'"
    :busy="store.busy"
    :error="store.error"
    @unlock="unlock"
  />

  <div v-else class="app-shell grid size-full overflow-hidden bg-window" :class="{ collapsed: sidebarCollapsed }">
    <aside class="sidebar col-start-1 flex min-w-0 flex-col overflow-hidden bg-sidebar p-2.5">
      <div class="min-h-0 flex-auto overflow-x-hidden overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <button class="mb-0.5 flex min-h-9 w-full items-center gap-2.5 rounded-lg border-0 bg-transparent px-[9px] text-left text-[12.5px] font-normal whitespace-nowrap text-muted hover:bg-hover hover:text-foreground [&>span]:min-w-0 [&>span]:overflow-hidden [&>span]:text-ellipsis" @click="newEntry"><IconGlyph name="plus" :size="18" /><span>{{ t(store.entryTypes.length ? "新建条目" : "先创建类型") }}</span></button>

      <nav class="grid gap-0.5 [&>button]:flex [&>button]:min-h-9 [&>button]:w-full [&>button]:items-center [&>button]:gap-2.5 [&>button]:rounded-lg [&>button]:border-0 [&>button]:bg-transparent [&>button]:px-[9px] [&>button]:text-left [&>button]:text-[12.5px] [&>button]:font-normal [&>button]:whitespace-nowrap [&>button]:text-muted [&>button:hover]:bg-hover [&>button:hover]:text-foreground">
        <button :class="{ 'bg-selected font-[620] text-foreground': !store.options.typeId && !store.options.favoriteOnly && !store.options.tag }" @click="chooseSection(null)">
          <span class="grid h-5 w-[19px] flex-[0_0_19px] place-items-center text-current"><IconGlyph name="vault" /></span><span class="min-w-0 overflow-hidden text-ellipsis">{{ t("全部条目") }}</span> <em class="ml-auto text-[10px] not-italic text-faint">{{ store.entries.length }}</em>
        </button>
        <button :class="{ 'bg-selected font-[620] text-foreground': store.options.favoriteOnly }" @click="chooseSection(null, true)"><span class="grid h-5 w-[19px] flex-[0_0_19px] place-items-center text-current"><IconGlyph name="star" /></span><span class="min-w-0 overflow-hidden text-ellipsis">{{ t("收藏") }}</span></button>
      </nav>

      <div class="group mt-[17px] mr-[3px] mb-1.5 ml-[9px] flex items-center justify-between">
        <p class="m-0 flex items-center gap-[9px] text-xs font-[620] text-muted"><IconGlyph name="folder" :size="16" /><span>{{ t("类型") }}</span></p>
        <button class="grid size-[25px] place-items-center rounded-[7px] border-0 bg-transparent p-0 text-muted opacity-0 transition-opacity duration-[120ms] group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-hover hover:text-foreground focus-visible:bg-hover focus-visible:text-foreground" :aria-label="t('创建类型')" :title="t('创建类型')" @click="newType(false)"><IconGlyph name="plus" :size="16" /></button>
      </div>
      <nav class="grid gap-0.5">
        <div v-for="item in store.entryTypes" :key="item.id" class="group relative">
          <button class="flex min-h-[38px] w-full items-center gap-2.5 rounded-[11px] border-0 bg-transparent pr-[33px] pl-[43px] text-left text-[13px] font-normal whitespace-nowrap text-muted hover:bg-hover hover:text-foreground" :class="{ 'bg-selected font-medium text-foreground': store.options.typeId === item.id }" @click="chooseSection(item.id)">
            <span class="min-w-0 overflow-hidden text-ellipsis">{{ item.name }}</span>
          </button>
          <button class="absolute top-1/2 right-1.5 grid min-h-[26px] w-[26px] -translate-y-1/2 place-items-center rounded-[7px] border-0 bg-transparent p-0 text-muted opacity-0 transition-opacity duration-[120ms] group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-danger-soft hover:text-danger focus-visible:bg-danger-soft focus-visible:text-danger" :aria-label="t('删除类型 {name}', { name: item.name })" :title="t('删除 {name}', { name: item.name })" @click="deleteType(item.id, item.name)">
            <IconGlyph name="close" :size="14" />
          </button>
        </div>
        <p v-if="!store.entryTypes.length" class="mt-[7px] mr-[9px] mb-1 ml-[43px] text-[11px] text-faint">{{ t("尚未创建类型") }}</p>
      </nav>

      <template v-if="store.tags.length">
        <div class="mt-[18px] mr-[3px] mb-1.5 ml-[9px] flex items-center justify-between">
          <p class="m-0 flex items-center gap-[9px] text-xs font-[620] text-muted"><IconGlyph name="tag" :size="16" /><span>{{ t("标签") }}</span></p>
        </div>
        <nav class="grid gap-0.5">
          <button v-for="tag in store.tags" :key="tag" class="flex min-h-[38px] w-full items-center gap-2.5 rounded-[11px] border-0 bg-transparent pr-[33px] pl-[43px] text-left text-[13px] font-normal whitespace-nowrap text-muted hover:bg-hover hover:text-foreground" :class="{ 'bg-selected font-medium text-foreground': store.options.tag === tag }" @click="chooseTag(tag)">
            <span class="min-w-0 overflow-hidden text-ellipsis">{{ tag }}</span>
          </button>
        </nav>
      </template>
      </div>

      <div class="mt-[9px] grid flex-none gap-0.5 border-t border-line pt-[9px] [&>button]:flex [&>button]:min-h-9 [&>button]:w-full [&>button]:items-center [&>button]:gap-2.5 [&>button]:rounded-lg [&>button]:border-0 [&>button]:bg-transparent [&>button]:px-[9px] [&>button]:text-left [&>button]:text-[12.5px] [&>button]:whitespace-nowrap [&>button]:text-muted [&>button:hover]:bg-hover [&>button:hover]:text-foreground">
        <button @click="showSettings = true"><span class="grid h-5 w-[19px] flex-[0_0_19px] place-items-center"><IconGlyph name="settings" /></span><span class="min-w-0 overflow-hidden text-ellipsis">{{ t("设置") }}</span></button>
        <button @click="lock"><span class="grid h-5 w-[19px] flex-[0_0_19px] place-items-center"><IconGlyph name="lock" /></span><span class="min-w-0 overflow-hidden text-ellipsis">{{ t("锁定") }}</span> <kbd class="ml-auto">Ctrl L</kbd></button>
      </div>
    </aside>

    <div
      v-if="!sidebarCollapsed"
      class="sidebar-resizer relative z-4 col-start-2 w-[5px] min-w-[5px] cursor-col-resize touch-none bg-sidebar outline-0"
      role="separator"
      :aria-label="t('调整侧边栏宽度')"
      aria-orientation="vertical"
      :aria-valuenow="Math.round(sidebarWidth)"
      aria-valuemin="186"
      aria-valuemax="340"
      tabindex="0"
      @pointerdown="startSidebarResize"
      @keydown="adjustSidebarSize"
      @dblclick="resetSidebarSize"
    />

    <main class="col-start-3 flex w-full min-h-0 min-w-0 flex-col overflow-hidden bg-window">
      <header class="flex h-[70px] flex-none items-center justify-start gap-2.5 bg-window pt-3.5 pr-[154px] pb-2.5 pl-3.5" data-tauri-drag-region @dblclick="toggleMaximizeWindow">
        <button
          class="grid size-8 flex-none place-items-center rounded-lg border-0 bg-transparent p-0 text-muted hover:bg-hover hover:text-foreground"
          :aria-label="t(sidebarCollapsed ? '展开侧边栏' : '收起侧边栏')"
          :title="t(sidebarCollapsed ? '展开侧边栏' : '收起侧边栏')"
          @dblclick.stop
          @click.stop="toggleSidebar"
        >
          <IconGlyph name="sidebar" :size="17" />
        </button>
        <div data-tauri-drag-region>
          <h1 class="m-0 text-xl font-[680]" data-tauri-drag-region>{{ pageTitle }}</h1>
        </div>
      </header>

      <div class="flex flex-none gap-2 border-b border-line bg-window pr-[18px] pb-[13px] pl-[22px] max-[760px]:flex-wrap">
        <div class="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-[10px] border border-line bg-control px-2.5">
          <span class="grid flex-none place-items-center text-muted"><IconGlyph name="search" :size="17" /></span>
          <input ref="searchInput" v-model="query" class="h-[34px] min-w-0 border-0 bg-transparent p-0 shadow-none focus:shadow-none" :placeholder="t('搜索名称、账号、网址或标签')" />
          <kbd>Ctrl K</kbd>
        </div>
        <select class="h-9 min-h-9 w-[148px] flex-none border-line bg-control px-2.5 py-0 max-[760px]:w-full" :value="store.options.sort" :aria-label="t('排序')" @change="updateSort">
          <option value="updatedDesc">{{ t("最近更新") }}</option>
          <option value="createdDesc">{{ t("最近创建") }}</option>
          <option value="nameAsc">{{ t("名称 A–Z") }}</option>
          <option value="nameDesc">{{ t("名称 Z–A") }}</option>
          <option value="usernameAsc">{{ t("账号名称") }}</option>
        </select>
      </div>

      <div
        ref="contentGrid"
        class="content-grid grid min-h-0 min-w-0 flex-1 overflow-hidden bg-window"
        :class="{ 'detail-open': store.selected, resizing: resizingPanels }"
        :style="store.selected ? { '--list-pane-width': `${listPanePercent}%` } : undefined"
      >
        <section class="min-w-0 overflow-auto bg-window [scrollbar-color:color-mix(in_srgb,var(--muted)_40%,transparent)_transparent] [scrollbar-width:thin]">
          <div v-if="store.entries.length" class="p-2">
            <button
              v-for="entry in store.entries"
              :key="entry.id"
              class="grid min-h-[70px] w-full grid-cols-[38px_minmax(0,1fr)_auto] items-center gap-[11px] rounded-[10px] border-0 bg-transparent px-2.5 py-2 text-left text-foreground hover:bg-hover"
              :class="{ 'bg-selected': store.selected?.id === entry.id }"
              @click="selectEntry(entry.id)"
            >
              <div class="grid size-[38px] place-items-center rounded-[10px] bg-[#e2e7f7] text-[11px] font-[760] text-[#29469f]">{{ initials(entry.name) }}</div>
              <div class="min-w-0">
                <div class="flex items-center gap-1.5"><b class="overflow-hidden text-[12.5px] font-[640] text-ellipsis whitespace-nowrap">{{ entry.name }}</b><span v-if="entry.favorite" class="text-[10px] text-[#b88626]">★</span></div>
                <p class="mt-[3px] mb-1 overflow-hidden text-[10.5px] text-ellipsis whitespace-nowrap text-muted">{{ entry.username || entry.url || t("未填写账号") }}</p>
                <div class="flex flex-wrap gap-1 [&>i]:rounded-[5px] [&>i]:bg-control [&>i]:px-1.5 [&>i]:py-0.5 [&>i]:text-[9px] [&>i]:not-italic [&>i]:text-muted [&>span]:rounded-[5px] [&>span]:bg-control [&>span]:px-1.5 [&>span]:py-0.5 [&>span]:text-[9px] [&>span]:text-muted"><span>{{ typeLabel(entry.typeId) }}</span><i v-for="tag in entry.tags.slice(0, 2)" :key="tag">{{ tag }}</i></div>
              </div>
              <time class="mt-[3px] self-start text-[9.5px] text-faint">{{ formatDate(entry.updatedAt) }}</time>
            </button>
          </div>
          <div v-else class="grid h-full content-center justify-items-center p-10 text-center">
            <div class="mb-[15px] grid size-[52px] place-items-center rounded-[14px] bg-control text-muted"><IconGlyph name="search" :size="24" /></div>
            <h2 class="mb-[7px] text-[17px]">{{ t(query ? "没有匹配的条目" : "这里还没有账号") }}</h2>
            <p class="mb-0 max-w-[330px] leading-[1.55] text-muted">{{ t(query ? "试试其他关键词或清除筛选条件。" : store.entryTypes.length ? "添加第一个账号条目开始使用。" : "请先创建类型，然后就能添加条目。") }}</p>
            <button v-if="!query && !store.entryTypes.length" class="mt-[17px] min-h-[var(--button-height)] rounded-ui-control border border-primary bg-primary px-3.5 text-xs font-[620] text-primary-foreground enabled:hover:border-primary-hover enabled:hover:bg-primary-hover" @click="newType(true)">{{ t("创建第一个类型") }}</button>
          </div>
        </section>

        <div
          v-if="store.selected"
          class="pane-resizer relative z-3 w-[5px] min-w-[5px] cursor-col-resize touch-none outline-0"
          role="separator"
          :aria-label="t('调整密码列表和详情的宽度')"
          aria-orientation="vertical"
          :aria-valuenow="Math.round(listPanePercent)"
          aria-valuemin="30"
          aria-valuemax="70"
          tabindex="0"
          @pointerdown="startPanelResize"
          @keydown="adjustPanelSize"
          @dblclick="resetPanelSize"
        />

        <Transition
          name="detail-slide"
          @before-leave="prepareDetailLeave"
          @after-leave="cleanupDetailLeave"
        >
        <aside v-if="store.selected" class="min-w-0 overflow-y-auto bg-window px-[34px] py-7 [scrollbar-color:color-mix(in_srgb,var(--muted)_40%,transparent)_transparent] [scrollbar-width:thin] max-[1000px]:px-[26px] max-[1000px]:py-6">
          <header class="flex items-center gap-[13px]">
            <div class="grid size-[52px] place-items-center rounded-[13px] bg-[#e2e7f7] text-[13px] font-[760] text-[#29469f]">{{ initials(store.selected.name) }}</div>
            <div class="min-w-0 flex-1"><p class="mb-1 text-[10px] text-muted">{{ typeLabel(store.selected.typeId) }}</p><h2 class="m-0 overflow-hidden text-xl font-[660] text-ellipsis whitespace-nowrap">{{ store.selected.name }}</h2></div>
            <button class="size-[34px] flex-none rounded-lg border-0 bg-transparent p-0 text-[21px] text-faint hover:bg-hover" :class="{ 'text-[#b88626]': store.selected.favorite }" :aria-label="t('收藏')" @click="store.toggleFavorite(store.selected)">
              {{ store.selected.favorite ? "★" : "☆" }}
            </button>
          </header>

          <div class="mt-[22px] mb-[9px] flex items-center gap-2.5">
            <button class="min-h-[var(--button-height)] min-w-[90px] flex-1 rounded-ui-control border border-line-strong bg-surface px-3.5 text-xs font-[620] enabled:hover:bg-control" @click="editEntry">{{ t("编辑") }}</button>
            <button class="border-0 bg-transparent p-[5px] font-semibold text-danger" @click="removeSelected">{{ t("删除") }}</button>
          </div>

          <section class="border-b border-line py-3.5">
            <p class="mb-[7px] text-[10.5px] font-[540] text-muted">{{ t("用户名或邮箱") }}</p>
            <div class="flex min-w-0 items-center gap-[7px] [&>button]:min-h-[27px] [&>button]:flex-none [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-control [&>button]:px-[9px] [&>button]:text-[10px] [&>button]:font-semibold [&>button:hover]:bg-hover [&>span]:min-w-0 [&>span]:flex-1 [&>span]:overflow-hidden [&>span]:text-[12.5px] [&>span]:text-ellipsis [&>span]:whitespace-nowrap"><span>{{ store.selected.username || "—" }}</span><button v-if="store.selected.username" @click="copyField('username', t('用户名'))">{{ t("复制") }}</button></div>
          </section>
          <section class="border-b border-line py-3.5">
            <p class="mb-[7px] text-[10.5px] font-[540] text-muted">{{ t("密码") }}</p>
            <div class="flex min-w-0 items-center gap-[7px] [&>button]:min-h-[27px] [&>button]:flex-none [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-control [&>button]:px-[9px] [&>button]:text-[10px] [&>button]:font-semibold [&>button:hover]:bg-hover">
              <code class="min-w-0 flex-1 overflow-hidden font-[inherit] text-[12.5px] tracking-[0.08em] text-ellipsis whitespace-nowrap">{{ revealed.has('password') ? store.selected.password || "—" : "••••••••••••" }}</code>
              <button @click="reveal('password')">{{ t(revealed.has("password") ? "隐藏" : "显示") }}</button>
              <button v-if="store.selected.password" @click="copyField('password', t('密码'))">{{ t("复制") }}</button>
            </div>
          </section>
          <section v-if="store.selected.url" class="border-b border-line py-3.5">
            <p class="mb-[7px] text-[10.5px] font-[540] text-muted">{{ t("网址") }}</p>
            <div class="flex min-w-0 items-center gap-[7px]"><span class="min-w-0 flex-1 overflow-hidden text-[12.5px] whitespace-normal [overflow-wrap:anywhere]">{{ store.selected.url }}</span></div>
          </section>
          <section v-for="field in store.selected.customFields" :key="field.id" class="border-b border-line py-3.5">
            <p class="mb-[7px] text-[10.5px] font-[540] text-muted">{{ field.label }}</p>
            <div class="flex min-w-0 items-center gap-[7px] [&>button]:min-h-[27px] [&>button]:flex-none [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-control [&>button]:px-[9px] [&>button]:text-[10px] [&>button]:font-semibold [&>button:hover]:bg-hover">
              <code v-if="field.secret" class="min-w-0 flex-1 overflow-hidden font-[inherit] text-[12.5px] text-ellipsis whitespace-nowrap">{{ revealed.has(field.id) ? field.value : "••••••••" }}</code>
              <span v-else class="min-w-0 flex-1 overflow-hidden text-[12.5px] text-ellipsis whitespace-nowrap">{{ field.value || "—" }}</span>
              <button v-if="field.secret" @click="reveal(field.id)">{{ t(revealed.has(field.id) ? "隐藏" : "显示") }}</button>
              <button @click="copyField(`custom:${field.id}`, field.label)">{{ t("复制") }}</button>
            </div>
          </section>
          <section v-if="store.selected.tags.length" class="border-b border-line py-3.5">
            <p class="mb-[7px] text-[10.5px] font-[540] text-muted">{{ t("标签") }}</p>
            <div class="flex flex-wrap gap-1 [&>span]:rounded-[5px] [&>span]:bg-control [&>span]:px-1.5 [&>span]:py-0.5 [&>span]:text-[9px] [&>span]:text-muted"><span v-for="tag in store.selected.tags" :key="tag">{{ tag }}</span></div>
          </section>
          <section v-if="store.selected.notes" class="border-b border-line py-3.5">
            <p class="mb-[7px] text-[10.5px] font-[540] text-muted">{{ t("备注") }}</p>
            <p class="m-0 text-xs leading-[1.6] whitespace-pre-wrap text-foreground">{{ store.selected.notes }}</p>
          </section>
          <footer class="px-0 pt-[17px] pb-[5px] text-[9px] text-faint">{{ t("创建于 {created} · 更新于 {updated}", { created: formatDate(store.selected.createdAt), updated: formatDate(store.selected.updatedAt) }) }}</footer>
        </aside>
        </Transition>
      </div>
    </main>
  </div>

  <EntryEditor v-if="showEditor" :entry="editingEntry" :entry-types="store.entryTypes" @save="saveEntry" @cancel="showEditor = false" />
  <TypeEditor v-if="showTypeEditor" :busy="store.busy" @save="saveType" @cancel="closeTypeEditor" />
  <SettingsDialog
    v-if="showSettings && store.settings"
    :settings="store.settings"
    @close="showSettings = false"
    @save-settings="saveSettings"
    @imported="store.loadEntries"
    @notify="handleSettingsNotification"
  />

  <Transition name="toast" mode="out-in">
    <div
      v-if="currentToast"
      :key="currentToast.id"
      class="toast fixed right-6 bottom-[22px] z-100 max-w-[360px] cursor-pointer rounded-[10px] border border-line-strong bg-surface px-[15px] py-[11px] text-xs text-foreground shadow-dialog"
      :class="currentToast.tone"
      role="status"
      aria-live="polite"
      @click="dismissToast(currentToast.id)"
    >
      {{ currentToast.message }}
    </div>
  </Transition>
    </div>
  </div>
</template>
