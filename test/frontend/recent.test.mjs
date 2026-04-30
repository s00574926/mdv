import assert from "node:assert/strict";

import { normalizeRecentPath, recentFileName, recentMenuLabel } from "../../src/recent.ts";

function runTest(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

runTest("normalizeRecentPath strips the Windows extended path prefix", () => {
  assert.equal(normalizeRecentPath(String.raw`\\?\C:\docs\plan.md`), String.raw`C:\docs\plan.md`);
});

runTest("normalizeRecentPath restores UNC paths cleanly", () => {
  assert.equal(
    normalizeRecentPath(String.raw`\\?\UNC\server\share\roadmap.md`),
    String.raw`\\server\share\roadmap.md`
  );
});

runTest("normalizeRecentPath handles mixed-case Windows extended prefixes", () => {
  assert.equal(normalizeRecentPath(String.raw`\\?\c:\docs\plan.md`), String.raw`c:\docs\plan.md`);
  assert.equal(
    normalizeRecentPath(String.raw`\\?\unc\server\share\roadmap.md`),
    String.raw`\\server\share\roadmap.md`
  );
});

runTest("recentFileName returns only the filename", () => {
  assert.equal(recentFileName(String.raw`\\?\C:\docs\plan.md`), "plan.md");
  assert.equal(recentFileName("C:/docs/notes.md"), "notes.md");
});

runTest("recentMenuLabel prefixes the filename with a sequence number", () => {
  assert.equal(recentMenuLabel(String.raw`\\?\C:\docs\plan.md`, 0), "1. plan.md");
  assert.equal(recentMenuLabel("C:/docs/notes.md", 2), "3. notes.md");
});
