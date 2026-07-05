<script lang="ts">
  import { onMount } from "svelte";
  import {
    classifySources,
    onSourceDrop,
    pickDirectories,
    pickDirectory,
    pickFiles,
  } from "../lib/tauri";
  import { resizeModeCopy, sourceKindLabel, type Copy } from "../lib/i18n.svelte";
  import type { BatchSettings, SourceEntry } from "../lib/types";

  let {
    settings = $bindable(),
    copy,
  }: {
    settings: BatchSettings;
    copy: Copy;
  } = $props();

  let sources = $state<SourceEntry[]>([]);
  let dropActive = $state(false);
  let cropPreviewWide = $state(true);
  let longestSideWide = $state(true);
  let showPreviewBubble = $state(false);
  let showAdvanced = $state(false);
  let showAllSources = $state(false);
  let previewTimer: number | undefined;

  const processingMode = $derived(settings.resizeMode);
  const usesBoxSize = $derived(processingMode === "fitBox" || processingMode === "fixedCrop");
  const usesWidth = $derived(processingMode === "fitWidth");
  const usesHeight = $derived(processingMode === "fitHeight");
  const usesCrop = $derived(processingMode === "fixedCrop");
  const visibleSources = $derived(showAllSources ? sources : sources.slice(0, 5));
  const hiddenSourceCount = $derived(Math.max(0, sources.length - visibleSources.length));
  const previewTitle = $derived(previewCopy(processingMode).title);
  const previewDetail = $derived(previewCopy(processingMode).detail);
  const upscaleLabel = $derived(
    processingMode === "fitLongestSide"
      ? copy.longestSideShortfall
      : processingMode === "fitWidth"
        ? copy.widthShortfall
        : processingMode === "fitHeight"
          ? copy.heightShortfall
          : copy.sizeShortfall,
  );
  const upscaleOptionLabel = $derived(
    processingMode === "fitLongestSide"
      ? copy.upscaleToLongestSide
      : processingMode === "fitWidth"
        ? copy.upscaleToWidth
        : processingMode === "fitHeight"
          ? copy.upscaleToHeight
          : copy.upscaleToFit,
  );

  onMount(() => {
    refreshSources();
    const unlisteners: Array<() => void> = [];
    onSourceDrop(addSources).then((unlisten) => unlisteners.push(unlisten));
    return () => {
      for (const unlisten of unlisteners) unlisten();
      if (previewTimer) window.clearTimeout(previewTimer);
    };
  });

  $effect(() => {
    const snapshot = settings.inputSources.join("\n");
    refreshSources(snapshot);
  });

  $effect(() => {
    if (sources.length <= 5) showAllSources = false;
  });

  async function refreshSources(_snapshot = settings.inputSources.join("\n")) {
    sources = settings.inputSources.length > 0 ? await classifySources(settings.inputSources) : [];
  }

  function addSources(paths: string[]) {
    const seen = new Set(settings.inputSources);
    const next = [...settings.inputSources];
    for (const path of paths) {
      if (!seen.has(path)) {
        seen.add(path);
        next.push(path);
      }
    }
    settings.inputSources = next;
  }

  async function addFiles() {
    addSources(await pickFiles());
  }

  async function addDirectories() {
    addSources(await pickDirectories());
  }

  async function pickOutput() {
    const dir = await pickDirectory();
    if (dir) settings.outputDir = dir;
  }

  function removeSource(path: string) {
    settings.inputSources = settings.inputSources.filter((source) => source !== path);
  }

  function clearSources() {
    settings.inputSources = [];
    showAllSources = false;
  }

  function sourceName(path: string): string {
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }

  function previewCopy(mode: BatchSettings["resizeMode"]) {
    return resizeModeCopy(mode, copy);
  }

  function sizeLabel(value: number): string {
    const numberValue = Number(value);
    return Number.isFinite(numberValue) && numberValue > 0 ? `${numberValue}px` : "-";
  }

  function widthMeasure(mode: BatchSettings["resizeMode"]): string {
    if (mode === "fitWidth" || mode === "fixedCrop" || mode === "fitBox") return sizeLabel(settings.width);
    if (mode === "fitLongestSide" && longestSideWide) return sizeLabel(settings.maxSide);
    return copy.flexible;
  }

  function heightMeasure(mode: BatchSettings["resizeMode"]): string {
    if (mode === "fitHeight" || mode === "fixedCrop" || mode === "fitBox") return sizeLabel(settings.height);
    if (mode === "fitLongestSide" && !longestSideWide) return sizeLabel(settings.maxSide);
    return copy.flexible;
  }

  function openPreviewBubble(autoClose = true) {
    showPreviewBubble = true;
    if (previewTimer) window.clearTimeout(previewTimer);
    if (autoClose) {
      previewTimer = window.setTimeout(() => {
        showPreviewBubble = false;
        previewTimer = undefined;
      }, 4200);
    }
  }

  function togglePreviewBubble() {
    if (showPreviewBubble) {
      showPreviewBubble = false;
      if (previewTimer) window.clearTimeout(previewTimer);
      previewTimer = undefined;
      return;
    }
    openPreviewBubble(false);
  }

  function updateProcessingMode(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value as BatchSettings["resizeMode"];
    settings.thumbnail = false;
    settings.resizeMode = value;
    openPreviewBubble();
  }
