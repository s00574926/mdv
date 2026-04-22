import "./styles.css";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  clearPreview,
  renderWorkspaceFrame,
  type WorkspacePayload
} from "./view";

const WORKSPACE_UPDATED_EVENT = "workspace://updated";

type MermaidInstance = (typeof import("mermaid"))["default"];

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
        securityLevel: "antiscript",
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

async function renderWorkspace(workspace: WorkspacePayload): Promise<void> {
  renderWorkspaceFrame(elements, workspace);

  if (!workspace.document.html) {
    return;
  }

  await renderMermaid();
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

clearPreview(elements.preview);
