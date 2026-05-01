import "./styles.css";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { documentDir } from "@tauri-apps/api/path";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { message, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  applyPreviewScale,
  clampContextMenuPosition,
  DEFAULT_PREVIEW_SCALE,
  clearPreview,
  getUnsavedUntitledDocumentIndexes,
  getNextPreviewScale,
  hasUnsavedUntitledContent,
  isPreviewZoomShortcut,
  renderDocumentTabs,
  renderEditor,
  renderExplorer,
  sameDocumentTabs,
  sameExplorer,
  sameRecentPaths,
  setTrustedPreviewHtml,
  shouldShowEditorPreview,
  type WorkspacePayload
} from "./view";
import {
  getShortcutAction,
  getShortcutLabel,
  isMacLikePlatform,
  type ShortcutAction
} from "./shortcuts";
import { buildDefaultSavePath } from "./save-path";
import {
  THEME_STORAGE_KEY,
  getMermaidTheme,
  getNextTheme,
  getThemeToggleLabel,
  resolveInitialTheme,
  type AppTheme
} from "./theme";
import { normalizeRecentPath, recentMenuLabel } from "./recent";

const WORKSPACE_UPDATED_EVENT = "workspace://updated";
const EDITOR_PREVIEW_CLASS = "app-root-with-editor-preview";
const EDITOR_UPDATE_DEBOUNCE_DELAY_MS = 150;

type MermaidInstance = (typeof import("mermaid"))["default"];
type ClipboardItemLike = typeof ClipboardItem;
interface MermaidPowerPointClipboardPayload {
  svg: string;
  width: number;
  height: number;
}
type WorkspaceCommand =
  | "new_document"
  | "close_document"
  | "open_folder"
  | "open_markdown_dialog"
  | "open_folder_dialog"
  | "open_recent_index"
  | "select_document"
  | "save_active_document"
  | "save_active_document_as"
  | "save_active_document_to_path";

let mermaidInstancePromise: Promise<MermaidInstance> | undefined;
let pendingTitlebarDrag:
  | {
      screenX: number;
      screenY: number;
    }
  | undefined;

const currentWindow = getCurrentWindow();
const isMacLike = isMacLikePlatform(window.navigator.platform);
let activeTheme = resolveInitialTheme(
  window.localStorage.getItem(THEME_STORAGE_KEY),
  window.matchMedia("(prefers-color-scheme: dark)").matches
);

document.documentElement.dataset.theme = activeTheme;

function queryRequired<TElement extends Element>(selector: string): TElement {
  const element = document.querySelector<TElement>(selector);
  if (!element) {
    throw new Error(`Missing required element: ${selector}`);
  }

  return element;
}

const elements = {
  appRoot: queryRequired<HTMLElement>("#app-root"),
  documentTabsPanel: queryRequired<HTMLElement>("#document-tabs-panel"),
  documentTabs: queryRequired<HTMLElement>("#document-tabs"),
  editorPanel: queryRequired<HTMLElement>("#editor-panel"),
  editor: queryRequired<HTMLTextAreaElement>("#editor"),
  explorerPanel: queryRequired<HTMLElement>("#explorer-panel"),
  explorerTree: queryRequired<HTMLElement>("#explorer-tree"),
  preview: queryRequired<HTMLElement>("#preview"),
  previewContextMenu: queryRequired<HTMLElement>("#preview-context-menu"),
  previewContextMenuCopyMermaid: queryRequired<HTMLButtonElement>("#preview-context-menu-copy-mermaid"),
  previewContextMenuCopyPowerPoint: queryRequired<HTMLButtonElement>("#preview-context-menu-copy-powerpoint"),
  titlebar: queryRequired<HTMLElement>(".titlebar"),
  titlebarMenuButton: queryRequired<HTMLButtonElement>("#titlebar-menu-button"),
  titlebarMenu: queryRequired<HTMLElement>("#titlebar-menu"),
  titlebarMenuNew: queryRequired<HTMLButtonElement>("#titlebar-menu-new"),
  titlebarMenuOpen: queryRequired<HTMLButtonElement>("#titlebar-menu-open"),
  titlebarMenuOpenFolder: queryRequired<HTMLButtonElement>("#titlebar-menu-open-folder"),
  titlebarMenuOpenRecent: queryRequired<HTMLButtonElement>("#titlebar-menu-open-recent"),
  titlebarMenuQuit: queryRequired<HTMLButtonElement>("#titlebar-menu-quit"),
  titlebarMenuRecent: queryRequired<HTMLElement>("#titlebar-menu-recent"),
  titlebarMenuSave: queryRequired<HTMLButtonElement>("#titlebar-menu-save"),
  titlebarMenuSaveAs: queryRequired<HTMLButtonElement>("#titlebar-menu-save-as"),
  titlebarTheme: queryRequired<HTMLButtonElement>("#titlebar-theme"),
  titlebarMinimize: queryRequired<HTMLButtonElement>("#titlebar-minimize"),
  titlebarMaximize: queryRequired<HTMLButtonElement>("#titlebar-maximize"),
  titlebarMaximizeIcon: queryRequired<HTMLElement>("#titlebar-maximize-icon"),
  titlebarClose: queryRequired<HTMLButtonElement>("#titlebar-close")
};

let previewScale = applyPreviewScale(elements.preview, DEFAULT_PREVIEW_SCALE);
let editorUpdateChain = Promise.resolve();
let latestEditorUpdateId = 0;
let pendingEditorUpdateTimeoutId: number | undefined;
let pendingEditorUpdate:
  | {
      documentIndex: number;
      markdown: string;
      updateId: number;
    }
  | undefined;
let previewContextMenuTarget: SVGSVGElement | undefined;
let currentWorkspace: WorkspacePayload | undefined;
let pendingWindowCloseRequest: Promise<void> | undefined;
let appExitInProgress = false;

async function getMermaid(): Promise<MermaidInstance> {
  if (!mermaidInstancePromise) {
    mermaidInstancePromise = import("mermaid").then(({ default: mermaid }) => mermaid);
  }

  return mermaidInstancePromise;
}

