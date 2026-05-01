export { invoke } from "./tauri-fixture";

export function convertFileSrc(path: string): string {
  return `asset://${encodeURIComponent(path)}`;
}
