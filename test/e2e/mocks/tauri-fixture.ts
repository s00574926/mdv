const TRUST_MODEL = "trusted-local-markdown-preview";

interface RenderedDocument {
  title: string;
  html: string;
  sourceName: string;
  sourcePath: string;
  watching: boolean;
  trustModel: typeof TRUST_MODEL;
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

interface DocumentState {
  label: string;
  path: string | null;
  markdown: string;
  html: string;
  isUntitled: boolean;
  hasUnsavedContent: boolean;
  editorText: string | null;
}

interface WorkspacePayload {
  document: RenderedDocument;
  editorText: string | null;
  currentFilePath: string | null;
  explorer: ExplorerRoot | null;
  explorerUpdated: boolean;
  recentPaths: string[];
  documentTabs: {
    label: string;
    isUntitled: boolean;
    hasUnsavedContent: boolean;
    isActive: boolean;
  }[];
  activeDocumentIndex: number | null;
}

interface HarnessState {
  commands: { command: string; args?: unknown }[];
  documents: DocumentState[];
  activeDocumentIndex: number | null;
  explorer: ExplorerRoot | null;
  recentPaths: string[];
  nextOpenPath: string | null;
  nextSavePath: string | null;
  nextMessageResult: string;
  windowMaximized: boolean;
  windowMinimizeCount: number;
  windowDragCount: number;
  exitRequested: boolean;
}

interface HarnessApi {
  getState(): HarnessState;
  reset(): void;
  setNextOpenPath(path: string | null): void;
  setNextSavePath(path: string | null): void;
  setNextMessageResult(result: string): void;
  emitWorkspaceUpdated(workspace?: WorkspacePayload): Promise<void>;
  emitWindowDragDrop(paths: string[]): Promise<void>;
}

declare global {
  interface Window {
    __MDV_E2E__?: HarnessApi;
  }
}

type WorkspaceListener = (event: { payload: WorkspacePayload }) => void | Promise<void>;
type DragDropEventPayload =
  | { type: "enter"; paths: string[]; position: { x: number; y: number } }
  | { type: "over"; position: { x: number; y: number } }
  | { type: "drop"; paths: string[]; position: { x: number; y: number } }
  | { type: "leave" };
type DragDropListener = (event: { payload: DragDropEventPayload }) => void | Promise<void>;

const listeners = new Set<WorkspaceListener>();
const dragDropListeners = new Set<DragDropListener>();
const state = createInitialState();

function createInitialState(): HarnessState {
  return {
    commands: [],
    documents: [],
    activeDocumentIndex: null,
    explorer: null,
    recentPaths: [],
    nextOpenPath: "C:/Users/Test/Documents/opened.md",
    nextSavePath: "C:/Users/Test/Documents/draft.md",
    nextMessageResult: "Discard",
    windowMaximized: false,
    windowMinimizeCount: 0,
    windowDragCount: 0,
    exitRequested: false
  };
}

function cloneState(): HarnessState {
  return structuredClone(state);
}

function replaceState(nextState: HarnessState): void {
  state.commands = nextState.commands;
  state.documents = nextState.documents;
  state.activeDocumentIndex = nextState.activeDocumentIndex;
  state.explorer = nextState.explorer;
  state.recentPaths = nextState.recentPaths;
  state.nextOpenPath = nextState.nextOpenPath;
  state.nextSavePath = nextState.nextSavePath;
  state.nextMessageResult = nextState.nextMessageResult;
  state.windowMaximized = nextState.windowMaximized;
  state.windowMinimizeCount = nextState.windowMinimizeCount;
  state.windowDragCount = nextState.windowDragCount;
  state.exitRequested = nextState.exitRequested;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? "document.md";
}

function fileStem(path: string): string {
  return fileName(path).replace(/\.[^.]+$/, "");
}

function directoryName(path: string): string {
  return path.replace(/[\\/][^\\/]*$/, "") || path;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function renderMarkdown(markdown: string): string {
  const trimmed = markdown.trim();
  if (!trimmed) {
    return "";
  }

  if (/^(flowchart|graph)\s/m.test(trimmed)) {
    return `<pre class="mermaid">${escapeHtml(trimmed)}</pre>\n`;
  }

  return trimmed
    .split(/\n{2,}/)
    .map((block) => {
      const heading = block.match(/^#\s+(.+)$/);
      if (heading) {
        return `<h1>${escapeHtml(heading[1])}</h1>`;
      }
      return `<p>${escapeHtml(block).replaceAll("\n", "<br />")}</p>`;
    })
    .join("\n");
}

function documentPayload(document: DocumentState | undefined): RenderedDocument {
  if (!document) {
    return {
      title: "",
      html: "",
      sourceName: "",
      sourcePath: "",
      watching: false,
      trustModel: TRUST_MODEL
    };
  }

  return {
    title: document.path ? fileStem(document.path) : document.label,
    html: document.html,
    sourceName: document.path ? fileName(document.path) : "",
    sourcePath: document.path ?? "",
    watching: Boolean(document.path),
    trustModel: TRUST_MODEL
  };
}

function workspacePayload(): WorkspacePayload {
  const activeDocument =
    state.activeDocumentIndex === null
      ? undefined
      : state.documents[state.activeDocumentIndex];

  return {
    document: documentPayload(activeDocument),
    editorText: activeDocument?.editorText ?? null,
    currentFilePath: activeDocument?.path ?? null,
    explorer: state.explorer,
    explorerUpdated: true,
    recentPaths: [...state.recentPaths],
    documentTabs: state.documents.map((document, index) => ({
      label: document.label,
      isUntitled: document.isUntitled,
      hasUnsavedContent: document.hasUnsavedContent,
      isActive: index === state.activeDocumentIndex
    })),
    activeDocumentIndex: state.activeDocumentIndex
  };
}

function rememberRecentPath(path: string): void {
  state.recentPaths = [path, ...state.recentPaths.filter((candidate) => candidate !== path)].slice(
    0,
    10
  );
}

function createUntitledDocument(): DocumentState {
  const untitledCount = state.documents.filter((document) => document.isUntitled).length + 1;
  return {
    label: untitledCount === 1 ? "Untitled" : `Untitled ${untitledCount}`,
    path: null,
    markdown: "",
    html: "",
    isUntitled: true,
    hasUnsavedContent: false,
    editorText: ""
  };
}

function createOpenedDocument(path: string, markdown?: string): DocumentState {
  const contents = markdown ?? `# ${fileStem(path)}\n\nOpened from ${path}`;
  return {
    label: fileName(path),
    path,
    markdown: contents,
    html: renderMarkdown(contents),
    isUntitled: false,
    hasUnsavedContent: false,
    editorText: null
  };
}

function openDocument(document: DocumentState): WorkspacePayload {
  state.documents.push(document);
  state.activeDocumentIndex = state.documents.length - 1;
  if (document.path) {
    rememberRecentPath(document.path);
  }
  return workspacePayload();
}

function buildExplorerRoot(path: string): ExplorerRoot {
  const rootName = fileName(path);
  return {
    name: rootName,
    path,
    children: [
      {
        name: "guide.md",
        path: `${path}/guide.md`,
        kind: "file",
        children: []
      },
      {
        name: "notes.txt",
        path: `${path}/notes.txt`,
        kind: "file",
        children: []
      },
      {
        name: "nested",
        path: `${path}/nested`,
        kind: "directory",
        children: [
          {
            name: "nested.md",
            path: `${path}/nested/nested.md`,
            kind: "file",
            children: []
          }
        ]
      }
    ]
  };
}

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  state.commands.push({ command, args });

  switch (command) {
    case "current_workspace":
      return workspacePayload() as T;

    case "new_document":
      return openDocument(createUntitledDocument()) as T;

    case "update_document_content": {
      const index = Number(args?.index);
      const markdown = String(args?.markdown ?? "");
      const document = state.documents[index];
      if (!document) {
        throw new Error(`Missing document at index ${index}`);
      }
      document.markdown = markdown;
      document.html = renderMarkdown(markdown);
      document.hasUnsavedContent = true;
      document.editorText = markdown;
      return workspacePayload() as T;
    }

    case "save_active_document_to_path": {
      const path = String(args?.path ?? "");
      const document =
        state.activeDocumentIndex === null
          ? undefined
          : state.documents[state.activeDocumentIndex];
      if (!document || !path) {
        throw new Error("No active document to save");
      }
      document.path = path;
      document.label = fileName(path);
      document.isUntitled = false;
      document.hasUnsavedContent = false;
      rememberRecentPath(path);
      return workspacePayload() as T;
    }

    case "open_markdown_dialog": {
      const path = state.nextOpenPath;
      if (!path) {
        return workspacePayload() as T;
      }
      return openDocument(createOpenedDocument(path)) as T;
    }

    case "open_markdown": {
      const path = String(args?.path ?? "");
      if (!path) {
        return workspacePayload() as T;
      }
      return openDocument(createOpenedDocument(path)) as T;
    }

    case "open_dropped_path": {
      const path = String(args?.path ?? "");
      if (!path) {
        return workspacePayload() as T;
      }

      if (/\.md$/i.test(path)) {
        return openDocument(createOpenedDocument(path)) as T;
      }

      state.explorer = buildExplorerRoot(path);
      state.activeDocumentIndex = null;
      return workspacePayload() as T;
    }

    case "open_folder": {
      const path = String(args?.path ?? state.nextOpenPath ?? "C:/Users/Test/Documents");
      state.explorer = buildExplorerRoot(path);
      state.activeDocumentIndex = null;
      return workspacePayload() as T;
    }

    case "select_explorer_file": {
      const path = String(args?.path ?? "");
      const existingIndex = state.documents.findIndex((document) => document.path === path);
      if (existingIndex >= 0) {
        state.activeDocumentIndex = existingIndex;
        return workspacePayload() as T;
      }
      return openDocument(createOpenedDocument(path, `# ${fileStem(path)}\n\nSelected from explorer.`)) as T;
    }

    case "select_document": {
      const index = Number(args?.index);
      if (index < 0 || index >= state.documents.length) {
        throw new Error(`Missing document at index ${index}`);
      }
      state.activeDocumentIndex = index;
      return workspacePayload() as T;
    }

    case "close_document": {
      const index = Number(args?.index);
      state.documents.splice(index, 1);
      if (!state.documents.length) {
        state.activeDocumentIndex = null;
      } else if (state.activeDocumentIndex === null || state.activeDocumentIndex >= state.documents.length) {
        state.activeDocumentIndex = state.documents.length - 1;
      }
      return workspacePayload() as T;
    }

    case "open_recent_index": {
      const index = Number(args?.index);
      const path = state.recentPaths[index];
      if (!path) {
        throw new Error("That recent file entry no longer exists.");
      }
      return openDocument(createOpenedDocument(path)) as T;
    }

    case "save_active_document":
    case "save_active_document_as":
      return workspacePayload() as T;

    case "copy_mermaid_diagram_as_powerpoint":
      return undefined as T;

    case "exit_app":
      state.exitRequested = true;
      return undefined as T;

    default:
      throw new Error(`Unhandled Tauri command: ${command}`);
  }
}

export async function listenToWorkspace(listener: WorkspaceListener): Promise<() => void> {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export async function listenToDragDrop(listener: DragDropListener): Promise<() => void> {
  dragDropListeners.add(listener);
  return () => {
    dragDropListeners.delete(listener);
  };
}

export function getDocumentDirectory(): string {
  return "C:/Users/Test/Documents";
}

export function getNextOpenPath(): string | null {
  return state.nextOpenPath;
}

export function getNextSavePath(): string | null {
  return state.nextSavePath;
}

export function getNextMessageResult(): string {
  return state.nextMessageResult;
}

export function setWindowMaximized(maximized: boolean): void {
  state.windowMaximized = maximized;
}

export function isWindowMaximized(): boolean {
  return state.windowMaximized;
}

export function recordWindowMinimize(): void {
  state.windowMinimizeCount += 1;
}

export function recordWindowDrag(): void {
  state.windowDragCount += 1;
}

async function emitWorkspaceUpdated(workspace = workspacePayload()): Promise<void> {
  await Promise.all([...listeners].map((listener) => listener({ payload: workspace })));
}

async function emitWindowDragDrop(paths: string[]): Promise<void> {
  const position = { x: 200, y: 200 };
  await Promise.all([
    ...dragDropListeners
  ].map((listener) => listener({ payload: { type: "enter", paths, position } })));
  await Promise.all([
    ...dragDropListeners
  ].map((listener) => listener({ payload: { type: "drop", paths, position } })));
}

if (!window.__MDV_E2E__) {
  window.__MDV_E2E__ = {
    getState: cloneState,
    reset() {
      replaceState(createInitialState());
      listeners.clear();
      dragDropListeners.clear();
    },
    setNextOpenPath(path) {
      state.nextOpenPath = path;
    },
    setNextSavePath(path) {
      state.nextSavePath = path;
    },
    setNextMessageResult(result) {
      state.nextMessageResult = result;
    },
    emitWorkspaceUpdated,
    emitWindowDragDrop
  };
}
