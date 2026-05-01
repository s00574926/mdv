import { listenToWorkspace } from "./tauri-fixture";

export async function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void | Promise<void>
): Promise<() => void> {
  if (event !== "workspace://updated") {
    return () => {};
  }

  return listenToWorkspace(handler as (event: { payload: unknown }) => void | Promise<void>);
}
