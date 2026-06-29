<script lang="ts">
  import { formatLabel } from "../lib/format";
  import type { BatchSettings } from "../lib/types";

  let {
    settings,
  }: {
    settings: BatchSettings;
  } = $props();

  function basename(path: string): string {
    if (!path) return "—";
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }

  const sourceTitle = $derived(settings.inputSources.join("\n"));
  const sourceLabel = $derived(
    settings.inputSources.length === 1
      ? basename(settings.inputSources[0])
      : `${settings.inputSources.length} 个来源`,
  );
  const sizeLabel = $derived(
    settings.resizeMode === "fitLongestSide"
      ? `最长边 ${settings.maxSide}px`
      : settings.resizeMode === "fitWidth"
        ? `宽 ${settings.width}px`
        : settings.resizeMode === "fitHeight"
          ? `高 ${settings.height}px`
          : `${settings.width}x${settings.height}px`,
  );
</script>

<section class="summary-bar">
  <div class="summary-paths">
    <span class="path-chip" title={sourceTitle}>
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      </svg>
      {sourceLabel}
    </span>
    <svg class="arrow" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M5 12h14M13 6l6 6-6 6" />
    </svg>
    <span class="path-chip" title={settings.outputDir}>
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      </svg>
      {basename(settings.outputDir)}
    </span>
  </div>

  <div class="summary-params">
    <span class="param-chip">{sizeLabel}</span>
    <span class="param-chip">{settings.rotation === "auto" ? "EXIF自动校正" : "手动旋转"}</span>
    <span class="param-chip">{settings.allowUpscale ? "允许放大" : "不放大"}</span>
    <span class="param-chip">质量 {settings.quality}</span>
    <span class="param-chip">{formatLabel(settings.outputFormat)}</span>
    <span class="param-chip">并发 {settings.concurrency}</span>
    <span class="param-chip">{settings.copyNonImages ? "复制非图片" : "忽略非图片"}</span>
    <span class="param-chip">{settings.skipExisting ? "跳过已存在" : "覆盖已存在"}</span>
  </div>
</section>
