import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(
  readFileSync(new URL("../../package.json", import.meta.url), "utf8")
);
const tauriConfig = JSON.parse(
  readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8")
);
const rustWorkflow = readFileSync(
  new URL("../../.github/workflows/rust.yml", import.meta.url),
  "utf8"
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

runTest("ubuntu rust workflow installs Tauri Linux system dependencies", () => {
  for (const packageName of [
    "libwebkit2gtk-4.1-dev",
    "libxdo-dev",
    "libssl-dev",
    "libayatana-appindicator3-dev",
    "librsvg2-dev"
  ]) {
    assert.match(
      rustWorkflow,
      new RegExp(`\\b${packageName}\\b`),
      `Rust workflow should install ${packageName} before building Tauri on Ubuntu`
    );
  }
});
