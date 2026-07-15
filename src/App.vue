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
    class="window-root"
    :class="{ 'vault-window': viewState === 'vault' || viewState === 'locked', 'sidebar-is-collapsed': sidebarCollapsed && viewState === 'vault', 'sidebar-is-resizing': resizingSidebar }"
    :style="{ '--sidebar-live-width': `${sidebarWidth}px` }"
  >
    <header class="window-titlebar">
      <div v-if="viewState !== 'vault'" class="titlebar-drag-region" data-tauri-drag-region @dblclick="toggleMaximizeWindow" />
      <div class="window-controls">
        <button :aria-label="t('最小化')" :title="t('最小化')" @dblclick.stop @click.stop="minimizeWindow"><IconGlyph name="minimize" :size="17" /></button>
        <button :aria-label="t('最大化')" :title="t('最大化')" @dblclick.stop @click.stop="toggleMaximizeWindow"><IconGlyph name="maximize" :size="16" /></button>
        <button class="window-close" :aria-label="t('关闭')" :title="t('关闭')" @dblclick.stop @click.stop="closeWindow"><IconGlyph name="close" :size="18" /></button>
      </div>
    </header>

    <div class="window-content">
  <div v-if="viewState === 'loading'" class="loading-page">
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

  <div v-else class="app-shell" :class="{ collapsed: sidebarCollapsed }">
    <aside class="sidebar">
      <div class="sidebar-scroll">
      <button class="sidebar-new" @click="newEntry"><IconGlyph name="plus" :size="18" /><span>{{ t(store.entryTypes.length ? "新建条目" : "先创建类型") }}</span></button>

      <nav class="main-nav">
        <button :class="{ active: !store.options.typeId && !store.options.favoriteOnly && !store.options.tag }" @click="chooseSection(null)">
          <span class="nav-symbol"><IconGlyph name="vault" /></span><span>{{ t("全部条目") }}</span> <em>{{ store.entries.length }}</em>
        </button>
        <button :class="{ active: store.options.favoriteOnly }" @click="chooseSection(null, true)"><span class="nav-symbol"><IconGlyph name="star" /></span><span>{{ t("收藏") }}</span></button>
      </nav>

      <div class="nav-label-row">
        <p class="nav-label"><IconGlyph name="folder" :size="16" /><span>{{ t("类型") }}</span></p>
        <button :aria-label="t('创建类型')" :title="t('创建类型')" @click="newType(false)"><IconGlyph name="plus" :size="16" /></button>
      </div>
      <nav class="main-nav type-nav">
        <div v-for="item in store.entryTypes" :key="item.id" class="type-row">
          <button class="type-select" :class="{ active: store.options.typeId === item.id }" @click="chooseSection(item.id)">
            <span>{{ item.name }}</span>
          </button>
          <button class="type-delete" :aria-label="t('删除类型 {name}', { name: item.name })" :title="t('删除 {name}', { name: item.name })" @click="deleteType(item.id, item.name)">
            <IconGlyph name="close" :size="14" />
          </button>
        </div>
        <p v-if="!store.entryTypes.length" class="empty-types">{{ t("尚未创建类型") }}</p>
      </nav>

      <template v-if="store.tags.length">
        <div class="nav-label-row static-label-row">
          <p class="nav-label"><IconGlyph name="tag" :size="16" /><span>{{ t("标签") }}</span></p>
        </div>
        <nav class="main-nav tag-nav">
          <button v-for="tag in store.tags" :key="tag" :class="{ active: store.options.tag === tag }" @click="chooseTag(tag)">
            <span>{{ tag }}</span>
          </button>
        </nav>
      </template>
      </div>

      <div class="sidebar-footer">
        <button @click="showSettings = true"><span class="footer-symbol"><IconGlyph name="settings" /></span><span>{{ t("设置") }}</span></button>
        <button @click="lock"><span class="footer-symbol"><IconGlyph name="lock" /></span><span>{{ t("锁定") }}</span> <kbd>Ctrl L</kbd></button>
      </div>
    </aside>

    <div
      v-if="!sidebarCollapsed"
      class="sidebar-resizer"
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

    <main class="vault-main">
      <header class="topbar" data-tauri-drag-region @dblclick="toggleMaximizeWindow">
        <button
          class="workspace-sidebar-toggle"
          :aria-label="t(sidebarCollapsed ? '展开侧边栏' : '收起侧边栏')"
          :title="t(sidebarCollapsed ? '展开侧边栏' : '收起侧边栏')"
          @dblclick.stop
          @click.stop="toggleSidebar"
        >
          <IconGlyph name="sidebar" :size="17" />
        </button>
        <div data-tauri-drag-region>
          <h1 data-tauri-drag-region>{{ pageTitle }}</h1>
        </div>
      </header>

      <div class="toolbar">
        <div class="search-box">
          <span><IconGlyph name="search" :size="17" /></span>
          <input ref="searchInput" v-model="query" :placeholder="t('搜索名称、账号、网址或标签')" />
          <kbd>Ctrl K</kbd>
        </div>
        <select :value="store.options.sort" :aria-label="t('排序')" @change="updateSort">
          <option value="updatedDesc">{{ t("最近更新") }}</option>
          <option value="createdDesc">{{ t("最近创建") }}</option>
          <option value="nameAsc">{{ t("名称 A–Z") }}</option>
          <option value="nameDesc">{{ t("名称 Z–A") }}</option>
          <option value="usernameAsc">{{ t("账号名称") }}</option>
        </select>
      </div>

      <div
        ref="contentGrid"
        class="content-grid"
        :class="{ 'detail-open': store.selected, resizing: resizingPanels }"
        :style="store.selected ? { '--list-pane-width': `${listPanePercent}%` } : undefined"
      >
        <section class="entry-list-panel">
          <div v-if="store.entries.length" class="entry-list">
            <button
              v-for="entry in store.entries"
              :key="entry.id"
              class="entry-row"
              :class="{ selected: store.selected?.id === entry.id }"
              @click="selectEntry(entry.id)"
            >
              <div class="entry-avatar">{{ initials(entry.name) }}</div>
              <div class="entry-summary">
                <div><b>{{ entry.name }}</b><span v-if="entry.favorite" class="favorite-star">★</span></div>
                <p>{{ entry.username || entry.url || t("未填写账号") }}</p>
                <div class="tag-line"><span>{{ typeLabel(entry.typeId) }}</span><i v-for="tag in entry.tags.slice(0, 2)" :key="tag">{{ tag }}</i></div>
              </div>
              <time>{{ formatDate(entry.updatedAt) }}</time>
            </button>
          </div>
          <div v-else class="empty-state">
            <div class="empty-icon"><IconGlyph name="search" :size="24" /></div>
            <h2>{{ t(query ? "没有匹配的条目" : "这里还没有账号") }}</h2>
            <p>{{ t(query ? "试试其他关键词或清除筛选条件。" : store.entryTypes.length ? "添加第一个账号条目开始使用。" : "请先创建类型，然后就能添加条目。") }}</p>
            <button v-if="!query && !store.entryTypes.length" class="primary empty-action" @click="newType(true)">{{ t("创建第一个类型") }}</button>
          </div>
        </section>

        <div
          v-if="store.selected"
          class="pane-resizer"
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
        <aside v-if="store.selected" class="detail-panel">
          <header class="detail-header">
            <div class="entry-avatar large-avatar">{{ initials(store.selected.name) }}</div>
            <div class="detail-title"><p>{{ typeLabel(store.selected.typeId) }}</p><h2>{{ store.selected.name }}</h2></div>
            <button class="favorite-button" :class="{ active: store.selected.favorite }" :aria-label="t('收藏')" @click="store.toggleFavorite(store.selected)">
              {{ store.selected.favorite ? "★" : "☆" }}
            </button>
          </header>

          <div class="detail-actions">
            <button class="secondary" @click="editEntry">{{ t("编辑") }}</button>
            <button class="danger-quiet text-button" @click="removeSelected">{{ t("删除") }}</button>
          </div>

          <section class="detail-section">
            <p class="detail-label">{{ t("用户名或邮箱") }}</p>
            <div class="value-row"><span>{{ store.selected.username || "—" }}</span><button v-if="store.selected.username" @click="copyField('username', t('用户名'))">{{ t("复制") }}</button></div>
          </section>
          <section class="detail-section">
            <p class="detail-label">{{ t("密码") }}</p>
            <div class="value-row secret-value">
              <code>{{ revealed.has('password') ? store.selected.password || "—" : "••••••••••••" }}</code>
              <button @click="reveal('password')">{{ t(revealed.has("password") ? "隐藏" : "显示") }}</button>
              <button v-if="store.selected.password" @click="copyField('password', t('密码'))">{{ t("复制") }}</button>
            </div>
          </section>
          <section v-if="store.selected.url" class="detail-section">
            <p class="detail-label">{{ t("网址") }}</p>
            <div class="value-row"><span class="wrap-value">{{ store.selected.url }}</span></div>
          </section>
          <section v-for="field in store.selected.customFields" :key="field.id" class="detail-section">
            <p class="detail-label">{{ field.label }}</p>
            <div class="value-row">
              <code v-if="field.secret">{{ revealed.has(field.id) ? field.value : "••••••••" }}</code>
              <span v-else>{{ field.value || "—" }}</span>
              <button v-if="field.secret" @click="reveal(field.id)">{{ t(revealed.has(field.id) ? "隐藏" : "显示") }}</button>
              <button @click="copyField(`custom:${field.id}`, field.label)">{{ t("复制") }}</button>
            </div>
          </section>
          <section v-if="store.selected.tags.length" class="detail-section">
            <p class="detail-label">{{ t("标签") }}</p>
            <div class="detail-tags"><span v-for="tag in store.selected.tags" :key="tag">{{ tag }}</span></div>
          </section>
          <section v-if="store.selected.notes" class="detail-section">
            <p class="detail-label">{{ t("备注") }}</p>
            <p class="notes-text">{{ store.selected.notes }}</p>
          </section>
          <footer class="detail-meta">{{ t("创建于 {created} · 更新于 {updated}", { created: formatDate(store.selected.createdAt), updated: formatDate(store.selected.updatedAt) }) }}</footer>
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
      class="toast"
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
