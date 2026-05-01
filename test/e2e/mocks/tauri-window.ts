import {
  isWindowMaximized,
  listenToDragDrop,
  recordWindowDrag,
  recordWindowMinimize,
  setWindowMaximized
} from "./tauri-fixture";

interface CloseRequestedEvent {
  preventDefault(): void;
}

type DragDropEventPayload =
  | { type: "enter"; paths: string[]; position: { x: number; y: number } }
  | { type: "over"; position: { x: number; y: number } }
  | { type: "drop"; paths: string[]; position: { x: number; y: number } }
  | { type: "leave" };

const currentWindow = {
  async isMaximized(): Promise<boolean> {
    return isWindowMaximized();
  },
  async toggleMaximize(): Promise<void> {
    setWindowMaximized(!isWindowMaximized());
  },
  async minimize(): Promise<void> {
    recordWindowMinimize();
  },
  async startDragging(): Promise<void> {
    recordWindowDrag();
  },
  async onResized(): Promise<() => void> {
    return () => {};
  },
  async onCloseRequested(_handler: (event: CloseRequestedEvent) => void | Promise<void>): Promise<() => void> {
    return () => {};
  },
  async onDragDropEvent(
    handler: (event: { payload: DragDropEventPayload }) => void | Promise<void>
  ): Promise<() => void> {
    return listenToDragDrop(handler);
  }
};

export function getCurrentWindow(): typeof currentWindow {
  return currentWindow;
}
