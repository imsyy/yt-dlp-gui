import { expect, test } from "vitest";
import { isValidTimeRange } from "@/utils/validate";

test("allows full videos, open-ended clips and clips ending at the known duration", () => {
  expect(isValidTimeRange(null, null, 60)).toBe(true);
  expect(isValidTimeRange(10, null, 60)).toBe(true);
  expect(isValidTimeRange(null, 60, 60)).toBe(true);
  expect(isValidTimeRange(10, 60, 60)).toBe(true);
  expect(isValidTimeRange(100, 200, 0)).toBe(true);
});

test("rejects clips outside the video and reversed, negative or invalid ranges", () => {
  expect(isValidTimeRange(60, null, 60)).toBe(false);
  expect(isValidTimeRange(0, 61, 60)).toBe(false);
  expect(isValidTimeRange(20, 10)).toBe(false);
  expect(isValidTimeRange(20, 20)).toBe(false);
  expect(isValidTimeRange(-1, 20)).toBe(false);
  expect(isValidTimeRange(NaN, 20)).toBe(false);
  expect(isValidTimeRange(0, Infinity)).toBe(false);
});
