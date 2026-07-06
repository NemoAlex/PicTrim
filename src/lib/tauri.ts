import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCopy } from "./i18n.svelte";
import type {
  BatchProgress,
  BatchSettings,
  FailureEntry,
  PreviewDirectoryPage,
  PreviewPair,
  PreviewTree,
  SourceEntry,
} from "./types";

export function startBatch(settings: BatchSettings): Promise<void> {
  return invoke("start_batch", { settings });
}

export function cancelBatch(): Promise<void> {
  return invoke("cancel_batch");
}

export function loadSettings(): Promise<BatchSettings | null> {
  return invoke("load_settings");
}

export function saveSettings(settings: BatchSettings): Promise<void> {
  return invoke("save_settings", { settings });
}

export function classifySources(paths: string[]): Promise<SourceEntry[]> {
  return invoke("classify_sources", { paths });
}

export function loadPreviewTree(settings: BatchSettings): Promise<PreviewTree> {
  return invoke("load_preview_tree", { settings });
}

export function loadPreviewDirectory(
  settings: BatchSettings,
  dirPath: string | null,
  offset: number,
  limit: number,
): Promise<PreviewDirectoryPage> {
  return invoke("load_preview_directory", { settings, dirPath, offset, limit });
}

export function renderPreview(settings: BatchSettings, srcPath: string): Promise<PreviewPair> {
  return invoke("render_preview", { settings, srcPath });
}

export async function pickDirectory(): Promise<string | null> {
  const copy = getCopy();
  const selected = await open({
    directory: true,
    multiple: false,
    title: copy.selectFolderTitle,
  });
  return typeof selected === "string" ? selected : null;
}

export async function pickDirectories(): Promise<string[]> {
  const copy = getCopy();
  const selected = await open({
    directory: true,
    multiple: true,
    title: copy.selectFolderTitle,
  });
  return Array.isArray(selected) ? selected.filter((path): path is string => typeof path === "string") : [];
}

export async function pickFiles(): Promise<string[]> {
  const copy = getCopy();
  const selected = await open({
    directory: false,
    multiple: true,
    title: copy.selectFilesTitle,
  });
  return Array.isArray(selected)
    ? selected.filter((path): path is string => typeof path === "string")
    : typeof selected === "string"
      ? [selected]
      : [];
}

export function onSourceDrop(callback: (paths: string[]) => void): Promise<UnlistenFn> {
  return getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "drop") {
      callback(event.payload.paths);
    }
  });
}

export function onProgress(callback: (progress: BatchProgress) => void): Promise<UnlistenFn> {
  return listen<BatchProgress>("batch-progress", (event) => callback(event.payload));
}

export function onFailures(callback: (failures: FailureEntry[]) => void): Promise<UnlistenFn> {
  return listen<FailureEntry[]>("batch-failures", (event) => callback(event.payload));
}
