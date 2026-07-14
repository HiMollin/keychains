import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { setLanguage } from "./i18n";
import "./styles.css";

const searchParams = new URLSearchParams(location.search);

if (import.meta.env.DEV && searchParams.has("preview")) {
  const previewState = searchParams.get("view") ?? "setup";
  const entryTypes = [
    { id: "personal", name: "个人账号", createdAt: Date.now() - 500_000 },
    { id: "work", name: "工作应用", createdAt: Date.now() - 400_000 },
    { id: "network", name: "网络设备", createdAt: Date.now() - 300_000 }
  ];
  const entries = [
    { id: "1", typeId: "personal", name: "Apple ID", url: "https://appleid.apple.com", username: "mollin@example.com", tags: ["个人"], favorite: true, createdAt: Date.now() - 86_400_000, updatedAt: Date.now() },
    { id: "2", typeId: "work", name: "GitHub", url: "https://github.com", username: "mollin", tags: ["开发"], favorite: true, createdAt: Date.now() - 172_800_000, updatedAt: Date.now() - 3_600_000 },
    { id: "3", typeId: "work", name: "Figma", url: "", username: "design@example.com", tags: ["工作"], favorite: false, createdAt: Date.now() - 259_200_000, updatedAt: Date.now() - 7_200_000 },
    { id: "4", typeId: "network", name: "家庭 Wi-Fi", url: "", username: "Home Network", tags: ["家庭"], favorite: false, createdAt: Date.now() - 345_600_000, updatedAt: Date.now() - 86_400_000 }
  ];
  const fullEntries = entries.map((entry) => ({
    ...entry,
    password: "Preview-Only-2026!",
    notes: entry.id === "1" ? "仅用于本地界面预览的示例条目。" : "",
    customFields: []
  }));
  const previewWindow = window as unknown as {
    __TAURI_INTERNALS__: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown> };
  };
  previewWindow.__TAURI_INTERNALS__ = {
    invoke: async (command, args) => {
      if (command === "get_vault_state") {
        return previewState === "setup"
          ? { exists: false, locked: true }
          : { exists: true, locked: previewState === "locked" };
      }
      if (command === "get_settings") return { autoLockMinutes: 5, clipboardClearSeconds: 30, theme: "system", language: "system" };
      if (command === "list_entry_types") return entryTypes;
      if (command === "list_entries") return entries;
      if (command === "get_entry") return fullEntries.find((entry) => entry.id === args?.id) ?? fullEntries[0];
      return undefined;
    }
  };
}

const savedTheme = localStorage.getItem("keychains-theme");
document.documentElement.dataset.theme =
  savedTheme === "light" || savedTheme === "dark" || savedTheme === "system"
    ? savedTheme
    : "system";
setLanguage("system");

createApp(App).use(createPinia()).mount("#app");
