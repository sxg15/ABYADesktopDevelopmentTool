import { describe, expect, it } from "vitest";
import { localeKeys, normalizeLocale } from "./index";

describe("localization contracts", () => {
  it("keeps Chinese and English keys complete", () => {
    expect(localeKeys("zh-CN")).toEqual(localeKeys("en-US"));
  });

  it("defaults Chinese OS locales to Chinese", () => {
    expect(normalizeLocale("zh-Hans-CN")).toBe("zh-CN");
    expect(normalizeLocale("en-US")).toBe("en-US");
  });
});
