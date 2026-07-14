import { describe, expect, it } from "vitest";
import { errorMessage, initials, passwordStrength } from "./security";

describe("passwordStrength", () => {
  it("keeps weak passwords allowed but clearly labels them", () => {
    expect(passwordStrength("abc").score).toBeLessThan(2);
  });

  it("recognizes long mixed passwords", () => {
    expect(passwordStrength("A-longer!password-2026").score).toBe(4);
  });
});

describe("display helpers", () => {
  it("extracts compact initials", () => {
    expect(initials("GitHub")).toBe("GI");
    expect(initials("邮箱")).toBe("邮箱");
  });

  it("reads structured command errors", () => {
    expect(errorMessage({ code: "TEST", message: "发生错误" })).toBe("发生错误");
  });
});