function syncThemeToggleButton(): void {
  const label = getThemeToggleLabel(activeTheme);
  elements.titlebarTheme.setAttribute("aria-label", label);
  elements.titlebarTheme.title = label;
}

function getEditorBackgroundColor(): string {
  const editorBackgroundColor = getComputedStyle(elements.editor).backgroundColor;
  if (editorBackgroundColor && editorBackgroundColor !== "rgba(0, 0, 0, 0)" && editorBackgroundColor !== "transparent") {
    return editorBackgroundColor;
  }

  const bodyBackgroundColor = getComputedStyle(document.body).backgroundColor;
  if (bodyBackgroundColor && bodyBackgroundColor !== "rgba(0, 0, 0, 0)" && bodyBackgroundColor !== "transparent") {
    return bodyBackgroundColor;
  }

  return "#000000";
}

function capturePreviewViewState(): {
  scrollLeft: number;
  scrollTop: number;
  scale: number;
} {
  return {
    scrollLeft: elements.preview.scrollLeft,
    scrollTop: elements.preview.scrollTop,
    scale: previewScale
  };
}

function restorePreviewViewState(viewState: {
  scrollLeft: number;
  scrollTop: number;
  scale: number;
}): void {
  previewScale = applyPreviewScale(elements.preview, viewState.scale);
  elements.preview.scrollLeft = viewState.scrollLeft;
  elements.preview.scrollTop = viewState.scrollTop;
}

async function rerenderThemeSensitivePreview(): Promise<void> {
  if (!currentWorkspace || elements.preview.hidden || !elements.preview.querySelector(".mermaid")) {
    return;
  }

  const viewState = capturePreviewViewState();
  closePreviewContextMenu();
  setTrustedPreviewHtml(elements.preview, currentWorkspace.document);
  await renderMermaid();
  restorePreviewViewState(viewState);
}

async function setAppTheme(theme: AppTheme, options?: { persist?: boolean }): Promise<void> {
  const changed = activeTheme !== theme;
  activeTheme = theme;
  document.documentElement.dataset.theme = theme;
  syncThemeToggleButton();

  if (options?.persist !== false) {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  }

  if (changed) {
    await rerenderThemeSensitivePreview();
  }
}

function setBusyState(isBusy: boolean): void {
  document
    .querySelectorAll<HTMLButtonElement>(
      ".tree-file-button, .document-tab, .document-tab-close, .titlebar-menu-item, .titlebar-menu-button, .context-menu-item"
    )
    .forEach((button) => {
      button.disabled = isBusy;
    });
  elements.editor.readOnly = isBusy;
}

function setEditorPreviewMode(isEnabled: boolean): void {
  if (isEnabled) {
    elements.appRoot.classList.add(EDITOR_PREVIEW_CLASS);
    return;
  }

  elements.appRoot.classList.remove(EDITOR_PREVIEW_CLASS);
}

function setTitlebarMenuOpen(isOpen: boolean): void {
  elements.titlebarMenu.hidden = !isOpen;
  elements.titlebarMenuButton.setAttribute("aria-expanded", String(isOpen));
  if (!isOpen) {
    setRecentSubmenuOpen(false);
  }
}

function closeTitlebarMenu(): void {
  setTitlebarMenuOpen(false);
}

function toggleTitlebarMenu(): void {
  setTitlebarMenuOpen(elements.titlebarMenu.hidden);
}

function isInteractiveTitlebarTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }

  return target.closest(
    "button, .document-tab, .document-tab-item, .titlebar-menu, .titlebar-menu-wrap, .titlebar-controls"
  ) !== null;
}

function setRecentSubmenuOpen(isOpen: boolean): void {
  elements.titlebarMenuRecent.hidden = !isOpen;
  elements.titlebarMenuOpenRecent.setAttribute("aria-expanded", String(isOpen));
}

function toggleRecentSubmenu(): void {
  setRecentSubmenuOpen(elements.titlebarMenuRecent.hidden);
}

function closePreviewContextMenu(): void {
  previewContextMenuTarget = undefined;
  elements.previewContextMenu.hidden = true;
}

function openPreviewContextMenu(left: number, top: number, target: SVGSVGElement): void {
  previewContextMenuTarget = target;
  elements.previewContextMenu.hidden = false;
  elements.previewContextMenu.style.left = "0px";
  elements.previewContextMenu.style.top = "0px";
  const position = clampContextMenuPosition({
    left,
    top,
    menuWidth: elements.previewContextMenu.offsetWidth,
    menuHeight: elements.previewContextMenu.offsetHeight,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight
  });
  elements.previewContextMenu.style.left = `${position.left}px`;
  elements.previewContextMenu.style.top = `${position.top}px`;
}

function getMermaidSvgTarget(target: EventTarget | null): SVGSVGElement | null {
  if (!(target instanceof Element)) {
    return null;
  }

  const svg = target.closest("svg");
  if (!(svg instanceof SVGSVGElement)) {
    return null;
  }

  return svg.closest(".mermaid") ? svg : null;
}

function getSvgRenderSize(svg: SVGSVGElement): { width: number; height: number } {
  const rect = svg.getBoundingClientRect();
  const viewBox = svg.viewBox.baseVal;
  const width = rect.width || svg.width.baseVal.value || viewBox.width;
  const height = rect.height || svg.height.baseVal.value || viewBox.height;

  if (!width || !height) {
    throw new Error("Mermaid diagram does not have a measurable size.");
  }

  return {
    width,
    height
  };
}

function serializeSvg(svg: SVGSVGElement, width: number, height: number): string {
  const clone = svg.cloneNode(true);
  if (!(clone instanceof SVGSVGElement)) {
    throw new Error("Could not clone Mermaid diagram SVG.");
  }

  clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
  clone.setAttribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
  clone.setAttribute("width", String(width));
  clone.setAttribute("height", String(height));
  if (!clone.getAttribute("viewBox")) {
    clone.setAttribute("viewBox", `0 0 ${width} ${height}`);
  }

  return new XMLSerializer().serializeToString(clone);
}

