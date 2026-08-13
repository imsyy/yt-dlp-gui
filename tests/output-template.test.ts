import assert from "node:assert/strict";
import test from "node:test";
import { composeOutputTemplate, DEFAULT_OUTPUT_TEMPLATE } from "../src/utils/output-template.ts";

test("places static prefix and suffix around the filename before its extension", () => {
  assert.equal(
    composeOutputTemplate("%(upload_date)s - %(title)s.%(ext)s", "[Archive] ", " [4K]"),
    "[Archive] %(upload_date)s - %(title)s [4K].%(ext)s",
  );
});

test("falls back to the shared default template", () => {
  assert.equal(composeOutputTemplate("", "", ""), DEFAULT_OUTPUT_TEMPLATE);
});