</script>

<section class="panel setup-panel">
  <div class="task-flow">
    <section class="task-section source-task">
      <div class="task-head">
        <span class="task-step">1</span>
        <div>
          <h3>{copy.sourceTaskTitle}</h3>
          <p>{settings.inputSources.length > 0 ? copy.sourceTaskReady(settings.inputSources.length) : copy.sourceTaskHint}</p>
        </div>
      </div>

      <div class="source-section">
        <div
          class:active={dropActive}
          class:has-sources={sources.length > 0}
          class="drop-zone"
          role="button"
          tabindex="0"
          ondragenter={() => (dropActive = true)}
          ondragover={(event) => {
            event.preventDefault();
            dropActive = true;
          }}
          ondragleave={() => (dropActive = false)}
          ondrop={() => (dropActive = false)}
        >
          <div class="drop-head">
            <div>
              <strong>{settings.inputSources.length > 0 ? copy.inputSourceCount(settings.inputSources.length) : copy.dropFilesOrFolders}</strong>
            </div>
            {#if sources.length > 0}
              <button class="secondary clear-button" onclick={clearSources}>{copy.clear}</button>
            {/if}
          </div>

          <div class="source-list">
            {#if sources.length > 0}
              {#each visibleSources as source}
                <div class:missing={source.kind === "missing"} class="source-item">
                  <span class="source-kind">{sourceKindLabel(source.kind, copy)}</span>
                  <span class="source-name" title={source.path}>{sourceName(source.path)}</span>
                  <button class="icon-button" aria-label={copy.removeSource} title={copy.removeSource} onclick={() => removeSource(source.path)}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true">
                      <path d="M18 6 6 18M6 6l12 12" />
                    </svg>
                  </button>
                </div>
              {/each}
              {#if hiddenSourceCount > 0}
                <button class="source-more" onclick={() => (showAllSources = true)}>
                  {copy.moreSources(hiddenSourceCount)}
                </button>
              {:else if showAllSources && sources.length > 5}
                <button class="source-more" onclick={() => (showAllSources = false)}>
                  {copy.collapseSources}
                </button>
              {/if}
            {:else}
              <div class="source-empty">
                <span>{copy.dropHere}</span>
              </div>
            {/if}
          </div>

          <div class="source-actions">
            <button class="secondary" onclick={addFiles}>{copy.addFiles}</button>
            <button class="secondary" onclick={addDirectories}>{copy.addFolders}</button>
          </div>
        </div>
      </div>
    </section>

    <section class="task-section">
      <div class="task-head">
        <span class="task-step">2</span>
        <div>
          <h3>{copy.outputTaskTitle}</h3>
          <p>{copy.outputTaskHint}</p>
        </div>
      </div>

      <div class="paths">
        <label>
          <div class="path-row">
            <input readonly placeholder={copy.outputPlaceholder} value={settings.outputDir} />
            <button class="secondary" onclick={pickOutput}>{copy.choose}</button>
          </div>
        </label>
      </div>
    </section>

    <section class="task-section">
      <div class="task-head">
        <span class="task-step">3</span>
        <div>
          <h3>{copy.ruleTaskTitle}</h3>
          <p>{previewTitle}，{previewDetail}</p>
        </div>
      </div>

      <div class="rule-panel">
        <div class="settings core-rules">
          <div class="processing-field">
            <div class="field-label-row">
              <span>{copy.processingMode}</span>
              <button class="icon-button preview-button" aria-label={copy.previewProcessingMode} title={copy.previewProcessingMode} onclick={togglePreviewBubble}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <circle cx="12" cy="12" r="10" />
                  <path d="M12 16v-4" />
                  <path d="M12 8h.01" />
                </svg>
              </button>
            </div>
            <div class="processing-control">
              <select value={processingMode} onchange={updateProcessingMode}>
                <option value="fitLongestSide">{resizeModeCopy("fitLongestSide", copy).title}</option>
                <option value="fitBox">{resizeModeCopy("fitBox", copy).title}</option>
                <option value="fitWidth">{resizeModeCopy("fitWidth", copy).title}</option>
                <option value="fitHeight">{resizeModeCopy("fitHeight", copy).title}</option>
                <option value="fixedCrop">{resizeModeCopy("fixedCrop", copy).title}</option>
              </select>
            </div>

            {#if showPreviewBubble}
              <div class="preview-popover" data-mode={processingMode}>
                <div class="preview-popover-arrow"></div>
                <div class="resize-preview" data-mode={processingMode}>
                  <div class="diagram-art">
                    <div aria-hidden="true">
                      {#key processingMode + (processingMode === "fitLongestSide" ? String(longestSideWide) : "")}
                        <div
                          class="motion-canvas"
                          data-longest-preview={processingMode === "fitLongestSide" ? (longestSideWide ? "landscape" : "portrait") : undefined}
                          data-crop-preview={processingMode === "fixedCrop" ? (cropPreviewWide ? "wide" : "tall") : undefined}
                          data-crop-x={processingMode === "fixedCrop" ? settings.cropHorizontal : undefined}
                          data-crop-y={processingMode === "fixedCrop" ? settings.cropVertical : undefined}
                        >
                          <div class="motion-photo source-photo">
                            <span class="photo-sun"></span>
                            <span class="photo-ridge photo-ridge-back"></span>
                            <span class="photo-ridge photo-ridge-front"></span>
                            <span class="photo-shine"></span>
                          </div>
                          <span class="flow-arrow"></span>
                          <div class="result-frame">
                            <div class="motion-photo result-photo">
                              <span class="photo-sun"></span>
                              <span class="photo-ridge photo-ridge-back"></span>
                              <span class="photo-ridge photo-ridge-front"></span>
                              <span class="photo-shine"></span>
                            </div>
                          </div>
                          {#if processingMode === "fixedCrop"}
                            <div class="crop-window"></div>
                          {/if}
                          <span class="measure-line measure-width"></span>
                          <span class="measure-line measure-height"></span>
                          <span class="measure-label measure-label-width">{copy.widthMeasure}: {widthMeasure(processingMode)}</span>
                          <span class="measure-label measure-label-height">{copy.heightMeasure}: {heightMeasure(processingMode)}</span>
                        </div>
                      {/key}
                    </div>
                  </div>
                  <div class="preview-copy">
                    <strong>{previewTitle}</strong>
                    <span>{previewDetail}</span>
                    {#if processingMode === "fitLongestSide"}
                      <div class="crop-preview-toggle">
                        <strong class="crop-preview-label">{copy.exampleCase}</strong>
                        <div>
                          <button class:active={longestSideWide} onclick={() => (longestSideWide = true)}>{copy.landscapeImage}</button>
                          <button class:active={!longestSideWide} onclick={() => (longestSideWide = false)}>{copy.portraitImage}</button>
                        </div>
                      </div>
                    {:else if processingMode === "fixedCrop"}
                      <div class="crop-preview-toggle">
                        <strong class="crop-preview-label">{copy.exampleCase}</strong>
                        <div>
                          <button class:active={cropPreviewWide} onclick={() => (cropPreviewWide = true)}>{copy.sourceTooWide}</button>
                          <button class:active={!cropPreviewWide} onclick={() => (cropPreviewWide = false)}>{copy.sourceTooTall}</button>
                        </div>
                      </div>
                    {/if}
                  </div>
                </div>
              </div>
            {/if}
          </div>

          {#if usesBoxSize}
            <label>
              <span>{copy.maxWidth}</span>
              <input type="number" min="1" max="50000" bind:value={settings.width} />
            </label>
            <label>
              <span>{copy.maxHeight}</span>
              <input type="number" min="1" max="50000" bind:value={settings.height} />
            </label>
          {:else}
            {#if usesWidth}
              <label>
                <span>{copy.targetWidth}</span>
                <input type="number" min="1" max="50000" bind:value={settings.width} />
              </label>
            {:else if usesHeight}
              <label>
                <span>{copy.targetHeight}</span>
                <input type="number" min="1" max="50000" bind:value={settings.height} />
              </label>
            {:else}
              <label>
                <span>{copy.longestSide}</span>
                <input type="number" min="1" max="50000" bind:value={settings.maxSide} />
              </label>
            {/if}
          {/if}
          <label>
            <span>{copy.quality}</span>
            <input type="number" min="1" max="100" bind:value={settings.quality} />
          </label>
          <label>
            <span>{copy.outputFormat}</span>
            <select bind:value={settings.outputFormat}>
              <option value="jpg">JPG</option>
              <option value="png">PNG</option>
              <option value="webp">WebP</option>
              <option value="keep">{copy.outputKeep}</option>
            </select>
          </label>
        </div>

        <button class="advanced-toggle" class:open={showAdvanced} onclick={() => (showAdvanced = !showAdvanced)} aria-expanded={showAdvanced}>
          <span>{copy.advancedSettings}</span>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M6 9l6 6 6-6" />
          </svg>
        </button>

        {#if showAdvanced}
          <div class="advanced-panel">
            <div class="settings advanced-rules">
              {#if !usesCrop}
                <label>
                  <span>{upscaleLabel}</span>
                  <select bind:value={settings.allowUpscale}>
                    <option value={false}>{copy.noUpscale}</option>
                    <option value={true}>{upscaleOptionLabel}</option>
                  </select>
                </label>
              {/if}
              {#if usesCrop}
                <label>
                  <span>{copy.horizontalOverflow}</span>
                  <select bind:value={settings.cropHorizontal}>
                    <option value="center">{copy.cropCenter}</option>
                    <option value="left">{copy.cropLeft}</option>
                    <option value="right">{copy.cropRight}</option>
                  </select>
                </label>
                <label>
                  <span>{copy.verticalOverflow}</span>
                  <select bind:value={settings.cropVertical}>
                    <option value="center">{copy.cropCenter}</option>
                    <option value="top">{copy.cropTop}</option>
                    <option value="bottom">{copy.cropBottom}</option>
                  </select>
                </label>
              {/if}
              <label>
                <span>{copy.rotation}</span>
                <select bind:value={settings.rotation}>
                  <option value="auto">{copy.autoRotation}</option>
                  <option value="rotate0">{copy.noRotation}</option>
                  <option value="rotate90">{copy.rotate90}</option>
                  <option value="rotate180">{copy.rotate180}</option>
                  <option value="rotate270">{copy.rotate270}</option>
                </select>
              </label>
              <label>
                <span>{copy.concurrency}</span>
                <input type="number" min="1" max="128" bind:value={settings.concurrency} />
              </label>
              <label>
                <span>{copy.nonImageFiles}</span>
                <select bind:value={settings.copyNonImages}>
                  <option value={false}>{copy.ignoreNonImages}</option>
                  <option value={true}>{copy.copyNonImages}</option>
                </select>
              </label>
              <label>
                <span>{copy.existingFiles}</span>
                <select bind:value={settings.skipExisting}>
                  <option value={true}>{copy.skipExisting}</option>
                  <option value={false}>{copy.overwriteExisting}</option>
                </select>
              </label>
            </div>
          </div>
        {/if}
      </div>
    </section>
  </div>
</section>