function getSvgViewBox(svg: SVGSVGElement): DOMRectReadOnly {
  const viewBox = svg.viewBox.baseVal;
  if (viewBox && viewBox.width && viewBox.height) {
    return viewBox;
  }

  const { width, height } = getSvgRenderSize(svg);
  return new DOMRectReadOnly(0, 0, width, height);
}

function getProjectedSvgRect(
  svg: SVGSVGElement,
  element: Element
): { x: number; y: number; width: number; height: number } {
  const svgRect = svg.getBoundingClientRect();
  const elementRect = element.getBoundingClientRect();
  const viewBox = getSvgViewBox(svg);
  const scaleX = svgRect.width ? viewBox.width / svgRect.width : 1;
  const scaleY = svgRect.height ? viewBox.height / svgRect.height : 1;

  return {
    x: viewBox.x + (elementRect.left - svgRect.left) * scaleX,
    y: viewBox.y + (elementRect.top - svgRect.top) * scaleY,
    width: elementRect.width * scaleX,
    height: elementRect.height * scaleY
  };
}

function getLabelTextElement(foreignObject: Element): HTMLElement | null {
  const label = foreignObject.querySelector<HTMLElement>(".nodeLabel, .edgeLabel");
  if (label) {
    return label;
  }

  return foreignObject.querySelector<HTMLElement>("span, div");
}

function getLabelLines(foreignObject: Element): string[] {
  const label = getLabelTextElement(foreignObject);
  if (!label) {
    return [];
  }

  const text = (label.innerText || label.textContent || "").replace(/\r\n?/gu, "\n").trim();

  if (!text) {
    return [];
  }

  return text
    .split("\n")
    .map((line: string) => line.trim())
    .filter(Boolean);
}

function setSvgTextStyles(
  text: SVGTextElement,
  computedStyle: CSSStyleDeclaration,
  scaleX: number
): void {
  const fontSize = Number.parseFloat(computedStyle.fontSize);
  if (Number.isFinite(fontSize) && fontSize > 0) {
    text.setAttribute("font-size", `${fontSize * scaleX}`);
  }

  if (computedStyle.fontFamily) {
    text.setAttribute("font-family", computedStyle.fontFamily);
  }
  if (computedStyle.fontWeight) {
    text.setAttribute("font-weight", computedStyle.fontWeight);
  }
  if (computedStyle.fontStyle) {
    text.setAttribute("font-style", computedStyle.fontStyle);
  }
  if (computedStyle.color) {
    text.setAttribute("fill", computedStyle.color);
  }

  const letterSpacing = Number.parseFloat(computedStyle.letterSpacing);
  if (Number.isFinite(letterSpacing)) {
    text.setAttribute("letter-spacing", `${letterSpacing * scaleX}`);
  }
}

function createSvgLabelReplacement(
  liveSvg: SVGSVGElement,
  liveForeignObject: SVGForeignObjectElement,
  exportDocument: Document
): SVGGElement | null {
  const lines = getLabelLines(liveForeignObject);
  if (!lines.length) {
    return null;
  }

  const svgRect = liveSvg.getBoundingClientRect();
  const viewBox = getSvgViewBox(liveSvg);
  const scaleX = svgRect.width ? viewBox.width / svgRect.width : 1;
  const scaleY = svgRect.height ? viewBox.height / svgRect.height : 1;
  const labelRect = getProjectedSvgRect(liveSvg, liveForeignObject);
  const labelElement = getLabelTextElement(liveForeignObject);
  const computedStyle = labelElement ? getComputedStyle(labelElement) : null;

  const group = exportDocument.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("class", "mdv-powerpoint-label");

  const backgroundColor = computedStyle?.backgroundColor ?? "rgba(0, 0, 0, 0)";
  if (backgroundColor !== "rgba(0, 0, 0, 0)" && backgroundColor !== "transparent") {
    const rect = exportDocument.createElementNS("http://www.w3.org/2000/svg", "rect");
    rect.setAttribute("x", `${labelRect.x}`);
    rect.setAttribute("y", `${labelRect.y}`);
    rect.setAttribute("width", `${labelRect.width}`);
    rect.setAttribute("height", `${labelRect.height}`);
    rect.setAttribute("rx", `${Math.min(labelRect.height / 6, 6 * scaleX)}`);
    rect.setAttribute("fill", backgroundColor);
    group.append(rect);
  }

  const text = exportDocument.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", `${labelRect.x + labelRect.width / 2}`);
  text.setAttribute("y", `${labelRect.y + labelRect.height / 2}`);
  text.setAttribute("text-anchor", "middle");
  text.setAttribute("dominant-baseline", "middle");

  if (computedStyle) {
    setSvgTextStyles(text, computedStyle, scaleX);
  }

  const fontSize = computedStyle ? Number.parseFloat(computedStyle.fontSize) || 16 : 16;
  const lineHeightCss = computedStyle ? Number.parseFloat(computedStyle.lineHeight) : Number.NaN;
  const lineHeight = (Number.isFinite(lineHeightCss) ? lineHeightCss : fontSize * 1.4) * scaleY;
  const startOffset = lines.length > 1 ? (-lineHeight * (lines.length - 1)) / 2 : 0;

  lines.forEach((line, index) => {
    const tspan = exportDocument.createElementNS("http://www.w3.org/2000/svg", "tspan");
    tspan.setAttribute("x", text.getAttribute("x") ?? "0");
    tspan.setAttribute("dy", `${index === 0 ? startOffset : lineHeight}`);
    tspan.textContent = line;
    text.append(tspan);
  });

  group.append(text);
  return group;
}

function replaceForeignObjectLabelsForPowerPoint(
  liveSvg: SVGSVGElement,
  exportSvg: SVGSVGElement,
  exportDocument: Document
): void {
  const liveForeignObjects = Array.from(
    liveSvg.querySelectorAll<SVGForeignObjectElement>("foreignObject")
  );
  const exportForeignObjects = Array.from(
    exportSvg.querySelectorAll<SVGForeignObjectElement>("foreignObject")
  );

  liveForeignObjects.forEach((liveForeignObject, index) => {
    const exportForeignObject = exportForeignObjects[index];
    if (!exportForeignObject) {
      return;
    }

    const replacement = createSvgLabelReplacement(liveSvg, liveForeignObject, exportDocument);
    if (replacement) {
      exportForeignObject.replaceWith(replacement);
      return;
    }

    exportForeignObject.remove();
  });
}

