import assert from "node:assert/strict";

import {
  getMermaidTheme,
  getNextTheme,
  getThemeToggleLabel,
  resolveInitialTheme
} from "../../src/theme.ts";

function runTest(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

runTest("resolveInitialTheme prefers a saved theme", () => {
  assert.equal(resolveInitialTheme("light", true), "light");
  assert.equal(resolveInitialTheme("dark", false), "dark");
});

runTest("resolveInitialTheme falls back to the system preference", () => {
  assert.equal(resolveInitialTheme(null, true), "dark");
  assert.equal(resolveInitialTheme(undefined, false), "light");
  assert.equal(resolveInitialTheme("unexpected", true), "dark");
});

runTest("getNextTheme toggles between dark and light", () => {
  assert.equal(getNextTheme("dark"), "light");
  assert.equal(getNextTheme("light"), "dark");
});

runTest("getThemeToggleLabel describes the next theme", () => {
  assert.equal(getThemeToggleLabel("dark"), "Switch to light theme");
  assert.equal(getThemeToggleLabel("light"), "Switch to dark theme");
});

runTest("getMermaidTheme maps the light palette to Mermaid's default theme", () => {
  assert.equal(getMermaidTheme("dark"), "dark");
  assert.equal(getMermaidTheme("light"), "default");
});
