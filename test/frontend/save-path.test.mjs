import assert from "node:assert/strict";

import { buildDefaultSavePath } from "../../src/save-path.ts";

function runTest(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

runTest("returns only the filename when no default directory is available", () => {
  assert.equal(buildDefaultSavePath(undefined, "Untitled.md"), "Untitled.md");
});

runTest("preserves a trailing Windows separator", () => {
  assert.equal(
    buildDefaultSavePath("C:\\Users\\me\\Documents\\", "Untitled.md"),
    "C:\\Users\\me\\Documents\\Untitled.md"
  );
});

runTest("preserves a trailing POSIX separator", () => {
  assert.equal(
    buildDefaultSavePath("/Users/me/Documents/", "Untitled.md"),
    "/Users/me/Documents/Untitled.md"
  );
});

runTest("joins Windows default directories with a Windows separator", () => {
  assert.equal(
    buildDefaultSavePath(String.raw`C:\Users\me\Documents`, "Untitled.md"),
    String.raw`C:\Users\me\Documents\Untitled.md`
  );
});

runTest("joins POSIX default directories with a POSIX separator", () => {
  assert.equal(
    buildDefaultSavePath("/Users/me/Documents", "Untitled.md"),
    "/Users/me/Documents/Untitled.md"
  );
});