function getClipboardItemConstructor(): ClipboardItemLike {
  if (typeof ClipboardItem === "undefined") {
    throw new Error("Clipboard image export is not supported in this WebView.");
  }

  return ClipboardItem;
}

function getMermaidExportBackgroundColor(svg: SVGSVGElement): string {
  const candidates = [svg.closest(".mermaid"), elements.preview, document.body];

  for (const candidate of candidates) {
    if (!(candidate instanceof Element)) {
      continue;
    }

    const backgroundColor = getComputedStyle(candidate).backgroundColor;
    if (backgroundColor && backgroundColor !== "rgba(0, 0, 0, 0)" && backgroundColor !== "transparent") {
      return backgroundColor;
    }
  }

  return "#000000";
}

function loadRasterImage(source: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.addEventListener("load", () => {
      resolve(image);
    });
    image.addEventListener("error", () => {
      reject(new Error("Could not rasterize the Mermaid diagram."));
    });
    image.src = source;
  });
}

function canvasToPngBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (!blob) {
        reject(new Error("Could not encode the Mermaid diagram as PNG."));
        return;
      }

      resolve(blob);
    }, "image/png");
  });
}

async function createMermaidPngBlob(svg: SVGSVGElement): Promise<Blob> {
  const { width, height } = getSvgRenderSize(svg);
  const svgMarkup = serializeSvg(svg, width, height);
  const svgBlob = new Blob([svgMarkup], { type: "image/svg+xml;charset=utf-8" });
  const svgUrl = URL.createObjectURL(svgBlob);

  try {
    const image = await loadRasterImage(svgUrl);
    const scale = window.devicePixelRatio || 1;
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(width * scale));
    canvas.height = Math.max(1, Math.ceil(height * scale));
    const context = canvas.getContext("2d");
    if (!context) {
      throw new Error("Could not create a canvas for Mermaid export.");
    }

    context.scale(scale, scale);
    context.fillStyle = getMermaidExportBackgroundColor(svg);
    context.fillRect(0, 0, width, height);
    context.drawImage(image, 0, 0, width, height);
    return await canvasToPngBlob(canvas);
  } finally {
    URL.revokeObjectURL(svgUrl);
  }
}

async function copyMermaidDiagramToClipboard(svg: SVGSVGElement): Promise<void> {
  if (!navigator.clipboard?.write) {
    throw new Error("Clipboard image export is not available in this WebView.");
  }

  const pngBlob = await createMermaidPngBlob(svg);
  const ClipboardItemConstructor = getClipboardItemConstructor();
  await navigator.clipboard.write([new ClipboardItemConstructor({ "image/png": pngBlob })]);
}

function createMermaidPowerPointClipboardPayload(svg: SVGSVGElement): MermaidPowerPointClipboardPayload {
  const { width, height } = getSvgRenderSize(svg);
  const exportDocument = document.implementation.createDocument("http://www.w3.org/2000/svg", "svg");
  const clonedSvg = exportDocument.importNode(svg, true);
  if (!(clonedSvg instanceof SVGSVGElement)) {
    throw new Error("Could not clone Mermaid diagram SVG.");
  }
  replaceForeignObjectLabelsForPowerPoint(svg, clonedSvg, exportDocument);

  return {
    svg: serializeSvg(clonedSvg, width, height),
    width,
    height
  };
}

async function copyMermaidDiagramToPowerPointClipboard(svg: SVGSVGElement): Promise<void> {
  await invoke("copy_mermaid_diagram_as_powerpoint", {
    diagram: createMermaidPowerPointClipboardPayload(svg)
  });
}

function getActiveDocumentTab(workspace: WorkspacePayload): WorkspacePayload["documentTabs"][number] | null {
  if (workspace.activeDocumentIndex === null) {
    return null;
  }

  return workspace.documentTabs[workspace.activeDocumentIndex] ?? null;
}

function canSaveActiveUntitledDocument(workspace: WorkspacePayload): boolean {
  return getActiveDocumentTab(workspace)?.isUntitled ?? false;
}

function getDocumentTabAtIndex(
  workspace: WorkspacePayload | undefined,
  index: number
): WorkspacePayload["documentTabs"][number] | null {
  return workspace?.documentTabs[index] ?? null;
}

function syncMenuShortcutLabels(): void {
  document.querySelectorAll<HTMLElement>("[data-shortcut-action]").forEach((element) => {
    const shortcutAction = element.dataset.shortcutAction as ShortcutAction | undefined;
    if (!shortcutAction) {
      return;
    }

    element.textContent = getShortcutLabel(shortcutAction, isMacLike);
  });

  const quitShortcut = elements.titlebarMenuQuit.querySelector<HTMLElement>(".titlebar-menu-item-shortcut");
  if (quitShortcut) {
    quitShortcut.textContent = isMacLike ? "Cmd+Q" : "Alt+F4";
  }
}

function mergeWorkspacePayload(
  previousWorkspace: WorkspacePayload | undefined,
  workspace: WorkspacePayload
): WorkspacePayload {
  if (workspace.explorerUpdated || !previousWorkspace) {
    return workspace;
  }

  return {
    ...workspace,
    explorer: previousWorkspace.explorer
  };
}

function syncActiveExplorerFile(
  previousFilePath: string | null,
  currentFilePath: string | null
): void {
  if (previousFilePath === currentFilePath) {
    return;
  }

  let previousButton: HTMLButtonElement | undefined;
  let currentButton: HTMLButtonElement | undefined;
  const explorerButtons = elements.explorerTree.querySelectorAll<HTMLButtonElement>("[data-file-path]");

  for (const button of explorerButtons) {
    if (button.dataset.filePath === previousFilePath) {
      previousButton = button;
    } else if (button.dataset.filePath === currentFilePath) {
      currentButton = button;
    }

    if (previousButton && currentButton) {
      break;
    }
  }

  if (previousButton) {
    previousButton.classList.remove("tree-file-button-active");
    previousButton.setAttribute("aria-current", "false");
  }

  if (currentButton) {
    currentButton.classList.add("tree-file-button-active");
    currentButton.setAttribute("aria-current", "page");
  }
}

