export type GlyphName =
  | "sidebar"
  | "vault"
  | "star"
  | "globe"
  | "app"
  | "diamond"
  | "folder"
  | "tag"
  | "settings"
  | "lock"
  | "search"
  | "plus"
  | "minimize"
  | "maximize"
  | "close";
export type Theme = "system" | "light" | "dark";
export type Language = "system" | "zh-CN" | "en";
export type SortOrder =
  | "updatedDesc"
  | "createdDesc"
  | "nameAsc"
  | "nameDesc"
  | "usernameAsc";

export interface CustomField {
  id: string;
  label: string;
  value: string;
  secret: boolean;
}

export interface EntryInput {
  typeId: string;
  name: string;
  url: string;
  username: string;
  password: string;
  notes: string;
  tags: string[];
  favorite: boolean;
  customFields: CustomField[];
}

export interface Entry extends EntryInput {
  id: string;
  createdAt: number;
  updatedAt: number;
}

export interface EntrySummary {
  id: string;
  typeId: string;
  name: string;
  url: string;
  username: string;
  tags: string[];
  favorite: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface EntryTypeInput {
  name: string;
}

export interface EntryType extends EntryTypeInput {
  id: string;
  createdAt: number;
}

export interface VaultState {
  exists: boolean;
  locked: boolean;
}

export interface AppSettings {
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  theme: Theme;
  language: Language;
}

export interface ListOptions {
  query: string;
  sort: SortOrder;
  favoriteOnly: boolean;
  typeId: string | null;
  tag: string | null;
}

export interface PasswordOptions {
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
}

export interface BackupPreview {
  entryCount: number;
  exportedAt: number;
}

export interface ImportReport {
  imported: number;
  skipped: number;
}

export interface CommandError {
  code?: string;
  message?: string;
}
