import { test, expect } from "vitest";
import { composeOutputTemplate, DEFAULT_OUTPUT_TEMPLATE } from "@/utils/output-template";

test("places static prefix and suffix around the filename before its extension", () => {
  expect(
    composeOutputTemplate("%(upload_date)s - %(title)s.%(ext)s", "[Archive] ", " [4K]"),
  ).toBe("[Archive] %(upload_date)s - %(title)s [4K].%(ext)s");
});

test("falls back to the shared default template", () => {
  expect(composeOutputTemplate("", "", "")).toBe(DEFAULT_OUTPUT_TEMPLATE);
});
