import "./styles.css";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const WORKSPACE_UPDATED_EVENT = "workspace://updated";

type MermaidInstance = (typeof import("mermaid"))["default"];

interface RenderedDocument {
  title: string;
  html: string;
  sourceName: string;
  sourcePath: string;
  watching: boolean;
}

interface ExplorerNode {
  name: string;
  path: string;
  kind: "directory" | "file";
  children: ExplorerNode[];
}

interface ExplorerRoot {
  name: string;
  path: string;
  children: ExplorerNode[];
}

interface WorkspacePayload {
  document: RenderedDocument;
  currentFilePath: string | null;
  explorer: ExplorerRoot | null;
  recentPaths: string[];
}

let mermaidInstancePromise: Promise<MermaidInstance> | undefined;

function queryRequired<TElement extends Element>(selector: string): TElement {
  const element = document.querySelector<TElement>(selector);
  if (!element) {
    throw new Error(`Missing required element: ${selector}`);
  }

  return element;
}

const elements = {
  appRoot: queryRequired<HTMLElement>("#app-root"),
  explorerPanel: queryRequired<HTMLElement>("#explorer-panel"),
  explorerTree: queryRequired<HTMLElement>("#explorer-tree"),
  preview: queryRequired<HTMLElement>("#preview")
};

async function getMermaid(): Promise<MermaidInstance> {
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

function setBusyState(isBusy: boolean): void {
  document.querySelectorAll<HTMLButtonElement>(".tree-file-button").forEach((button) => {
    button.disabled = isBusy;
  });
}

async function renderMermaid(): Promise<void> {
  const nodes = elements.preview.querySelectorAll<HTMLElement>(".mermaid");

  if (!nodes.length) {
    return;
  }

  const mermaid = await getMermaid();
  await mermaid.run({ nodes });
}

function clearPreview(): void {
  elements.preview.innerHTML = "";
}

async function renderDocument(documentPayload: RenderedDocument): Promise<void> {
  if (!documentPayload.html) {
    clearPreview();
    return;
  }

  elements.preview.innerHTML = documentPayload.html;
  await renderMermaid();
}

function escapeAttribute(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("\"", "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function renderExplorerNode(node: ExplorerNode, currentFilePath: string | null): string {
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

function renderExplorer(explorer: ExplorerRoot | null, currentFilePath: string | null): void {
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

async function renderWorkspace(workspace: WorkspacePayload): Promise<void> {
  renderExplorer(workspace.explorer, workspace.currentFilePath);
  await renderDocument(workspace.document);
}

elements.explorerTree.addEventListener("click", async (event: MouseEvent) => {
  const target = event.target;
  if (!(target instanceof Element)) {
    return;
  }

  const button = target.closest<HTMLElement>("[data-file-path]");
  if (!button) {
    return;
  }

  const path = button.dataset.filePath;
  if (!path) {
    return;
  }

  setBusyState(true);

  try {
    const workspace = await invoke<WorkspacePayload>("select_explorer_file", { path });
    await renderWorkspace(workspace);
  } catch (error) {
    console.error(error);
  } finally {
    setBusyState(false);
  }
});

await listen<WorkspacePayload>(WORKSPACE_UPDATED_EVENT, async (event) => {
  try {
    await renderWorkspace(event.payload);
  } catch (error) {
    console.error(error);
  }
});

clearPreview();
