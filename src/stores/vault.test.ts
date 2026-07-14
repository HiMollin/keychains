import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { api } from "../api";
import type { Entry, EntrySummary } from "../types";
import { useVaultStore } from "./vault";

vi.mock("../api", () => ({
  api: {
    list: vi.fn(),
    get: vi.fn()
  }
}));

const website: Entry = {
  id: "website-1",
  typeId: "personal",
  name: "网站账号",
  url: "https://example.com",
  username: "user@example.com",
  password: "secret",
  notes: "",
  tags: [],
  favorite: true,
  customFields: [],
  createdAt: 1,
  updatedAt: 1
};

const appEntry: Entry = {
  ...website,
  id: "app-1",
  typeId: "work",
  name: "应用账号",
  url: "",
  favorite: false
};

const summary = (entry: Entry): EntrySummary => {
  const { password: _password, notes: _notes, customFields: _customFields, ...value } = entry;
  return value;
};

describe("vault section filters", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("clears a selection that is not present after switching sections", async () => {
    const store = useVaultStore();
    store.entries = [summary(website)];
    store.selected = website;
    store.options.query = "旧搜索";
    store.options.favoriteOnly = true;

    let resolveList!: (entries: EntrySummary[]) => void;
    vi.mocked(api.list).mockReturnValue(new Promise((resolve) => { resolveList = resolve; }));

    const switching = store.setType("work");

    expect(store.options).toMatchObject({ query: "", typeId: "work", favoriteOnly: false, tag: null });
    expect(api.list).toHaveBeenCalledTimes(1);
    expect(store.entries).toEqual([summary(website)]);
    expect(store.selected).toEqual(website);

    resolveList([summary(appEntry)]);
    await switching;

    expect(store.entries).toEqual([summary(appEntry)]);
    expect(store.selected).toBeNull();
    expect(api.get).not.toHaveBeenCalled();

    // The debounced query watcher may repeat the cleared query; it must not reload.
    await store.setQuery("");
    expect(api.list).toHaveBeenCalledTimes(1);
  });

  it("does not select the first entry when a list is loaded", async () => {
    const store = useVaultStore();
    vi.mocked(api.list).mockResolvedValue([summary(website), summary(appEntry)]);

    await store.loadEntries();

    expect(store.entries).toEqual([summary(website), summary(appEntry)]);
    expect(store.selected).toBeNull();
    expect(api.get).not.toHaveBeenCalled();
  });

  it("toggles the same entry between selected and unselected", async () => {
    const store = useVaultStore();
    store.entries = [summary(website)];
    vi.mocked(api.get).mockResolvedValue(website);

    await store.toggleSelection(website.id);
    expect(store.selected).toEqual(website);
    expect(api.get).toHaveBeenCalledTimes(1);

    await store.toggleSelection(website.id);
    expect(store.selected).toBeNull();
    expect(api.get).toHaveBeenCalledTimes(1);
  });
});
