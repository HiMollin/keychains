import { afterEach, describe, expect, it } from "vitest";
import { setLanguage, t } from "./i18n";

describe("i18n", () => {
  afterEach(() => setLanguage("system"));

  it("switches between Chinese and English", () => {
    setLanguage("en");
    expect(t("设置")).toBe("Settings");
    expect(document.documentElement.lang).toBe("en");

    setLanguage("zh-CN");
    expect(t("设置")).toBe("设置");
    expect(document.documentElement.lang).toBe("zh-CN");
  });

  it("interpolates translated messages", () => {
    setLanguage("en");
    expect(t("标签：{tag}", { tag: "Work" })).toBe("Tag: Work");
  });
});
