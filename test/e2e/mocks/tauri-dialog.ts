import { getNextMessageResult, getNextOpenPath, getNextSavePath } from "./tauri-fixture";

export async function open(): Promise<string | null> {
  return getNextOpenPath();
}

export async function save(): Promise<string | null> {
  return getNextSavePath();
}

export async function message(): Promise<string> {
  return getNextMessageResult();
}
