import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BackupPreview,
  Entry,
  EntryInput,
  EntrySummary,
  EntryType,
  EntryTypeInput,
  ImportReport,
  ListOptions,
  PasswordOptions,
  VaultState
} from "./types";

export const api = {
  state: () => invoke<VaultState>("get_vault_state"),
  initialize: (masterPassword: string) => invoke<void>("initialize_vault", { masterPassword }),
  restoreNew: (path: string, backupPassword: string, newMasterPassword: string) =>
    invoke<void>("restore_new_vault", { path, backupPassword, newMasterPassword }),
  unlock: (masterPassword: string) => invoke<void>("unlock_vault", { masterPassword }),
  lock: () => invoke<void>("lock_vault"),
  list: (options: ListOptions) => invoke<EntrySummary[]>("list_entries", { options }),
  listTypes: () => invoke<EntryType[]>("list_entry_types"),
  createType: (input: EntryTypeInput) => invoke<EntryType>("create_entry_type", { input }),
  removeType: (id: string) => invoke<void>("delete_entry_type", { id }),
  get: (id: string) => invoke<Entry>("get_entry", { id }),
  create: (input: EntryInput) => invoke<Entry>("create_entry", { input }),
  update: (id: string, input: EntryInput) => invoke<Entry>("update_entry", { id, input }),
  remove: (id: string) => invoke<void>("delete_entry", { id }),
  settings: () => invoke<AppSettings>("get_settings"),
  updateSettings: (settings: AppSettings) =>
    invoke<AppSettings>("update_settings", { settings }),
  changeMasterPassword: (currentPassword: string, newPassword: string) =>
    invoke<void>("change_master_password", { currentPassword, newPassword }),
  generatePassword: (options: PasswordOptions) =>
    invoke<string>("generate_password", { options }),
  copyField: (id: string, field: string) =>
    invoke<void>("copy_entry_field", { id, field }),
  exportBackup: (path: string, backupPassword: string) =>
    invoke<BackupPreview>("export_backup", { path, backupPassword }),
  inspectBackup: (path: string, backupPassword: string) =>
    invoke<BackupPreview>("inspect_backup", { path, backupPassword }),
  importBackup: (path: string, backupPassword: string, mode: "merge" | "replace") =>
    invoke<ImportReport>("import_backup", { path, backupPassword, mode })
};
