import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setLanguage } from "../i18n";
import type { AppSettings } from "../types";
import SettingsDialog from "./SettingsDialog.vue";

const dialogMocks = vi.hoisted(() => ({
  open: vi.fn(),
  save: vi.fn()
}));

const apiMocks = vi.hoisted(() => ({
  changeMasterPassword: vi.fn(),
  exportBackup: vi.fn(),
  inspectBackup: vi.fn(),
  importBackup: vi.fn()
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: dialogMocks.open,
  save: dialogMocks.save
}));

vi.mock("../api", () => ({ api: apiMocks }));

const settings: AppSettings = {
  theme: "system",
  language: "system",
  autoLockMinutes: 10,
  clipboardClearSeconds: 30
};

function findButton(wrapper: VueWrapper, label: string) {
  const button = wrapper.findAll("button").find((item) => item.text().includes(label));
  expect(button, `button containing “${label}”`).toBeTruthy();
  return button!;
}

async function openBackupTab(wrapper: VueWrapper) {
  await findButton(wrapper, "备份与恢复").trigger("click");
}

describe("SettingsDialog notifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setLanguage("zh-CN");
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  it("reports backup validation errors through the shared notification event", async () => {
    const wrapper = mount(SettingsDialog, { props: { settings } });
    await openBackupTab(wrapper);

    await findButton(wrapper, "导出加密备份").trigger("click");

    expect(wrapper.emitted("notify")).toEqual([
      ["请输入并确认备份密码", "error"]
    ]);
    expect(wrapper.find(".settings-message").exists()).toBe(false);
  });

  it("reports export success and clears the backup password", async () => {
    dialogMocks.save.mockResolvedValue("C:\\vault.kcbak");
    apiMocks.exportBackup.mockResolvedValue({ entryCount: 3 });
    const wrapper = mount(SettingsDialog, { props: { settings } });
    await openBackupTab(wrapper);
    const passwordInputs = wrapper.findAll<HTMLInputElement>('input[type="password"]');
    expect(passwordInputs).toHaveLength(2);
    await passwordInputs[0]!.setValue("backup-secret");
    await passwordInputs[1]!.setValue("backup-secret");

    await findButton(wrapper, "导出加密备份").trigger("click");
    await flushPromises();

    expect(apiMocks.exportBackup).toHaveBeenCalledWith("C:\\vault.kcbak", "backup-secret");
    expect(wrapper.emitted("notify")).toEqual([
      ["已加密备份 3 个条目", "success"]
    ]);
    expect(passwordInputs[0]!.element.value).toBe("");
    expect(passwordInputs[1]!.element.value).toBe("");
  });

  it("reports import success, refreshes entries, and clears the password", async () => {
    dialogMocks.open.mockResolvedValue("C:\\vault.kcbak");
    apiMocks.inspectBackup.mockResolvedValue({ entryCount: 4 });
    apiMocks.importBackup.mockResolvedValue({ imported: 3, skipped: 1 });
    const wrapper = mount(SettingsDialog, { props: { settings } });
    await openBackupTab(wrapper);
    const passwordInputs = wrapper.findAll<HTMLInputElement>('input[type="password"]');
    await passwordInputs[0]!.setValue("backup-secret");

    await findButton(wrapper, "合并导入").trigger("click");
    await flushPromises();

    expect(apiMocks.importBackup).toHaveBeenCalledWith("C:\\vault.kcbak", "backup-secret", "merge");
    expect(wrapper.emitted("notify")).toEqual([
      ["已导入 3 个条目，跳过 1 个", "success"]
    ]);
    expect(wrapper.emitted("imported")).toHaveLength(1);
    expect(passwordInputs[0]!.element.value).toBe("");
  });

  it("reports import failures through the shared error toast", async () => {
    dialogMocks.open.mockResolvedValue("C:\\vault.kcbak");
    apiMocks.inspectBackup.mockRejectedValue("备份密码错误或文件已损坏");
    const wrapper = mount(SettingsDialog, { props: { settings } });
    await openBackupTab(wrapper);
    await wrapper.findAll<HTMLInputElement>('input[type="password"]')[0]!.setValue("wrong-secret");

    await findButton(wrapper, "合并导入").trigger("click");
    await flushPromises();

    expect(wrapper.emitted("notify")).toEqual([
      ["备份密码错误或文件已损坏", "error"]
    ]);
    expect(wrapper.emitted("imported")).toBeUndefined();
  });
});
