import { getDocumentDirectory } from "./tauri-fixture";

export async function documentDir(): Promise<string> {
  return getDocumentDirectory();
}
