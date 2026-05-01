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

function viteDevServerAddress(command) {
  const host = command.match(/--host\s+([^\s]+)/)?.[1];
  const port = command.match(/--port\s+([^\s]+)/)?.[1];
  return { host, port };
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

runTest("tauri devUrl matches the Vite dev server bind address", () => {
  const devScript = packageJson.scripts?.dev ?? "";
  const devUrl = new URL(tauriConfig.build?.devUrl ?? "");
  const { host, port } = viteDevServerAddress(devScript);

  assert.equal(devUrl.hostname, host);
  assert.equal(devUrl.port, port);
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

runTest("ubuntu workflow runs the frontend quality gate", () => {
  for (const command of ["npm ci", "npm run typecheck", "npm run build", "npm test"]) {
    assert.match(
      rustWorkflow,
      new RegExp(command.replaceAll(" ", "\\s+")),
      `Rust workflow should run ${command}`
    );
  }
});

runTest("ubuntu workflow runs the Rust lint gate", () => {
  assert.match(
    rustWorkflow,
    /cargo\s+clippy\s+--manifest-path\s+src-tauri\/Cargo\.toml\s+--all-targets\s+--\s+-D\s+warnings/,
    "Rust workflow should fail on clippy warnings from the Tauri crate"
  );
});
