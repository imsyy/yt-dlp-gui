import { describe, expect, it, vi } from "vitest";

// format.ts 顶层导入 @/locales（读写 document），node 单测环境用桩替掉
vi.mock("@/locales", () => ({
  default: { global: { t: (key: string) => key, locale: { value: "en-US" } } },
}));

import { formatDateYMD, formatDuration } from "@/utils/format";

describe("formatDuration", () => {
  it("formats m:ss without hour padding", () => {
    expect(formatDuration(61)).toBe("1:01");
    expect(formatDuration(0)).toBe("");
    expect(formatDuration(null)).toBe("");
    expect(formatDuration(undefined)).toBe("");
    expect(formatDuration(-5)).toBe("");
    expect(formatDuration(NaN)).toBe("");
  });

  it("formats h:mm:ss with unpadded hours", () => {
    expect(formatDuration(3661)).toBe("1:01:01");
    expect(formatDuration(3723.9)).toBe("1:02:03");
  });

  it("supports custom empty placeholder", () => {
    expect(formatDuration(0, "0:00")).toBe("0:00");
    expect(formatDuration(null, "-")).toBe("-");
  });
});

describe("formatDateYMD", () => {
  it("formats millisecond timestamps as YYYY-MM-DD", () => {
    // 2025-09-19 00:00:00 UTC may shift by timezone; build from local parts instead
    const date = new Date(2025, 8, 19, 12, 0, 0);
    expect(formatDateYMD(date.getTime())).toBe("2025-09-19");
  });

  it("returns empty string for missing values", () => {
    expect(formatDateYMD(null)).toBe("");
    expect(formatDateYMD(undefined)).toBe("");
    expect(formatDateYMD(0)).toBe("");
  });
});