function renderRecentMenuItems(recentPaths: string[]): void {
  elements.titlebarMenuRecent.replaceChildren();

  if (!recentPaths.length) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "titlebar-menu-item";
    button.disabled = true;
    const label = document.createElement("span");
    label.className = "titlebar-menu-item-label";
    label.textContent = "No Recent Files";
    button.append(label);
    elements.titlebarMenuRecent.append(button);
    return;
  }

  recentPaths.forEach((path, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "titlebar-menu-item";
    button.dataset.menuAction = "open-recent";
    button.dataset.recentIndex = String(index);
    button.title = normalizeRecentPath(path);
    const label = document.createElement("span");
    label.className = "titlebar-menu-item-label";
    label.textContent = recentMenuLabel(path, index);
    button.append(label);
    elements.titlebarMenuRecent.append(button);
  });
}

function renderTitlebarMenu(workspace: WorkspacePayload): void {
  const canSaveUntitled = canSaveActiveUntitledDocument(workspace);
  elements.titlebarMenuSave.hidden = !canSaveUntitled;
  elements.titlebarMenuSaveAs.hidden = !canSaveUntitled;
  renderRecentMenuItems(workspace.recentPaths);
}

function shouldRefreshTitlebarMenu(
  previousWorkspace: WorkspacePayload | undefined,
  workspace: WorkspacePayload
): boolean {
  if (!previousWorkspace) {
    return true;
  }

  return (
    !sameRecentPaths(previousWorkspace.recentPaths, workspace.recentPaths) ||
    canSaveActiveUntitledDocument(previousWorkspace) !== canSaveActiveUntitledDocument(workspace)
  );
}

function shouldRefreshPreviewMarkup(
  previousWorkspace: WorkspacePayload | undefined,
  workspace: WorkspacePayload,
  wasPreviewHidden: boolean
): boolean {
  if (!previousWorkspace || wasPreviewHidden) {
    return true;
  }

  return (
    previousWorkspace.editorText !== workspace.editorText ||
    previousWorkspace.document.html !== workspace.document.html
  );
}

async function renderPreview(
  previousWorkspace: WorkspacePayload | undefined,
  workspace: WorkspacePayload
): Promise<void> {
  const wasPreviewHidden = elements.preview.hidden;
  const previewMarkupChanged = shouldRefreshPreviewMarkup(previousWorkspace, workspace, wasPreviewHidden);
  const previewViewState = previewMarkupChanged ? capturePreviewViewState() : undefined;

  if (workspace.editorText !== null) {
    const showEditorPreview = shouldShowEditorPreview(workspace);
    setEditorPreviewMode(showEditorPreview);

    if (!showEditorPreview) {
      elements.preview.hidden = true;
      clearPreview(elements.preview);
      return;
    }

    elements.preview.hidden = false;
    if (previewMarkupChanged) {
      setTrustedPreviewHtml(elements.preview, workspace.document);
      if (workspace.document.html.includes('class="mermaid"')) {
        await renderMermaid();
      }
      if (previewViewState) {
        restorePreviewViewState(previewViewState);
      }
    }

    return;
  }

  setEditorPreviewMode(false);
  elements.preview.hidden = false;

  if (!workspace.document.html) {
    clearPreview(elements.preview);
    return;
  }

  if (!previewMarkupChanged) {
    return;
  }

  setTrustedPreviewHtml(elements.preview, workspace.document);
  if (workspace.document.html.includes('class="mermaid"')) {
    await renderMermaid();
  }
  if (previewViewState) {
    restorePreviewViewState(previewViewState);
  }
}

async function syncWindowMaximizeState(): Promise<void> {
  const isMaximized = await currentWindow.isMaximized();
  document.documentElement.dataset.windowMaximized = String(isMaximized);
  elements.titlebarMaximizeIcon.classList.toggle("window-icon-maximized", isMaximized);
  elements.titlebarMaximize.setAttribute(
    "aria-label",
    isMaximized ? "Restore window" : "Maximize window"
  );
}

async function renderMermaid(): Promise<void> {
  const nodes = elements.preview.querySelectorAll<HTMLElement>(".mermaid");

  if (!nodes.length) {
    return;
  }

  const mermaid = await getMermaid();
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "antiscript",
    theme: getMermaidTheme(activeTheme),
    themeVariables: {
      background: getEditorBackgroundColor()
    }
  });
  await mermaid.run({ nodes });
}

async function renderWorkspace(workspace: WorkspacePayload): Promise<void> {
  const previousWorkspace = currentWorkspace;
  const mergedWorkspace = mergeWorkspacePayload(previousWorkspace, workspace);
  currentWorkspace = mergedWorkspace;
  closePreviewContextMenu();

  if (!previousWorkspace || !sameExplorer(previousWorkspace.explorer, mergedWorkspace.explorer)) {
    renderExplorer(elements, mergedWorkspace.explorer, mergedWorkspace.currentFilePath);
  } else if (previousWorkspace.currentFilePath !== mergedWorkspace.currentFilePath) {
    syncActiveExplorerFile(previousWorkspace.currentFilePath, mergedWorkspace.currentFilePath);
  }

  if (!previousWorkspace || !sameDocumentTabs(previousWorkspace.documentTabs, mergedWorkspace.documentTabs)) {
    renderDocumentTabs(elements, mergedWorkspace.documentTabs);
  }

  if (!previousWorkspace || previousWorkspace.editorText !== mergedWorkspace.editorText) {
    renderEditor(elements, mergedWorkspace.editorText);
  }

  if (shouldRefreshTitlebarMenu(previousWorkspace, mergedWorkspace)) {
    renderTitlebarMenu(mergedWorkspace);
  }

  elements.editor.dataset.documentIndex =
    mergedWorkspace.editorText !== null && mergedWorkspace.activeDocumentIndex !== null
      ? String(mergedWorkspace.activeDocumentIndex)
      : "";

  await renderPreview(previousWorkspace, mergedWorkspace);
}

