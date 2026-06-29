<script lang="ts">
  import { onMount } from "svelte";
  import {
    classifySources,
    onSourceDrop,
    pickDirectories,
    pickDirectory,
    pickFiles,
  } from "../lib/tauri";
  import type { BatchSettings, SourceEntry } from "../lib/types";

  let {
    settings = $bindable(),
  }: {
    settings: BatchSettings;
  } = $props();

  let sources = $state<SourceEntry[]>([]);
  let dropActive = $state(false);
  let cropPreviewWide = $state(true);
  let longestSideWide = $state(true);

  const processingMode = $derived(settings.resizeMode);
  const usesBoxSize = $derived(processingMode === "fitBox" || processingMode === "fixedCrop");
  const usesWidth = $derived(processingMode === "fitWidth");
  const usesHeight = $derived(processingMode === "fitHeight");
  const usesCrop = $derived(processingMode === "fixedCrop");
  const previewTitle = $derived(previewCopy(processingMode).title);
  const previewDetail = $derived(previewCopy(processingMode).detail);
  const upscaleLabel = $derived(
    processingMode === "fitLongestSide"
      ? "长边不足时"
      : processingMode === "fitWidth"
        ? "宽度不足时"
        : processingMode === "fitHeight"
          ? "高度不足时"
          : "尺寸不足时",
  );
  const upscaleOptionLabel = $derived(
    processingMode === "fitLongestSide"
      ? "放大到目标尺寸"
      : processingMode === "fitWidth"
        ? "放大到目标宽度"
        : processingMode === "fitHeight"
          ? "放大到目标高度"
          : "放大到一边满足尺寸",
  );

  onMount(() => {
    refreshSources();
    const unlisteners: Array<() => void> = [];
    onSourceDrop(addSources).then((unlisten) => unlisteners.push(unlisten));
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  });

  $effect(() => {
    const snapshot = settings.inputSources.join("\n");
    refreshSources(snapshot);
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
  }

  function sourceName(path: string): string {
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }

  function kindLabel(kind: SourceEntry["kind"]): string {
    if (kind === "directory") return "目录";
    if (kind === "file") return "文件";
    return "缺失";
  }

  function previewCopy(mode: BatchSettings["resizeMode"]) {
    if (mode === "fitBox") {
      return {
        title: "等比例缩放，限制宽高",
        detail: "按最大宽度和最大高度限制图片，保持完整画面，不裁剪。",
      };
    }
    if (mode === "fitWidth") {
      return {
        title: "缩放到指定宽度",
        detail: "宽度缩放到目标值，高度按比例自动计算。",
      };
    }
    if (mode === "fitHeight") {
      return {
        title: "缩放到指定高度",
        detail: "高度缩放到目标值，宽度按比例自动计算。",
      };
    }
    if (mode === "fixedCrop") {
      return {
        title: "缩放到固定宽高并裁剪",
        detail: "先等比铺满目标尺寸，再按定位方式裁掉多余部分。",
      };
    }
    return {
      title: "等比例缩放，限制长边",
      detail: "只限制图片最长的一边，另一边按比例自动计算。",
    };
  }

  function sizeLabel(value: number): string {
    const numberValue = Number(value);
    return Number.isFinite(numberValue) && numberValue > 0 ? `${numberValue}px` : "-";
  }

  function widthMeasure(mode: BatchSettings["resizeMode"]): string {
    if (mode === "fitWidth" || mode === "fixedCrop" || mode === "fitBox") return sizeLabel(settings.width);
    if (mode === "fitLongestSide" && longestSideWide) return sizeLabel(settings.maxSide);
    return "弹性";
  }

  function heightMeasure(mode: BatchSettings["resizeMode"]): string {
    if (mode === "fitHeight" || mode === "fixedCrop" || mode === "fitBox") return sizeLabel(settings.height);
    if (mode === "fitLongestSide" && !longestSideWide) return sizeLabel(settings.maxSide);
    return "弹性";
  }

  function cropHorizontalText(value: BatchSettings["cropHorizontal"]): string {
    if (value === "left") return "左";
    if (value === "right") return "右";
    return "中";
  }

  function cropVerticalText(value: BatchSettings["cropVertical"]): string {
    if (value === "top") return "上";
    if (value === "bottom") return "下";
    return "中";
  }

  function updateProcessingMode(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value as BatchSettings["resizeMode"];
    settings.thumbnail = false;
    settings.resizeMode = value;
  }
</script>

<section class="panel panel-card setup-panel">
  <div class="setup-head">
    <h2>批量处理图片</h2>
    <p>拖入文件或目录，选择输出位置并设置处理参数。</p>
  </div>

  <div class="config-layout">
    <div class="config-column">
      <div class="field-group">
        <h3 class="group-title">输入来源</h3>
        <div class="source-section">
          <div
            class:active={dropActive}
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
                <strong>{settings.inputSources.length > 0 ? `${settings.inputSources.length} 个输入来源` : "拖入文件或目录"}</strong>
              </div>
              {#if sources.length > 0}
                <button class="secondary clear-button" onclick={clearSources}>清空</button>
              {/if}
            </div>

            <div class="source-list">
              {#if sources.length > 0}
                {#each sources as source}
                  <div class:missing={source.kind === "missing"} class="source-item">
                    <span class="source-kind">{kindLabel(source.kind)}</span>
                    <span class="source-name" title={source.path}>{sourceName(source.path)}</span>
                    <button class="icon-button" aria-label="移除来源" title="移除来源" onclick={() => removeSource(source.path)}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true">
                        <path d="M18 6 6 18M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                {/each}
              {:else}
                <div class="source-empty">
                  <span>把文件、文件夹拖到这里</span>
                </div>
              {/if}
            </div>

            <div class="source-actions">
              <button class="secondary" onclick={addFiles}>添加文件</button>
              <button class="secondary" onclick={addDirectories}>添加目录</button>
            </div>
          </div>
        </div>
      </div>

      <div class="field-group">
        <h3 class="group-title">输出位置</h3>
        <div class="paths">
          <label>
            <div class="path-row">
              <input readonly placeholder="选择保存结果的文件夹" value={settings.outputDir} />
              <button class="secondary" onclick={pickOutput}>选择</button>
            </div>
          </label>
        </div>
      </div>

      <div class="field-group batch-fields">
        <div class="settings settings-3">
          <label class="wide-field">
            <span>并发数</span>
            <input type="number" min="1" max="128" bind:value={settings.concurrency} />
          </label>
          <label>
            <span>非图片文件</span>
            <select bind:value={settings.copyNonImages}>
              <option value={false}>忽略，不处理</option>
              <option value={true}>复制到目标目录</option>
            </select>
          </label>
          <label>
            <span>已存在文件</span>
            <select bind:value={settings.skipExisting}>
              <option value={true}>跳过，保留已有</option>
              <option value={false}>覆盖，重新生成</option>
            </select>
          </label>
        </div>
      </div>
    </div>

    <div class="config-column config-column-params">
      <div class="field-group">
        <h3 class="group-title">尺寸与变换</h3>

        <div class="resize-preview" data-mode={processingMode}>
          <div class="diagram-art">
            <div aria-hidden="true">
              {#key processingMode + (processingMode === "fitLongestSide" ? String(longestSideWide) : "")}
                <div class="motion-canvas"
                  data-longest-preview={processingMode === "fitLongestSide" ? (longestSideWide ? "landscape" : "portrait") : undefined}
                  data-crop-preview={processingMode === "fixedCrop" ? (cropPreviewWide ? "wide" : "tall") : undefined}
                  data-crop-x={processingMode === "fixedCrop" ? settings.cropHorizontal : undefined}
                  data-crop-y={processingMode === "fixedCrop" ? settings.cropVertical : undefined}>
                <div class="motion-photo">
                  <span class="photo-sun"></span>
                  <span class="photo-ridge photo-ridge-back"></span>
                  <span class="photo-ridge photo-ridge-front"></span>
                  <span class="photo-shine"></span>
                </div>
                {#if processingMode === "fixedCrop"}
                  <div class="crop-window"></div>
                {/if}
                <span class="measure-line measure-width"></span>
                <span class="measure-line measure-height"></span>
                <span class="measure-label measure-label-width">宽：{widthMeasure(processingMode)}</span>
                <span class="measure-label measure-label-height">高：{heightMeasure(processingMode)}</span>

              </div>
            {/key}
            </div>
          </div>
          <div class="preview-copy">
            <strong>{previewTitle}</strong>
            <span>{previewDetail}</span>
            {#if processingMode === "fitLongestSide"}
              <div class="crop-preview-toggle">
                <strong class="crop-preview-label">示例情形</strong>
                <div>
                  <button class:active={longestSideWide} onclick={() => (longestSideWide = true)}>横向图片</button>
                  <button class:active={!longestSideWide} onclick={() => (longestSideWide = false)}>纵向图片</button>
                </div>
              </div>
            {:else if processingMode === "fixedCrop"}
              <div class="crop-preview-toggle">
                <strong class="crop-preview-label">示例情形</strong>
                <div>
                  <button class:active={cropPreviewWide} onclick={() => (cropPreviewWide = true)}>原图过宽</button>
                  <button class:active={!cropPreviewWide} onclick={() => (cropPreviewWide = false)}>原图过高</button>
                </div>
              </div>
            {/if}
          </div>
        </div>

        <div class="settings settings-3">
          <label class="wide-field">
            <span>处理方式</span>
            <select value={processingMode} onchange={updateProcessingMode}>
              <option value="fitLongestSide">等比例缩放，限制长边</option>
              <option value="fitBox">等比例缩放，限制宽高</option>
              <option value="fitWidth">缩放到指定宽度</option>
              <option value="fitHeight">缩放到指定高度</option>
              <option value="fixedCrop">缩放到固定宽高，裁剪多余部分</option>
            </select>
          </label>
          {#if usesBoxSize}
            <label>
              <span>最大宽度 (px)</span>
              <input type="number" min="1" max="50000" bind:value={settings.width} />
            </label>
            <label>
              <span>最大高度 (px)</span>
              <input type="number" min="1" max="50000" bind:value={settings.height} />
            </label>
          {:else}
            {#if usesWidth}
              <label>
                <span>目标宽度 (px)</span>
                <input type="number" min="1" max="50000" bind:value={settings.width} />
              </label>
            {:else if usesHeight}
              <label>
                <span>目标高度 (px)</span>
                <input type="number" min="1" max="50000" bind:value={settings.height} />
              </label>
            {:else}
              <label>
                <span>最长边 (px)</span>
                <input type="number" min="1" max="50000" bind:value={settings.maxSide} />
              </label>
            {/if}
          {/if}
          {#if !usesCrop}
            <label>
              <span>{upscaleLabel}</span>
              <select bind:value={settings.allowUpscale}>
                <option value={false}>不放大</option>
                <option value={true}>{upscaleOptionLabel}</option>
              </select>
            </label>
          {/if}
          {#if usesCrop}
            <label>
              <span>横向超出时</span>
              <select bind:value={settings.cropHorizontal}>
                <option value="center">截取中间</option>
                <option value="left">截左边</option>
                <option value="right">截右边</option>
              </select>
            </label>
            <label>
              <span>纵向超出时</span>
              <select bind:value={settings.cropVertical}>
                <option value="center">截取中间</option>
                <option value="top">截上面</option>
                <option value="bottom">截下面</option>
              </select>
            </label>
          {/if}
          <label>
            <span>旋转</span>
            <select bind:value={settings.rotation}>
              <option value="auto">EXIF自动校正</option>
              <option value="rotate0">不旋转</option>
              <option value="rotate90">顺时针 90°</option>
              <option value="rotate180">180°</option>
              <option value="rotate270">逆时针 90°</option>
            </select>
          </label>
        </div>
      </div>

      <div class="field-group">
        <div class="settings settings-3">
          <label>
            <span>质量</span>
            <input type="number" min="1" max="100" bind:value={settings.quality} />
          </label>
          <label>
            <span>输出格式</span>
            <select bind:value={settings.outputFormat}>
              <option value="jpg">JPG</option>
              <option value="png">PNG</option>
              <option value="webp">WebP</option>
              <option value="keep">保持原格式</option>
            </select>
          </label>
        </div>
      </div>
    </div>
  </div>
</section>
