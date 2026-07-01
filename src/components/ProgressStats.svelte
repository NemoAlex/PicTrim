<script lang="ts">
  import { formatBytes } from "../lib/format";
  import type { Copy } from "../lib/i18n.svelte";
  import type { BatchProgress } from "../lib/types";

  let {
    statusTitle,
    statusMessage,
    currentFile,
    progress,
    statusKind,
    copy,
  }: {
    statusTitle: string;
    statusMessage: string;
    currentFile: string;
    progress: BatchProgress | null;
    statusKind: string;
    copy: Copy;
  } = $props();

  const percent = $derived.by(() => {
    if (!progress || progress.discovered <= 0) return 0;
    return Math.min(100, Math.round((progress.processed / progress.discovered) * 100));
  });

  const fillClass = $derived(
    statusKind === "done" || statusKind === "error" || statusKind === "cancelled"
      ? statusKind
      : statusKind === "running" && (!progress || progress.discovered <= 0)
        ? "indeterminate"
        : "",
  );

  const fillWidth = $derived(fillClass === "indeterminate" ? "" : `${percent}%`);

  const hint = $derived(currentFile || statusMessage);

  const src = $derived(progress?.totalSrcBytes ?? 0);
  const dst = $derived(progress?.totalDstBytes ?? 0);
  const hasSize = $derived(src > 0);
  const savedPct = $derived(src > 0 ? Math.round(((src - dst) / src) * 100) : 0);

  const images = $derived(progress?.images ?? 0);
  const copied = $derived(progress?.copied ?? 0);
  const skipped = $derived(progress?.skipped ?? 0);
  const failed = $derived(progress?.failed ?? 0);
</script>

<section class="panel panel-card progress-panel">
  <div class="progress-head">
    <div class="progress-status">
      <span class="status-dot {statusKind}"></span>
      <h2 class="progress-title">{statusTitle}</h2>
    </div>
  </div>

  <div class="progress-bar">
    <div class="progress-fill {fillClass}" style:width={fillWidth}></div>
  </div>

  <div class="current-file">
    <span class="file" title={hint}>{hint}</span>
    {#if progress && progress.discovered > 0}
      <span class="count">{progress.processed} / {progress.discovered}</span>
    {/if}
  </div>

  {#if hasSize}
    <div class="size-card">
      <div class="size-flow">
        <span class="size-from">{formatBytes(src)}</span>
        <svg class="size-arrow" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M5 12h14M13 6l6 6-6 6" />
        </svg>
        <span class="size-to">{formatBytes(dst)}</span>
      </div>
      <div class="size-saved {savedPct >= 0 ? 'good' : 'bad'}">
        <strong>{Math.abs(savedPct)}%</strong>
        <span>{savedPct >= 0 ? copy.saved : copy.increased}</span>
      </div>
    </div>
  {/if}

  <div class="metrics">
    <span class="metric">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="3" y="3" width="18" height="18" rx="3" />
        <circle cx="8.5" cy="8.5" r="1.5" />
        <path d="M21 15l-5-5L5 21" />
      </svg>
      <span class="m-label">{copy.generatedImages}</span>
      <b>{images}</b>
    </span>
    {#if copied > 0}
      <span class="metric">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="9" y="9" width="11" height="11" rx="2" />
          <path d="M5 15V5a2 2 0 0 1 2-2h10" />
        </svg>
        <span class="m-label">{copy.copied}</span>
        <b>{copied}</b>
      </span>
    {/if}
    <span class="metric">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M5 4l8 8-8 8M15 4l4 8-4 8" />
      </svg>
      <span class="m-label">{copy.skipped}</span>
      <b>{skipped}</b>
    </span>
    <span class="metric {failed > 0 ? 'bad' : ''}">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
        <path d="M12 9v4M12 17h.01" />
      </svg>
      <span class="m-label">{copy.failed}</span>
      <b>{failed}</b>
    </span>
  </div>
</section>