async function awaitEditorFlush(): Promise<void> {
  await flushPendingEditorUpdate();
}

function clearPendingEditorUpdateTimeout(): void {
  if (pendingEditorUpdateTimeoutId === undefined) {
    return;
  }

  window.clearTimeout(pendingEditorUpdateTimeoutId);
  pendingEditorUpdateTimeoutId = undefined;
}

async function pushEditorUpdate(
  documentIndex: number,
  markdown: string,
  updateId: number
): Promise<void> {
  try {
    const workspace = await invoke<WorkspacePayload>("update_document_content", {
      index: documentIndex,
      markdown
    });
    if (
      updateId !== latestEditorUpdateId ||
      workspace.activeDocumentIndex !== documentIndex ||
      workspace.editorText !== markdown
    ) {
      return;
    }

    await renderWorkspace(workspace);
  } catch (error) {
    console.error(error);
  }
}

function queueEditorUpdate(documentIndex: number, markdown: string): void {
  pendingEditorUpdate = {
    documentIndex,
    markdown,
    updateId: ++latestEditorUpdateId
  };
  clearPendingEditorUpdateTimeout();
  pendingEditorUpdateTimeoutId = window.setTimeout(() => {
    void flushPendingEditorUpdate();
  }, EDITOR_UPDATE_DEBOUNCE_DELAY_MS);
}

async function flushPendingEditorUpdate(): Promise<void> {
  const pendingUpdate = pendingEditorUpdate;
  clearPendingEditorUpdateTimeout();

  if (!pendingUpdate) {
    await editorUpdateChain.catch(() => undefined);
    return;
  }

  pendingEditorUpdate = undefined;
  editorUpdateChain = editorUpdateChain
    .catch(() => undefined)
    .then(() =>
      pushEditorUpdate(
        pendingUpdate.documentIndex,
        pendingUpdate.markdown,
        pendingUpdate.updateId
      )
    );

  await editorUpdateChain.catch(() => undefined);
}

async function runWorkspaceCommand(
  command: WorkspaceCommand,
  args?: Record<string, unknown>,
  options?: { awaitEditorFlush?: boolean }
): Promise<WorkspacePayload | undefined> {
  closePreviewContextMenu();
  if (options?.awaitEditorFlush || pendingEditorUpdate) {
    await awaitEditorFlush();
  }

  setBusyState(true);

  try {
    const workspace = await invoke<WorkspacePayload>(command, args);
    await renderWorkspace(workspace);
    return workspace;
  } catch (error) {
    console.error(error);
    return undefined;
  } finally {
    setBusyState(false);
  }
}

async function openFolderFromDialog(): Promise<void> {
  closePreviewContextMenu();
  if (pendingEditorUpdate) {
    await awaitEditorFlush();
  }

  let defaultPath: string | undefined;
  try {
    defaultPath = await documentDir();
  } catch (error) {
    console.error(error);
  }

  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath
    });
    if (!selected || Array.isArray(selected)) {
      return;
    }

    await runWorkspaceCommand("open_folder", { path: selected });
  } catch (error) {
    console.error(error);
  }
}

async function selectDocumentTab(index: number): Promise<WorkspacePayload | undefined> {
  if (currentWorkspace?.activeDocumentIndex === index) {
    return currentWorkspace;
  }

  return runWorkspaceCommand("select_document", { index }, { awaitEditorFlush: true });
}

function getSuggestedUntitledSaveName(label: string): string {
  const sanitizedLabel = label.trim().replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-");
  const baseName = sanitizedLabel.length > 0 ? sanitizedLabel : "Untitled";
  return baseName.toLowerCase().endsWith(".md") ? baseName : `${baseName}.md`;
}

async function promptToSaveUntitledDocument(
  label: string,
  actionDescription: string
): Promise<"save" | "discard" | "cancel"> {
  const decision = await message(`What would you like to do with "${label}" before ${actionDescription}?`, {
    title: "Unsaved changes",
    kind: "warning",
    buttons: {
      yes: "Save",
      no: "Discard",
      cancel: "Cancel"
    }
  });

  if (decision === "Save" || decision === "Yes") {
    return "save";
  }

  if (decision === "Discard" || decision === "No") {
    return "discard";
  }

  return "cancel";
}

async function saveUntitledDocumentAtIndex(index: number): Promise<boolean> {
  const selectedWorkspace = await selectDocumentTab(index);
  const workspaceAfterSelection = selectedWorkspace ?? currentWorkspace;
  if (workspaceAfterSelection?.activeDocumentIndex !== index) {
    return false;
  }

  const selectedTab = getDocumentTabAtIndex(workspaceAfterSelection, index);
  if (!selectedTab?.isUntitled) {
    return true;
  }

  let defaultDirectory: string | undefined;
  try {
    defaultDirectory = await documentDir();
  } catch (error) {
    console.error(error);
  }

  let selectedPath: string | null;
  try {
    selectedPath = await saveDialog({
      title: `Save ${selectedTab.label}`,
      defaultPath: buildDefaultSavePath(
        defaultDirectory,
        getSuggestedUntitledSaveName(selectedTab.label)
      ),
      filters: [
        {
          name: "Markdown",
          extensions: ["md"]
        }
      ]
    });
  } catch (error) {
    console.error(error);
    return false;
  }

  if (!selectedPath) {
    return false;
  }

  const savedWorkspace = await runWorkspaceCommand("save_active_document_to_path", { path: selectedPath }, {
    awaitEditorFlush: true
  });
  return Boolean(savedWorkspace) && !getDocumentTabAtIndex(savedWorkspace, index)?.isUntitled;
}

async function resolveUntitledDocumentBeforeAction(
  index: number,
  actionDescription: string
): Promise<boolean> {
  await awaitEditorFlush();

  const documentTab = getDocumentTabAtIndex(currentWorkspace, index);
  if (!documentTab || !hasUnsavedUntitledContent(documentTab)) {
    return true;
  }

  const decision = await promptToSaveUntitledDocument(documentTab.label, actionDescription);
  if (decision === "cancel") {
    return false;
  }

  if (decision === "discard") {
    return true;
  }

  return saveUntitledDocumentAtIndex(index);
}

