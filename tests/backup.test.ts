import assert from "node:assert/strict";
import test from "node:test";
import { backupName } from "../src/backup.ts";

test("the button shows Off or the folder name", () => {
  assert.equal(backupName(""), "Off");
  assert.equal(backupName("  "), "Off");
  assert.equal(
    backupName("/Users/me/Library/CloudStorage/GoogleDrive-me/My Drive/Lumi"),
    "Lumi",
  );
});
