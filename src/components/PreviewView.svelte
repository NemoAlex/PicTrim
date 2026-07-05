<script lang="ts">
  import Panzoom from "@panzoom/panzoom";
  import type { PanzoomObject } from "@panzoom/panzoom";
  import { onDestroy } from "svelte";
  import { formatBytes } from "../lib/format";
  import { loadPreviewTree, renderPreview } from "../lib/tauri";
  import type { Copy } from "../lib/i18n.svelte";
  import type { BatchSettings, PreviewImage, PreviewItem, PreviewPair } from "../lib/types";

  let {
    settings,
    copy,
  }: {
    settings: BatchSettings;
    copy: Copy;
  } = $props();

  interface TreeNode {
    name: string;
    path: string;
    children: TreeNode[];
    item?: PreviewItem;
  }

  let items = $state<PreviewItem[]>([]);
  let tree = $state<TreeNode[]>([]);
  let selectedPath = $state("");
  let pair = $state<PreviewPair | null>(null);
  let treeLoading = $state(true);
  let previewLoading = $state(false);
  let treeError = $state("");
  let previewError = $state("");
  let beforeUrl = $state("");
  let afterUrl = $state("");
  let beforePreviewCanvas = $state<HTMLDivElement | null>(null);
  let afterPreviewCanvas = $state<HTMLDivElement | null>(null);
  let beforePreviewElement = $state<HTMLImageElement | null>(null);
  let afterPreviewElement = $state<HTMLImageElement | null>(null);
  let beforePreviewWidth = $state(0);
  let beforePreviewHeight = $state(0);
  let afterPreviewWidth = $state(0);
  let afterPreviewHeight = $state(0);
  let beforePreviewStartX = 0;
  let beforePreviewStartY = 0;
  let afterPreviewStartX = 0;
  let afterPreviewStartY = 0;
  let previewScale = $state(1);
  let zoomTarget = $state<"before" | "after" | null>(null);
  let zoomDialog = $state<HTMLDialogElement | null>(null);
  let zoomCanvas = $state<HTMLDivElement | null>(null);
  let zoomElement = $state<HTMLImageElement | null>(null);
  let zoomScale = $state(1);
  let zoomDisplayWidth = $state(0);
  let zoomDisplayHeight = $state(0);
  let zoomStartX = 0;
  let zoomStartY = 0;
  let panzoom: PanzoomObject | null = null;
  let beforePreviewPanzoom: PanzoomObject | null = null;
  let afterPreviewPanzoom: PanzoomObject | null = null;
  let syncingPreviewPanzoom = false;
  let treeRequestId = 0;
  let previewRequestId = 0;
  let lastTreeSignature = "";

  const selectedItem = $derived(items.find((item) => item.path === selectedPath) ?? null);
  const zoomImage = $derived(
    zoomTarget === "before" ? (pair?.before ?? null) : zoomTarget === "after" ? (pair?.after ?? null) : null,
  );
  const zoomUrl = $derived(zoomTarget === "before" ? beforeUrl : zoomTarget === "after" ? afterUrl : "");
  const zoomLabel = $derived(
    zoomTarget === "before" ? copy.previewBefore : zoomTarget === "after" ? copy.previewAfter : "",
  );
  const zoomPercent = $derived(`${Math.round(zoomScale * 100)}%`);
  const previewPercent = $derived(`${Math.round(previewScale * 100)}%`);

  onDestroy(() => {
    revokePreviewUrls();
  });

  $effect(() => {
    if (!zoomTarget || !zoomDialog || zoomDialog.open) return;
    zoomDialog.showModal();
  });

  $effect(() => {
    if (!zoomTarget || !zoomCanvas || !zoomElement) return;
    setupPanzoom();
  });

  $effect(() => {
    if (!pair || !beforeUrl || !afterUrl || !beforePreviewCanvas || !afterPreviewCanvas || !beforePreviewElement || !afterPreviewElement) {
      return;
    }
    setupPreviewPanzooms();
  });

  $effect(() => {
    const signature = `${settings.inputSources.join("\n")}\n${settings.outputDir}`;
    if (signature === lastTreeSignature) return;
    lastTreeSignature = signature;
    resetPreviewSelection();
    refreshTree();
  });

  function resetPreviewSelection() {
    selectedPath = "";
    pair = null;
    previewError = "";
    revokePreviewUrls();
  }

  async function refreshTree() {
    const currentRequest = ++treeRequestId;
    treeLoading = true;
    treeError = "";
    previewError = "";
    try {
      const next = await loadPreviewTree(settings);
      if (currentRequest !== treeRequestId) return;
      items = next.items;
      tree = buildTree(next.items);
      if (next.items.length === 0 || !next.items.some((item) => item.path === selectedPath)) resetPreviewSelection();
    } catch (error) {
      if (currentRequest !== treeRequestId) return;
      items = [];
      tree = [];
      resetPreviewSelection();
      treeError = String(error);
    } finally {
      if (currentRequest === treeRequestId) treeLoading = false;
    }
  }

  function selectFile(item: PreviewItem) {
    if (selectedPath === item.path && previewLoading) return;
    selectedPath = item.path;
    loadSelectedPreview(item.path);
  }

  async function loadSelectedPreview(path: string) {
    const currentRequest = ++previewRequestId;
    previewLoading = true;
    previewError = "";
    pair = null;
    revokePreviewUrls();
    try {
      const next = await renderPreview(settings, path);
      if (currentRequest !== previewRequestId) return;
      pair = next;
      beforeUrl = imageUrl(next.before);
      afterUrl = imageUrl(next.after);
    } catch (error) {
      if (currentRequest !== previewRequestId) return;
      previewError = errorMessage(error);
    } finally {
      if (currentRequest === previewRequestId) previewLoading = false;
    }
  }

  function buildTree(nextItems: PreviewItem[]): TreeNode[] {
    const roots: TreeNode[] = [];
    for (const item of nextItems) {
      let level = roots;
      const segments = item.segments.length > 0 ? item.segments : [item.name];
      segments.forEach((segment, index) => {
        const nodePath = segments.slice(0, index + 1).join("/");
        let node = level.find((candidate) => candidate.name === segment);
        if (!node) {
          node = { name: segment, path: nodePath, children: [] };
          level.push(node);
        }
        if (index === segments.length - 1) node.item = item;
        level = node.children;
      });
    }
    sortNodes(roots);
    return roots;
  }

  function sortNodes(nodes: TreeNode[]) {
    nodes.sort((left, right) => {
      if (Boolean(left.item) !== Boolean(right.item)) return left.item ? 1 : -1;
      return left.name.localeCompare(right.name);
    });
    for (const node of nodes) sortNodes(node.children);
  }

  function imageUrl(image: PreviewImage): string {
    const blob = new Blob([base64ToArrayBuffer(image.data)], { type: image.mime });
    return URL.createObjectURL(blob);
  }

  function base64ToArrayBuffer(data: string): ArrayBuffer {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }
    return bytes.buffer;
  }

  function revokePreviewUrls() {
    zoomTarget = null;
    destroyPanzoom();
    destroyPreviewPanzooms();
    if (beforeUrl) URL.revokeObjectURL(beforeUrl);
    if (afterUrl) URL.revokeObjectURL(afterUrl);
    beforeUrl = "";
    afterUrl = "";
  }

  function openZoom(target: "before" | "after") {
    resetZoomCanvas();
    zoomTarget = target;
  }

  function closeZoom() {
    if (zoomDialog?.open) zoomDialog.close();
    zoomTarget = null;
    destroyPanzoom();
    resetZoomCanvas();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeZoom();
  }

  function resetZoomCanvas() {
    panzoom?.reset({ animate: false, startX: zoomStartX, startY: zoomStartY });
    zoomScale = 1;
  }

  function setZoomScale(nextScale: number) {
    const scale = Math.min(6, Math.max(0.25, nextScale));
    panzoom?.zoom(scale, { animate: false });
    zoomScale = panzoom?.getScale() ?? scale;
  }

  function zoomIn() {
    setZoomScale(zoomScale * 1.25);
  }

  function zoomOut() {
    setZoomScale(zoomScale / 1.25);
  }

  function handleZoomWheel(event: WheelEvent) {
    if (!panzoom) return;
    panzoom.zoomWithWheel(event);
    zoomScale = panzoom.getScale();
  }

  function setupPanzoom() {
    destroyPanzoom();
    if (!zoomElement || !zoomCanvas || !zoomImage) return;
    requestAnimationFrame(() => {
      fitZoomImage();
      if (!zoomElement || !zoomCanvas) return;
      panzoom = Panzoom(zoomElement, {
        animate: false,
        canvas: true,
        cursor: "grab",
        maxScale: 8,
        minScale: 0.25,
        startX: zoomStartX,
        startY: zoomStartY,
        step: 0.24,
      });
      zoomCanvas.addEventListener("wheel", handleZoomWheel, { passive: false });
      zoomElement.addEventListener("panzoomchange", updateZoomScale);
      panzoom.reset({ animate: false, startX: zoomStartX, startY: zoomStartY });
      zoomScale = panzoom.getScale();
    });
  }

  function destroyPanzoom() {
    if (zoomCanvas) zoomCanvas.removeEventListener("wheel", handleZoomWheel);
    if (zoomElement) zoomElement.removeEventListener("panzoomchange", updateZoomScale);
    panzoom?.destroy();
    panzoom = null;
  }

  function setupPreviewPanzooms() {
    destroyPreviewPanzooms();
    if (!pair || !beforePreviewCanvas || !afterPreviewCanvas || !beforePreviewElement || !afterPreviewElement) return;
    requestAnimationFrame(() => {
      if (!pair || !beforePreviewCanvas || !afterPreviewCanvas || !beforePreviewElement || !afterPreviewElement) return;
      const beforeFit = fitImageInCanvas(pair.before, beforePreviewCanvas);
      const afterFit = fitImageInCanvas(pair.after, afterPreviewCanvas);
      beforePreviewWidth = beforeFit.width;
      beforePreviewHeight = beforeFit.height;
      afterPreviewWidth = afterFit.width;
      afterPreviewHeight = afterFit.height;
      beforePreviewStartX = beforeFit.x;
      beforePreviewStartY = beforeFit.y;
      afterPreviewStartX = afterFit.x;
      afterPreviewStartY = afterFit.y;
      requestAnimationFrame(() => {
        if (!beforePreviewCanvas || !afterPreviewCanvas || !beforePreviewElement || !afterPreviewElement) return;
        beforePreviewPanzoom = Panzoom(beforePreviewElement, {
          animate: false,
          canvas: true,
          cursor: "grab",
          maxScale: 8,
          minScale: 1,
          startX: beforeFit.x,
          startY: beforeFit.y,
          step: 0.24,
        });
        afterPreviewPanzoom = Panzoom(afterPreviewElement, {
          animate: false,
          canvas: true,
          cursor: "grab",
          maxScale: 8,
          minScale: 1,
          startX: afterFit.x,
          startY: afterFit.y,
          step: 0.24,
        });
        beforePreviewCanvas.addEventListener("wheel", handleBeforePreviewWheel, { passive: false });
        afterPreviewCanvas.addEventListener("wheel", handleAfterPreviewWheel, { passive: false });
        beforePreviewElement.addEventListener("panzoomchange", syncPreviewFromBefore);
        afterPreviewElement.addEventListener("panzoomchange", syncPreviewFromAfter);
        beforePreviewPanzoom.reset({ animate: false, startX: beforeFit.x, startY: beforeFit.y });
        afterPreviewPanzoom.reset({ animate: false, startX: afterFit.x, startY: afterFit.y });
      });
    });
  }

  function destroyPreviewPanzooms() {
    if (beforePreviewCanvas) beforePreviewCanvas.removeEventListener("wheel", handleBeforePreviewWheel);
    if (afterPreviewCanvas) afterPreviewCanvas.removeEventListener("wheel", handleAfterPreviewWheel);
    if (beforePreviewElement) beforePreviewElement.removeEventListener("panzoomchange", syncPreviewFromBefore);
    if (afterPreviewElement) afterPreviewElement.removeEventListener("panzoomchange", syncPreviewFromAfter);
    beforePreviewPanzoom?.destroy();
    afterPreviewPanzoom?.destroy();
    beforePreviewPanzoom = null;
    afterPreviewPanzoom = null;
    syncingPreviewPanzoom = false;
    previewScale = 1;
  }

  function fitImageInCanvas(image: PreviewImage, canvas: HTMLElement) {
    const width = Math.max(1, canvas.clientWidth);
    const height = Math.max(1, canvas.clientHeight);
    const scale = Math.min(width / image.width, height / image.height, 1);
    const displayWidth = Math.max(1, Math.round(image.width * scale));
    const displayHeight = Math.max(1, Math.round(image.height * scale));
    return {
      height: displayHeight,
      width: displayWidth,
      x: Math.round((width - displayWidth) / 2),
      y: Math.round((height - displayHeight) / 2),
    };
  }

  function handleBeforePreviewWheel(event: WheelEvent) {
    if (!beforePreviewPanzoom) return;
    beforePreviewPanzoom.zoomWithWheel(event);
    previewScale = beforePreviewPanzoom.getScale();
  }

  function handleAfterPreviewWheel(event: WheelEvent) {
    if (!afterPreviewPanzoom) return;
    afterPreviewPanzoom.zoomWithWheel(event);
    previewScale = afterPreviewPanzoom.getScale();
  }

  function previewZoomIn(source: "before" | "after") {
    zoomPreviewFrom(source, 1.25);
  }

  function previewZoomOut(source: "before" | "after") {
    zoomPreviewFrom(source, 1 / 1.25);
  }

  function zoomPreviewFrom(source: "before" | "after", factor: number) {
    const sourcePanzoom = source === "before" ? beforePreviewPanzoom : afterPreviewPanzoom;
    if (!sourcePanzoom) return;
    sourcePanzoom.zoom(Math.min(8, Math.max(1, sourcePanzoom.getScale() * factor)), {
      animate: false,
      force: true,
    });
    previewScale = sourcePanzoom.getScale();
    source === "before" ? syncPreviewFromBefore() : syncPreviewFromAfter();
  }

  function resetPreviewPanzooms() {
    beforePreviewPanzoom?.reset({
      animate: false,
      startX: beforePreviewStartX,
      startY: beforePreviewStartY,
    });
    afterPreviewPanzoom?.reset({
      animate: false,
      startX: afterPreviewStartX,
      startY: afterPreviewStartY,
    });
    previewScale = 1;
  }

  function syncPreviewFromBefore() {
    syncPreviewPanzoom(
      beforePreviewPanzoom,
      afterPreviewPanzoom,
      beforePreviewStartX,
      beforePreviewStartY,
      afterPreviewStartX,
      afterPreviewStartY,
    );
  }

  function syncPreviewFromAfter() {
    syncPreviewPanzoom(
      afterPreviewPanzoom,
      beforePreviewPanzoom,
      afterPreviewStartX,
      afterPreviewStartY,
      beforePreviewStartX,
      beforePreviewStartY,
    );
  }

  function syncPreviewPanzoom(
    source: PanzoomObject | null,
    target: PanzoomObject | null,
    sourceStartX: number,
    sourceStartY: number,
    targetStartX: number,
    targetStartY: number,
  ) {
    if (!source || !target || syncingPreviewPanzoom) return;
    syncingPreviewPanzoom = true;
    const pan = source.getPan();
    const scale = source.getScale();
    previewScale = scale;
    target.zoom(scale, { animate: false, force: true, silent: true });
    target.pan(targetStartX + pan.x - sourceStartX, targetStartY + pan.y - sourceStartY, {
      animate: false,
      force: true,
      silent: true,
    });
    syncingPreviewPanzoom = false;
  }

  function fitZoomImage() {
    if (!zoomCanvas || !zoomImage) return;
    const maxWidth = Math.max(1, zoomCanvas.clientWidth);
    const maxHeight = Math.max(1, zoomCanvas.clientHeight);
    const fitScale = Math.min(maxWidth / zoomImage.width, maxHeight / zoomImage.height, 1);
    zoomDisplayWidth = Math.max(1, Math.round(zoomImage.width * fitScale));
    zoomDisplayHeight = Math.max(1, Math.round(zoomImage.height * fitScale));
    zoomStartX = Math.round((zoomCanvas.clientWidth - zoomDisplayWidth) / 2);
    zoomStartY = Math.round((zoomCanvas.clientHeight - zoomDisplayHeight) / 2);
  }

  function updateZoomScale() {
    zoomScale = panzoom?.getScale() ?? 1;
  }

  function imageSize(image: PreviewImage): string {
    return `${image.width} x ${image.height}`;
  }

  function imageFormat(image: PreviewImage): string {
    switch (image.mime) {
      case "image/jpeg":
        return "JPG";
      case "image/png":
        return "PNG";
      case "image/webp":
        return "WebP";
      default:
        return image.mime.replace(/^image\//, "").toUpperCase();
    }
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#snippet treeNodes(nodes: TreeNode[], depth = 0)}
  {#each nodes as node (node.path)}
    {#if node.item}
      <button
        class:active={selectedPath === node.item.path}
        class="preview-tree-file"
        style={`padding-left: ${10 + depth * 16}px;`}
        title={node.item.rel}
        onclick={() => node.item && selectFile(node.item)}
      >
        <span>{node.name}</span>
      </button>
    {:else}
      <div class="preview-tree-folder" style={`padding-left: ${10 + depth * 16}px;`}>
        <span>{node.name}</span>
      </div>
      {@render treeNodes(node.children, depth + 1)}
    {/if}
  {/each}
{/snippet}

<section class="preview-layout">
  <aside class="preview-sidebar">
    <div class="preview-sidebar-head">
      <h2>{copy.previewTitle}</h2>
      <span>{copy.previewImageCount(items.length)}</span>
    </div>

    <div class="preview-tree" aria-label={copy.previewTreeLabel}>
      {#if treeLoading}
        <div class="preview-state">{copy.previewLoading}</div>
      {:else if treeError}
        <div class="preview-state error">{treeError}</div>
      {:else if items.length === 0}
        <div class="preview-state">{copy.previewEmpty}</div>
      {:else}
        {@render treeNodes(tree)}
      {/if}
    </div>
  </aside>

  <section class="preview-main">
    {#if treeLoading}
      <div class="preview-empty-panel">{copy.previewLoading}</div>
    {:else if !selectedItem}
      <div class="preview-empty-panel">{copy.previewSelectHint}</div>
    {:else}
      <div class="preview-main-head">
        <h2>{selectedItem?.rel ?? copy.previewTitle}</h2>
      </div>

      {#if previewLoading}
        <div class="preview-empty-panel">{copy.previewRendering}</div>
      {:else if previewError}
        <div class="preview-empty-panel error">{previewError}</div>
      {:else if pair && beforeUrl && afterUrl}
        <div class="preview-compare">
          <article class="preview-image-panel">
            <div class="preview-panel-toolbar">
              <div>
                <button type="button" aria-label={copy.previewZoomOut} title={copy.previewZoomOut} onclick={() => previewZoomOut("before")}>-</button>
                <button type="button" aria-label={copy.previewZoomReset} title={copy.previewZoomReset} onclick={resetPreviewPanzooms}>{previewPercent}</button>
                <button type="button" aria-label={copy.previewZoomIn} title={copy.previewZoomIn} onclick={() => previewZoomIn("before")}>+</button>
                <button type="button" aria-label={copy.previewZoom} title={copy.previewZoom} onclick={() => openZoom("before")}>
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <circle cx="11" cy="11" r="6"></circle>
                    <path d="M16 16l5 5"></path>
                  </svg>
                </button>
              </div>
            </div>
            <div bind:this={beforePreviewCanvas} class="preview-image-wrap">
              <img
                bind:this={beforePreviewElement}
                src={beforeUrl}
                alt={copy.previewBefore}
                draggable="false"
                style={`width: ${beforePreviewWidth}px; height: ${beforePreviewHeight}px;`}
              />
            </div>
            <div class="preview-meta">
              <strong>{copy.previewBefore}</strong>
              <span>{imageFormat(pair.before)}</span>
              <span>{imageSize(pair.before)}</span>
              <span>{formatBytes(pair.before.bytes)}</span>
            </div>
          </article>

          <article class="preview-image-panel">
            <div class="preview-panel-toolbar">
              <div>
                <button type="button" aria-label={copy.previewZoomOut} title={copy.previewZoomOut} onclick={() => previewZoomOut("after")}>-</button>
                <button type="button" aria-label={copy.previewZoomReset} title={copy.previewZoomReset} onclick={resetPreviewPanzooms}>{previewPercent}</button>
                <button type="button" aria-label={copy.previewZoomIn} title={copy.previewZoomIn} onclick={() => previewZoomIn("after")}>+</button>
                <button type="button" aria-label={copy.previewZoom} title={copy.previewZoom} onclick={() => openZoom("after")}>
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <circle cx="11" cy="11" r="6"></circle>
                    <path d="M16 16l5 5"></path>
                  </svg>
                </button>
              </div>
            </div>
            <div bind:this={afterPreviewCanvas} class="preview-image-wrap">
              <img
                bind:this={afterPreviewElement}
                src={afterUrl}
                alt={copy.previewAfter}
                draggable="false"
                style={`width: ${afterPreviewWidth}px; height: ${afterPreviewHeight}px;`}
              />
            </div>
            <div class="preview-meta">
              <strong>{copy.previewAfter}</strong>
              <span>{imageFormat(pair.after)}</span>
              <span>{imageSize(pair.after)}</span>
              <span>{formatBytes(pair.after.bytes)}</span>
            </div>
          </article>
        </div>
      {/if}
    {/if}
  </section>
</section>

{#if zoomImage && zoomUrl}
  <dialog
    bind:this={zoomDialog}
    class="preview-zoom-dialog"
    aria-label={`${zoomLabel} ${copy.previewZoom}`}
    onclose={closeZoom}
    onclick={(event) => {
      if (event.target === zoomDialog) closeZoom();
    }}
  >
    <header class="preview-zoom-head">
      <div class="preview-zoom-title">
        <h2>{zoomLabel}</h2>
        <p>{selectedItem?.rel ?? ""}</p>
      </div>
      <div class="preview-zoom-meta">
        <span>{imageFormat(zoomImage)}</span>
        <span>{imageSize(zoomImage)}</span>
        <span>{formatBytes(zoomImage.bytes)}</span>
      </div>
      <div class="preview-zoom-controls">
        <button type="button" aria-label={copy.previewZoomOut} title={copy.previewZoomOut} onclick={zoomOut}>-</button>
        <button type="button" aria-label={copy.previewZoomReset} title={copy.previewZoomReset} onclick={resetZoomCanvas}>
          {zoomPercent}
        </button>
        <button type="button" aria-label={copy.previewZoomIn} title={copy.previewZoomIn} onclick={zoomIn}>+</button>
        <button class="preview-zoom-close" type="button" aria-label={copy.closePreviewZoom} onclick={closeZoom}>
          x
        </button>
      </div>
    </header>
    <div bind:this={zoomCanvas} class="preview-zoom-canvas" role="presentation">
      <img
        bind:this={zoomElement}
        src={zoomUrl}
        alt={zoomLabel}
        draggable="false"
        style={`width: ${zoomDisplayWidth}px; height: ${zoomDisplayHeight}px;`}
      />
    </div>
  </dialog>
{/if}
