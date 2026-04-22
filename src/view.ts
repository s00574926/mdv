export const TRUSTED_PREVIEW_TRUST_MODEL = "trusted-local-markdown-preview";
export const DEFAULT_PREVIEW_SCALE = 1;
export const MIN_PREVIEW_SCALE = 0.5;
export const MAX_PREVIEW_SCALE = 2.5;

const PREVIEW_SCALE_STEP = 0.1;

export interface RenderedDocument {
  title: string;
  html: string;
  sourceName: string;
  sourcePath: string;
  watching: boolean;
  trustModel: typeof TRUSTED_PREVIEW_TRUST_MODEL;
}

export interface ExplorerNode {
  name: string;
  path: string;
  kind: "directory" | "file";
  children: ExplorerNode[];
}

export interface ExplorerRoot {
  name: string;
  path: string;
  children: ExplorerNode[];
}

export interface WorkspacePayload {
  document: RenderedDocument;
  currentFilePath: string | null;
  explorer: ExplorerRoot | null;
  recentPaths: string[];
}

interface ClassListLike {
  add(token: string): void;
  remove(token: string): void;
}

interface StyleLike {
  setProperty(name: string, value: string): void;
}

interface PreviewLike {
  innerHTML: string;
  style?: StyleLike;
}

interface ExplorerPanelLike {
  hidden: boolean;
}

interface ExplorerTreeLike {
  innerHTML: string;
}

export interface ViewElements {
  appRoot: {
    classList: ClassListLike;
  };
  explorerPanel: ExplorerPanelLike;
  explorerTree: ExplorerTreeLike;
  preview: PreviewLike;
}

export function clearPreview(preview: PreviewLike): void {
  preview.innerHTML = "";
}

export function isPreviewZoomShortcut(event: Pick<WheelEvent, "ctrlKey" | "metaKey">): boolean {
  return event.ctrlKey || event.metaKey;
}

export function clampPreviewScale(scale: number): number {
  return Number(Math.min(MAX_PREVIEW_SCALE, Math.max(MIN_PREVIEW_SCALE, scale)).toFixed(2));
}

export function getNextPreviewScale(currentScale: number, deltaY: number): number {
  if (deltaY === 0) {
    return clampPreviewScale(currentScale);
  }

  const delta = deltaY < 0 ? PREVIEW_SCALE_STEP : -PREVIEW_SCALE_STEP;
  return clampPreviewScale(currentScale + delta);
}

export function applyPreviewScale(preview: PreviewLike, scale: number): number {
  const nextScale = clampPreviewScale(scale);
  preview.style?.setProperty("--preview-scale", nextScale.toFixed(2));
  return nextScale;
}

export function setTrustedPreviewHtml(
  preview: PreviewLike,
  documentPayload: RenderedDocument
): void {
  if (documentPayload.trustModel !== TRUSTED_PREVIEW_TRUST_MODEL) {
    throw new Error(`Unexpected preview trust model: ${documentPayload.trustModel}`);
  }

  // mdv only injects HTML that crossed the explicit trusted preview boundary
  // on the Rust side. Untrusted Markdown must not be routed through this path.
  preview.innerHTML = documentPayload.html;
}

export function escapeAttribute(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("\"", "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function renderExplorerNode(node: ExplorerNode, currentFilePath: string | null): string {
  if (node.kind === "directory") {
    const children = node.children.map((child) => renderExplorerNode(child, currentFilePath)).join("");
    return `
      <details class="tree-directory">
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

export function renderExplorer(
  elements: Pick<ViewElements, "appRoot" | "explorerPanel" | "explorerTree">,
  explorer: ExplorerRoot | null,
  currentFilePath: string | null
): void {
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

export function renderWorkspaceFrame(elements: ViewElements, workspace: WorkspacePayload): void {
  renderExplorer(elements, workspace.explorer, workspace.currentFilePath);

  if (!workspace.document.html) {
    clearPreview(elements.preview);
    return;
  }

  setTrustedPreviewHtml(elements.preview, workspace.document);
}
