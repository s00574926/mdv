import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("../../package.json", import.meta.url), "utf8")
);
const tauriConfig = JSON.parse(
  readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8")
);

function runTest(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

runTest("package scripts avoid platform-specific npm command shims", () => {
  for (const [name, command] of Object.entries(packageJson.scripts ?? {})) {
    assert.doesNotMatch(
      command,
      /\bnpm\.cmd\b/i,
      `${name} script should use npm, not the Windows-only npm.cmd shim`
    );
  }
});

runTest("tauri build commands avoid platform-specific npm command shims", () => {
  for (const [name, command] of Object.entries(tauriConfig.build ?? {})) {
    if (typeof command !== "string") {
      continue;
    }

    assert.doesNotMatch(
      command,
      /\bnpm\.cmd\b/i,
      `${name} should use npm, not the Windows-only npm.cmd shim`
    );
  }
});
