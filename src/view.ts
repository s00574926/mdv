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
  editorText: string | null;
  currentFilePath: string | null;
  explorer: ExplorerRoot | null;
  explorerUpdated: boolean;
  recentPaths: string[];
  documentTabs: DocumentTab[];
  activeDocumentIndex: number | null;
}

export interface DocumentTab {
  label: string;
  isUntitled: boolean;
  hasUnsavedContent: boolean;
  isActive: boolean;
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
  hidden: boolean;
  style?: StyleLike;
}

interface ExplorerPanelLike {
  hidden: boolean;
}

interface ExplorerTreeLike {
  innerHTML: string;
}

interface DocumentTabsPanelLike {
  hidden: boolean;
}

interface DocumentTabsLike {
  innerHTML: string;
}

interface EditorPanelLike {
  hidden: boolean;
}

interface EditorLike {
  value: string;
  readOnly?: boolean;
}

export interface ViewElements {
  appRoot: {
    classList: ClassListLike;
  };
  documentTabsPanel: DocumentTabsPanelLike;
  documentTabs: DocumentTabsLike;
  editorPanel: EditorPanelLike;
  editor: EditorLike;
  explorerPanel: ExplorerPanelLike;
  explorerTree: ExplorerTreeLike;
  preview: PreviewLike;
}

export function clearPreview(preview: PreviewLike): void {
  preview.innerHTML = "";
}

export function renderEditor(
  elements: Pick<ViewElements, "editorPanel" | "editor">,
  editorText: string | null
): void {
  if (editorText === null) {
    elements.editorPanel.hidden = true;
    if (elements.editor.value) {
      elements.editor.value = "";
    }
    return;
  }

  elements.editorPanel.hidden = false;
  if (elements.editor.value !== editorText) {
    elements.editor.value = editorText;
  }
}

export function isPreviewZoomShortcut(event: Pick<WheelEvent, "ctrlKey" | "metaKey">): boolean {
  return event.ctrlKey || event.metaKey;
}

export function clampPreviewScale(scale: number): number {
  if (!Number.isFinite(scale)) {
    return DEFAULT_PREVIEW_SCALE;
  }

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

export interface ContextMenuPositionRequest {
  left: number;
  top: number;
  menuWidth: number;
  menuHeight: number;
  viewportWidth: number;
  viewportHeight: number;
  margin?: number;
}

export function clampContextMenuPosition({
  left,
  top,
  menuWidth,
  menuHeight,
  viewportWidth,
  viewportHeight,
  margin = 8
}: ContextMenuPositionRequest): { left: number; top: number } {
  const maxLeft = Math.max(margin, viewportWidth - menuWidth - margin);
  const maxTop = Math.max(margin, viewportHeight - menuHeight - margin);

  return {
    left: Math.min(Math.max(left, margin), maxLeft),
    top: Math.min(Math.max(top, margin), maxTop)
  };
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
  return escapeHtml(value).replaceAll("\"", "&quot;");
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function renderExplorerNode(node: ExplorerNode, currentFilePath: string | null): string {
  if (node.kind === "directory") {
    const children = node.children.map((child) => renderExplorerNode(child, currentFilePath)).join("");
    return `
      <details class="tree-directory">
        <summary>${escapeHtml(node.name)}</summary>
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
      ${escapeHtml(node.name)}
    </button>
  `;
}

function sameExplorerNodes(left: ExplorerNode[], right: ExplorerNode[]): boolean {
  if (left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    const leftNode = left[index];
    const rightNode = right[index];

    if (
      leftNode.name !== rightNode.name ||
      leftNode.path !== rightNode.path ||
      leftNode.kind !== rightNode.kind ||
      !sameExplorerNodes(leftNode.children, rightNode.children)
    ) {
      return false;
    }
  }

  return true;
}

export function sameExplorer(
  left: ExplorerRoot | null | undefined,
  right: ExplorerRoot | null | undefined
): boolean {
  if (left === right) {
    return true;
  }

  if (!left || !right) {
    return left === right;
  }

  return left.name === right.name && left.path === right.path && sameExplorerNodes(left.children, right.children);
}

export function sameDocumentTabs(
  left: DocumentTab[] | undefined,
  right: DocumentTab[] | undefined
): boolean {
  if (left === right) {
    return true;
  }

  if (!left || !right || left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    const leftTab = left[index];
    const rightTab = right[index];

    if (
      leftTab.label !== rightTab.label ||
      leftTab.isUntitled !== rightTab.isUntitled ||
      leftTab.hasUnsavedContent !== rightTab.hasUnsavedContent ||
      leftTab.isActive !== rightTab.isActive
    ) {
      return false;
    }
  }

  return true;
}

export function hasUnsavedUntitledContent(documentTab: DocumentTab | null | undefined): boolean {
  return Boolean(documentTab?.isUntitled && documentTab?.hasUnsavedContent);
}

export function getUnsavedUntitledDocumentIndexes(documentTabs: DocumentTab[] | undefined): number[] {
  if (!documentTabs?.length) {
    return [];
  }

  return documentTabs.reduce<number[]>((indexes, documentTab, index) => {
    if (hasUnsavedUntitledContent(documentTab)) {
      indexes.push(index);
    }
    return indexes;
  }, []);
}

export function sameRecentPaths(left: string[] | undefined, right: string[] | undefined): boolean {
  if (left === right) {
    return true;
  }

  if (!left || !right || left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }

  return true;
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

export function renderDocumentTabs(
  elements: Pick<ViewElements, "documentTabsPanel" | "documentTabs">,
  documentTabs: DocumentTab[]
): void {
  if (!documentTabs.length) {
    elements.documentTabsPanel.hidden = true;
    elements.documentTabs.innerHTML = "";
    return;
  }

  elements.documentTabsPanel.hidden = false;
  elements.documentTabs.innerHTML = documentTabs
    .map(
      (documentTab, index) => `
        <div class="document-tab-item${documentTab.isActive ? " document-tab-active" : ""}">
          <button
            type="button"
            class="document-tab"
            data-document-index="${index}"
            aria-current="${documentTab.isActive ? "page" : "false"}"
            aria-label="${escapeAttribute(documentTab.label)}"
          >
            <span class="document-tab-label">${escapeAttribute(documentTab.label)}</span>
          </button>
          <button
            type="button"
            class="document-tab-close"
            data-close-document-index="${index}"
            aria-label="Close ${escapeAttribute(documentTab.label)} tab"
          >
            <svg class="document-tab-close-icon" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M4.25 4.25 11.75 11.75M11.75 4.25 4.25 11.75"
                stroke="currentColor"
                stroke-width="1.7"
                stroke-linecap="round"
              ></path>
            </svg>
          </button>
        </div>
      `
    )
    .join("");
}

export function shouldShowEditorPreview(workspace: WorkspacePayload): boolean {
  return workspace.editorText !== null && workspace.document.html.includes('class="mermaid"');
}

export function renderWorkspaceFrame(elements: ViewElements, workspace: WorkspacePayload): void {
  renderExplorer(elements, workspace.explorer, workspace.currentFilePath);
  renderDocumentTabs(elements, workspace.documentTabs);
  renderEditor(elements, workspace.editorText);

  if (workspace.editorText !== null) {
    elements.preview.hidden = true;
    clearPreview(elements.preview);
    return;
  }

  elements.preview.hidden = false;

  if (!workspace.document.html) {
    clearPreview(elements.preview);
    return;
  }

  setTrustedPreviewHtml(elements.preview, workspace.document);
}
