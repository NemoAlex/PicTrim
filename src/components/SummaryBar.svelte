<script lang="ts">
  import { formatLabel } from "../lib/format";
  import type { BatchSettings } from "../lib/types";

  let {
    settings,
    running,
    onstop,
    onreset,
  }: {
    settings: BatchSettings;
    running: boolean;
    onstop: () => void;
    onreset: () => void;
  } = $props();

  function basename(path: string): string {
    if (!path) return "—";
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }
</script>

<section class="summary-bar">
  <div class="summary-paths">
    <span class="path-chip" title={settings.inputDir}>
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      </svg>
      {basename(settings.inputDir)}
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
    <span class="param-chip">{settings.maxSide}px</span>
    <span class="param-chip">质量 {settings.quality}</span>
    <span class="param-chip">{formatLabel(settings.outputFormat)}</span>
    <span class="param-chip">并发 {settings.concurrency}</span>
    <span class="param-chip">{settings.copyNonImages ? "复制非图片" : "忽略非图片"}</span>
    <span class="param-chip">{settings.skipExisting ? "跳过已存在" : "覆盖已存在"}</span>
  </div>

  <div class="summary-actions">
    {#if running}
      <button class="secondary danger" onclick={onstop}>停止</button>
    {:else}
      <button class="secondary" onclick={onreset}>返回</button>
    {/if}
  </div>
</section>
