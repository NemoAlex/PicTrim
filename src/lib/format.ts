import type { BatchProgress, OutputFormat } from "./types";

export function toNumber(value: string | number, fallback: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 2)} ${units[unit]}`;
}

export function formatLabel(value: OutputFormat): string {
  if (value === "jpg") return "JPG";
  if (value === "png") return "PNG";
  if (value === "webp") return "WebP";
  return "保持原格式";
}

export function phaseTitle(progress: BatchProgress): string {
  if (progress.phase === "scanning") return "正在扫描";
  if (progress.phase === "processing") return "正在处理";
  if (progress.phase === "error") return "处理出错";
  if (progress.cancelled) return "已停止";
  if (progress.done) return "处理完成";
  return "等待开始";
}
