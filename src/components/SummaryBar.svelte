<script lang="ts">
  import { outputFormatLabel, type Copy } from "../lib/i18n.svelte";
  import type { BatchSettings } from "../lib/types";

  let {
    settings,
    copy,
  }: {
    settings: BatchSettings;
    copy: Copy;
  } = $props();

  function basename(path: string): string {
    if (!path) return "-";
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }

  const sourceTitle = $derived(settings.inputSources.join("\n"));
  const sourceLabel = $derived(
    settings.inputSources.length === 1
      ? basename(settings.inputSources[0])
      : copy.summarySources(settings.inputSources.length),
  );
  const sizeLabel = $derived(
    settings.resizeMode === "fitLongestSide"
      ? copy.summaryLongestSide(settings.maxSide)
      : settings.resizeMode === "fitWidth"
        ? copy.summaryWidth(settings.width)
        : settings.resizeMode === "fitHeight"
          ? copy.summaryHeight(settings.height)
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
    <span class="param-chip">{settings.rotation === "auto" ? copy.autoRotation : copy.manualRotation}</span>
    <span class="param-chip">{settings.allowUpscale ? copy.allowUpscale : copy.disallowUpscale}</span>
    <span class="param-chip">{copy.qualitySummary(settings.quality)}</span>
    <span class="param-chip">{outputFormatLabel(settings.outputFormat, copy)}</span>
    <span class="param-chip">{copy.concurrencySummary(settings.concurrency)}</span>
    <span class="param-chip">{settings.copyNonImages ? copy.copyNonImagesSummary : copy.ignoreNonImagesSummary}</span>
    <span class="param-chip">{settings.skipExisting ? copy.skipExistingSummary : copy.overwriteExistingSummary}</span>
  </div>
</section>
