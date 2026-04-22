import "./styles.css";

import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const WORKSPACE_UPDATED_EVENT = "workspace://updated";
let mermaidInstancePromise;

const elements = {
  appRoot: document.querySelector("#app-root"),
  explorerPanel: document.querySelector("#explorer-panel"),
  explorerTree: document.querySelector("#explorer-tree"),
  preview: document.querySelector("#preview")
};

async function getMermaid() {
  if (!mermaidInstancePromise) {
    mermaidInstancePromise = import("mermaid").then(({ default: mermaid }) => {
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: "loose",
        theme: "dark"
      });

      return mermaid;
    });
  }

  return mermaidInstancePromise;
}

function setBusyState(isBusy) {
  document.querySelectorAll(".tree-file-button").forEach((button) => {
    button.disabled = isBusy;
  });
}

async function renderMermaid() {
  const nodes = elements.preview.querySelectorAll(".mermaid");

  if (!nodes.length) {
    return;
  }

  const mermaid = await getMermaid();
  await mermaid.run({ nodes });
}

function clearPreview() {
  elements.preview.innerHTML = "";
}

async function renderDocument(document) {
  if (!document.html) {
    clearPreview();
    return;
  }

  elements.preview.innerHTML = document.html;
  await renderMermaid();
}

function escapeAttribute(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("\"", "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function renderExplorerNode(node, currentFilePath) {
  if (node.kind === "directory") {
    const children = node.children.map((child) => renderExplorerNode(child, currentFilePath)).join("");
    return `
      <details class="tree-directory" open>
        <summary>${node.name}</summary>
        <div class="tree-children">${children}</div>
      </details>
    `;
  }

  const isActive = currentFilePath === node.path;
  return `
    <button
      type="button"
      class="tree-file-button${isActive ? " tree-file-button-active" : ""}"
      data-file-path="${escapeAttribute(node.path)}"
      role="treeitem"
      aria-current="${isActive ? "page" : "false"}"
    >
      ${node.name}
    </button>
  `;
}

function renderExplorer(explorer, currentFilePath) {
  if (!explorer) {
    elements.explorerPanel.hidden = true;
    elements.explorerTree.innerHTML = "";
    elements.appRoot.classList.remove("app-root-with-explorer");
    return;
  }

  elements.explorerPanel.hidden = false;
  elements.appRoot.classList.add("app-root-with-explorer");
  elements.explorerTree.innerHTML = explorer.children
    .map((node) => renderExplorerNode(node, currentFilePath))
    .join("");
}

async function renderWorkspace(workspace) {
  renderExplorer(workspace.explorer, workspace.currentFilePath);
  await renderDocument(workspace.document);
}

elements.explorerTree.addEventListener("click", async (event) => {
  const button = event.target.closest("[data-file-path]");
  if (!button) {
    return;
  }

  const path = button.dataset.filePath;
  if (!path) {
    return;
  }

  setBusyState(true);

  try {
    const workspace = await invoke("select_explorer_file", { path });
    await renderWorkspace(workspace);
  } catch (error) {
    console.error(error);
  } finally {
    setBusyState(false);
  }
});

await listen(WORKSPACE_UPDATED_EVENT, async (event) => {
  try {
    await renderWorkspace(event.payload);
  } catch (error) {
    console.error(error);
  }
});

clearPreview();