async function resolveUntitledDocumentsBeforeWindowClose(): Promise<boolean> {
  await awaitEditorFlush();

  const unsavedUntitledIndexes = getUnsavedUntitledDocumentIndexes(currentWorkspace?.documentTabs);
  for (const index of unsavedUntitledIndexes) {
    const shouldContinue = await resolveUntitledDocumentBeforeAction(index, "closing the app");
    if (!shouldContinue) {
      return false;
    }
  }

  return true;
}

async function closeDocumentTab(index: number): Promise<void> {
  const shouldClose = await resolveUntitledDocumentBeforeAction(index, "closing this tab");
  if (!shouldClose) {
    return;
  }

  await runWorkspaceCommand("close_document", { index });
}

async function closeWindowWithPrompt(): Promise<void> {
  if (pendingWindowCloseRequest) {
    await pendingWindowCloseRequest;
    return;
  }

  pendingWindowCloseRequest = (async () => {
    const shouldClose = await resolveUntitledDocumentsBeforeWindowClose();
    if (!shouldClose) {
      return;
    }

    try {
      appExitInProgress = true;
      await invoke("exit_app");
    } catch (error) {
      appExitInProgress = false;
      console.error(error);
    }
  })().finally(() => {
    pendingWindowCloseRequest = undefined;
  });

  await pendingWindowCloseRequest;
}

async function selectRelativeDocument(offset: number): Promise<void> {
  const workspace = currentWorkspace;
  if (!workspace?.documentTabs.length || workspace.activeDocumentIndex === null || workspace.documentTabs.length < 2) {
    return;
  }

  const nextIndex =
    (workspace.activeDocumentIndex + offset + workspace.documentTabs.length) %
    workspace.documentTabs.length;
  await runWorkspaceCommand("select_document", { index: nextIndex }, { awaitEditorFlush: true });
}

async function handleShortcutAction(action: ShortcutAction): Promise<void> {
  closeTitlebarMenu();

  switch (action) {
    case "new":
      await handleStaticMenuAction("new");
      break;
    case "open":
      await handleStaticMenuAction("open");
      break;
    case "open-folder":
      await handleStaticMenuAction("open-folder");
      break;
    case "save":
      if (currentWorkspace && canSaveActiveUntitledDocument(currentWorkspace)) {
        await handleStaticMenuAction("save");
      }
      break;
    case "save-as":
      if (currentWorkspace && canSaveActiveUntitledDocument(currentWorkspace)) {
        await handleStaticMenuAction("save-as");
      }
      break;
    case "close-tab":
      if (currentWorkspace?.activeDocumentIndex !== null && currentWorkspace?.activeDocumentIndex !== undefined) {
        await closeDocumentTab(currentWorkspace.activeDocumentIndex);
      }
      break;
    case "next-tab":
      await selectRelativeDocument(1);
      break;
    case "previous-tab":
      await selectRelativeDocument(-1);
      break;
    default:
      break;
  }
}

elements.preview.addEventListener(
  "wheel",
  (event: WheelEvent) => {
    if (!isPreviewZoomShortcut(event)) {
      return;
    }

    event.preventDefault();
    previewScale = applyPreviewScale(
      elements.preview,
      getNextPreviewScale(previewScale, event.deltaY)
    );
  },
  { passive: false }
);

elements.preview.addEventListener("contextmenu", (event: MouseEvent) => {
  const svg = getMermaidSvgTarget(event.target);
  if (!svg) {
    closePreviewContextMenu();
    return;
  }

  event.preventDefault();
  openPreviewContextMenu(event.clientX, event.clientY, svg);
});

elements.preview.addEventListener("scroll", () => {
  closePreviewContextMenu();
});

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

  await awaitEditorFlush();
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

elements.documentTabs.addEventListener("click", async (event: MouseEvent) => {
  const target = event.target;
  if (!(target instanceof Element)) {
    return;
  }

  const closeButton = target.closest<HTMLButtonElement>("[data-close-document-index]");
  if (closeButton) {
    const index = closeButton.dataset.closeDocumentIndex;
    if (!index) {
      return;
    }

    await closeDocumentTab(Number(index));
    return;
  }

  const button = target.closest<HTMLElement>("[data-document-index]");
  if (!button) {
    return;
  }

  const index = button.dataset.documentIndex;
  if (!index) {
    return;
  }

  await awaitEditorFlush();
  setBusyState(true);

  try {
    const workspace = await invoke<WorkspacePayload>("select_document", {
      index: Number(index)
    });
    await renderWorkspace(workspace);
  } catch (error) {
    console.error(error);
  } finally {
    setBusyState(false);
  }
});

elements.editor.addEventListener("input", () => {
  const index = elements.editor.dataset.documentIndex;
  if (!index) {
    return;
  }

  const documentIndex = Number(index);
  const markdown = elements.editor.value;
  queueEditorUpdate(documentIndex, markdown);
});

elements.titlebarMenuButton.addEventListener("click", (event) => {
  event.stopPropagation();
  toggleTitlebarMenu();
});

elements.titlebarTheme.addEventListener("click", async () => {
  await setAppTheme(getNextTheme(activeTheme));
});

async function handleStaticMenuAction(
  action: "new" | "save" | "save-as" | "open" | "open-folder" | "quit"
): Promise<void> {
  closeTitlebarMenu();

  switch (action) {
    case "new":
      await runWorkspaceCommand("new_document");
      break;
    case "save":
      if (currentWorkspace?.activeDocumentIndex !== null && currentWorkspace?.activeDocumentIndex !== undefined) {
        await saveUntitledDocumentAtIndex(currentWorkspace.activeDocumentIndex);
      }
      break;
    case "save-as":
      if (currentWorkspace?.activeDocumentIndex !== null && currentWorkspace?.activeDocumentIndex !== undefined) {
        await saveUntitledDocumentAtIndex(currentWorkspace.activeDocumentIndex);
      }
      break;
    case "open":
      await runWorkspaceCommand("open_markdown_dialog", undefined, { awaitEditorFlush: true });
      break;
    case "open-folder":
      await openFolderFromDialog();
      break;
    case "quit":
      await closeWindowWithPrompt();
      break;
    default:
      break;
  }
}

