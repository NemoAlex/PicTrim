import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { BatchProgress, BatchSettings, FailureEntry } from "./types";

export function startBatch(settings: BatchSettings): Promise<void> {
  return invoke("start_batch", { settings });
}

export function cancelBatch(): Promise<void> {
  return invoke("cancel_batch");
}

export async function pickDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择目录",
  });
  return typeof selected === "string" ? selected : null;
}

export function onProgress(callback: (progress: BatchProgress) => void): Promise<UnlistenFn> {
  return listen<BatchProgress>("batch-progress", (event) => callback(event.payload));
}

export function onFailures(callback: (failures: FailureEntry[]) => void): Promise<UnlistenFn> {
  return listen<FailureEntry[]>("batch-failures", (event) => callback(event.payload));
}
