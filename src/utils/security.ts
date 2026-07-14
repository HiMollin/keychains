import type { CommandError } from "../types";
import { currentLocale, t } from "../i18n";

export interface PasswordStrength {
  score: number;
  label: string;
}

export function passwordStrength(password: string): PasswordStrength {
  if (!password) return { score: 0, label: "未输入" };
  let score = 0;
  if (password.length >= 8) score += 1;
  if (password.length >= 12) score += 1;
  if (password.length >= 16) score += 1;
  const groups = [/[a-z]/, /[A-Z]/, /\d/, /[^\w\s]/].filter((pattern) => pattern.test(password)).length;
  if (groups >= 3) score += 1;
  score = Math.min(4, score);
  return {
    score,
    label: ["很弱", "较弱", "一般", "较强", "很强"][score] ?? "未知"
  };
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return t(error);
  if (error && typeof error === "object") {
    const commandError = error as CommandError;
    if (commandError.message) return t(commandError.message);
  }
  return t("操作失败，请稍后重试");
}

export function formatDate(timestamp: number): string {
  return new Intl.DateTimeFormat(currentLocale(), {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit"
  }).format(new Date(timestamp));
}

export function initials(name: string): string {
  const value = name.trim();
  return value ? value.slice(0, 2).toUpperCase() : "?";
}