elements.titlebarMenuNew.addEventListener("click", async () => {
  await handleStaticMenuAction("new");
});

elements.titlebarMenuSave.addEventListener("click", async () => {
  await handleStaticMenuAction("save");
});

elements.titlebarMenuSaveAs.addEventListener("click", async () => {
  await handleStaticMenuAction("save-as");
});

elements.titlebarMenuOpen.addEventListener("click", async () => {
  await handleStaticMenuAction("open");
});

elements.titlebarMenuOpenFolder.addEventListener("click", async () => {
  await handleStaticMenuAction("open-folder");
});

elements.titlebarMenuOpenRecent.addEventListener("click", (event) => {
  event.stopPropagation();
  toggleRecentSubmenu();
});

elements.titlebarMenuQuit.addEventListener("click", async () => {
  await handleStaticMenuAction("quit");
});

elements.previewContextMenuCopyMermaid.addEventListener("click", async () => {
  const target = previewContextMenuTarget;
  closePreviewContextMenu();
  if (!target) {
    return;
  }

  elements.previewContextMenuCopyMermaid.disabled = true;

  try {
    await copyMermaidDiagramToClipboard(target);
  } catch (error) {
    console.error(error);
  } finally {
    elements.previewContextMenuCopyMermaid.disabled = false;
  }
});

elements.previewContextMenuCopyPowerPoint.addEventListener("click", async () => {
  const target = previewContextMenuTarget;
  closePreviewContextMenu();
  if (!target) {
    return;
  }

  elements.previewContextMenuCopyPowerPoint.disabled = true;

  try {
    await copyMermaidDiagramToPowerPointClipboard(target);
  } catch (error) {
    console.error(error);
  } finally {
    elements.previewContextMenuCopyPowerPoint.disabled = false;
  }
});

elements.titlebarMenuRecent.addEventListener("click", async (event: MouseEvent) => {
  const target = event.target;
  if (!(target instanceof Element)) {
    return;
  }

  const button = target.closest<HTMLButtonElement>("[data-recent-index]");
  if (!button) {
    return;
  }

  const index = button.dataset.recentIndex;
  if (!index) {
    return;
  }

  closeTitlebarMenu();
  await runWorkspaceCommand(
    "open_recent_index",
    { index: Number(index) },
    { awaitEditorFlush: true }
  );
});

document.addEventListener("click", (event: MouseEvent) => {
  const target = event.target;
  if (!(target instanceof Node)) {
    return;
  }

  if (!elements.previewContextMenu.hidden && !elements.previewContextMenu.contains(target)) {
    closePreviewContextMenu();
  }

  if (
    elements.titlebarMenu.hidden ||
    elements.titlebarMenu.contains(target) ||
    elements.titlebarMenuButton.contains(target)
  ) {
    return;
  }

  closeTitlebarMenu();
});

document.addEventListener("contextmenu", (event: MouseEvent) => {
  const target = event.target;
  if (!(target instanceof Node)) {
    closePreviewContextMenu();
    return;
  }

  if (elements.previewContextMenu.contains(target)) {
    event.preventDefault();
    return;
  }

  const svg = getMermaidSvgTarget(target);
  if (!svg) {
    closePreviewContextMenu();
  }
});

document.addEventListener("keydown", (event: KeyboardEvent) => {
  if (event.key === "Escape") {
    closePreviewContextMenu();
    closeTitlebarMenu();
    return;
  }

  if (event.defaultPrevented) {
    return;
  }

  const shortcutAction = getShortcutAction(event);
  if (!shortcutAction) {
    return;
  }

  event.preventDefault();
  void handleShortcutAction(shortcutAction);
});

elements.titlebar.addEventListener("mousedown", (event: MouseEvent) => {
  if (event.button !== 0 || isInteractiveTitlebarTarget(event.target)) {
    return;
  }

  pendingTitlebarDrag = {
    screenX: event.screenX,
    screenY: event.screenY
  };
});

document.addEventListener("mousemove", (event: MouseEvent) => {
  if (!pendingTitlebarDrag) {
    return;
  }

  if ((event.buttons & 1) === 0) {
    pendingTitlebarDrag = undefined;
    return;
  }

  const distance = Math.max(
    Math.abs(event.screenX - pendingTitlebarDrag.screenX),
    Math.abs(event.screenY - pendingTitlebarDrag.screenY)
  );
  if (distance < 4) {
    return;
  }

  pendingTitlebarDrag = undefined;
  void currentWindow.startDragging().catch((error) => {
    console.error(error);
  });
});

document.addEventListener("mouseup", () => {
  pendingTitlebarDrag = undefined;
});

elements.titlebar.addEventListener("dblclick", async (event: MouseEvent) => {
  if (isInteractiveTitlebarTarget(event.target)) {
    return;
  }

  await currentWindow.toggleMaximize();
  await syncWindowMaximizeState();
});

elements.titlebarMinimize.addEventListener("click", async () => {
  await currentWindow.minimize();
});

elements.titlebarMaximize.addEventListener("click", async () => {
  await currentWindow.toggleMaximize();
  await syncWindowMaximizeState();
});

elements.titlebarClose.addEventListener("click", async () => {
  await closeWindowWithPrompt();
});

await currentWindow.onResized(async () => {
  await syncWindowMaximizeState();
});

await currentWindow.onCloseRequested(async (event) => {
  if (appExitInProgress) {
    return;
  }
  event.preventDefault();
  await closeWindowWithPrompt();
});

await listen<WorkspacePayload>(WORKSPACE_UPDATED_EVENT, async (event) => {
  try {
    await renderWorkspace(event.payload);
  } catch (error) {
    console.error(error);
  }
});

clearPreview(elements.preview);
renderRecentMenuItems([]);
syncMenuShortcutLabels();
syncThemeToggleButton();
await syncWindowMaximizeState();

try {
  const workspace = await invoke<WorkspacePayload>("current_workspace");
  await renderWorkspace(workspace);
} catch (error) {
  console.error(error);
}
