import { defineStore } from "pinia";
import { api } from "../api";
import type {
  AppSettings,
  Entry,
  EntryInput,
  EntrySummary,
  EntryType,
  EntryTypeInput,
  ListOptions,
  SortOrder,
  VaultState
} from "../types";
import { errorMessage } from "../utils/security";
import { t } from "../i18n";

const defaultOptions = (): ListOptions => ({
  query: "",
  sort: "updatedDesc",
  favoriteOnly: false,
  typeId: null,
  tag: null
});

type SectionOptions = Pick<ListOptions, "favoriteOnly" | "typeId" | "tag">;

export const useVaultStore = defineStore("vault", {
  state: () => ({
    vaultState: null as VaultState | null,
    settings: null as AppSettings | null,
    entries: [] as EntrySummary[],
    entryTypes: [] as EntryType[],
    selected: null as Entry | null,
    options: defaultOptions(),
    busy: false,
    error: "",
    notice: ""
  }),

  getters: {
    unlocked: (state): boolean => Boolean(state.vaultState?.exists && !state.vaultState.locked),
    tags: (state): string[] =>
      [...new Set(state.entries.flatMap((entry) => entry.tags))].sort((a, b) =>
        a.localeCompare(b, "zh-CN")
      )
  },

  actions: {
    async run<T>(action: () => Promise<T>): Promise<T | undefined> {
      this.busy = true;
      this.error = "";
      try {
        return await action();
      } catch (error) {
        this.error = errorMessage(error);
        throw error;
      } finally {
        this.busy = false;
      }
    },

    async refreshState() {
      const state = await this.run(() => api.state());
      if (state) this.vaultState = state;
      if (state?.exists) this.settings = await api.settings();
      if (this.unlocked) {
        await this.loadTypes();
        await this.loadEntries();
      }
    },

    async initialize(masterPassword: string) {
      await this.run(() => api.initialize(masterPassword));
      this.vaultState = { exists: true, locked: false };
      await this.afterUnlock();
    },

    async restoreNew(path: string, backupPassword: string, masterPassword: string) {
      await this.run(() => api.restoreNew(path, backupPassword, masterPassword));
      this.vaultState = { exists: true, locked: false };
      await this.afterUnlock();
    },

    async unlock(masterPassword: string) {
      await this.run(() => api.unlock(masterPassword));
      this.vaultState = { exists: true, locked: false };
      await this.afterUnlock();
    },

    async afterUnlock() {
      const settings = await api.settings();
      this.settings = settings;
      await this.loadTypes();
      await this.loadEntries();
    },

    async lock() {
      await api.lock();
      this.vaultState = { exists: true, locked: true };
      this.entries = [];
      this.entryTypes = [];
      this.selected = null;
      this.options = defaultOptions();
    },

    async loadEntries() {
      try {
        const entries = await api.list(this.options);
        const selected = this.selected && entries.some((entry) => entry.id === this.selected?.id)
          ? this.selected
          : null;

        // Loading or filtering a list never creates an implicit selection. The
        // detail pane is exclusively controlled by an explicit row click.
        this.entries = entries;
        this.selected = selected;
      } catch (error) {
        const message = errorMessage(error);
        if (message.includes("锁定")) await this.lock();
        else this.error = message;
      }
    },

    async loadTypes() {
      try {
        this.entryTypes = await api.listTypes();
      } catch (error) {
        this.error = errorMessage(error);
      }
    },

    async createType(input: EntryTypeInput) {
      const created = await this.run(() => api.createType(input));
      if (created) {
        this.entryTypes.push(created);
        this.notice = t("类型已创建");
      }
      return created;
    },

    async removeType(id: string) {
      await this.run(() => api.removeType(id));
      this.entryTypes = this.entryTypes.filter((item) => item.id !== id);
      if (this.options.typeId === id) await this.setType(null);
      this.notice = t("类型已删除");
    },

    async toggleSelection(id: string) {
      if (this.selected?.id === id) {
        this.selected = null;
        return;
      }

      this.selected = await this.run(() => api.get(id)) ?? null;
    },

    async save(input: EntryInput, id?: string) {
      const entry = id
        ? await this.run(() => api.update(id, input))
        : await this.run(() => api.create(input));
      if (entry) {
        this.selected = entry;
        await this.loadEntries();
        this.notice = t(id ? "条目已更新" : "条目已保存");
      }
    },

    async remove(id: string) {
      await this.run(() => api.remove(id));
      this.selected = null;
      await this.loadEntries();
      this.notice = t("条目已删除");
    },

    async toggleFavorite(entry: Entry | EntrySummary) {
      const full = "password" in entry ? entry : await api.get(entry.id);
      await this.save({
        typeId: full.typeId,
        name: full.name,
        url: full.url,
        username: full.username,
        password: full.password,
        notes: full.notes,
        tags: full.tags,
        favorite: !full.favorite,
        customFields: full.customFields
      }, full.id);
    },

    async setQuery(query: string) {
      if (this.options.query === query) return;
      this.options.query = query;
      await this.loadEntries();
    },

    async setSort(sort: SortOrder) {
      if (this.options.sort === sort) return;
      this.options.sort = sort;
      await this.loadEntries();
    },

    async setSection(section: Partial<SectionOptions>) {
      const next: SectionOptions & Pick<ListOptions, "query"> = {
        query: "",
        favoriteOnly: false,
        typeId: null,
        tag: null,
        ...section
      };
      const unchanged = Object.entries(next).every(
        ([key, value]) => this.options[key as keyof typeof next] === value
      );
      if (unchanged) return;

      Object.assign(this.options, next);
      await this.loadEntries();
    },

    async setType(typeId: string | null) {
      await this.setSection({ typeId });
    },

    async setFavoriteOnly(favoriteOnly: boolean) {
      await this.setSection({ favoriteOnly });
    },

    async setTag(tag: string | null) {
      await this.setSection({ tag });
    },

    async saveSettings(settings: AppSettings) {
      const saved = await this.run(() => api.updateSettings(settings));
      if (saved) {
        this.settings = saved;
        this.notice = t("设置已保存");
      }
    }
  }
});
