import assert from "node:assert/strict";

import {
  getShortcutAction,
  getShortcutLabel,
  isMacLikePlatform
} from "../../src/shortcuts.ts";

function runTest(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

runTest("isMacLikePlatform detects Apple platforms", () => {
  assert.equal(isMacLikePlatform("MacIntel"), true);
  assert.equal(isMacLikePlatform("Win32"), false);
});

runTest("getShortcutAction maps standard command shortcuts", () => {
  assert.equal(
    getShortcutAction({ key: "n", ctrlKey: true, metaKey: false, altKey: false, shiftKey: false }),
    "new"
  );
  assert.equal(
    getShortcutAction({ key: "O", ctrlKey: true, metaKey: false, altKey: false, shiftKey: true }),
    "open-folder"
  );
  assert.equal(
    getShortcutAction({ key: "Tab", ctrlKey: true, metaKey: false, altKey: false, shiftKey: true }),
    "previous-tab"
  );
  assert.equal(
    getShortcutAction({ key: "w", ctrlKey: false, metaKey: true, altKey: false, shiftKey: false }),
    "close-tab"
  );
  assert.equal(
    getShortcutAction({ key: "s", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }),
    undefined
  );
});

runTest("getShortcutLabel uses platform-aware primary modifiers", () => {
  assert.equal(getShortcutLabel("save", false), "Ctrl+S");
  assert.equal(getShortcutLabel("save-as", true), "Cmd+Shift+S");
  assert.equal(getShortcutLabel("next-tab", true), "Ctrl+Tab");
});
