<script lang="ts">
  import Panzoom from "@panzoom/panzoom";
  import type { PanzoomObject } from "@panzoom/panzoom";
  import { Minus, Plus, Search, X } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import { formatBytes } from "../lib/format";
  import { loadPreviewDirectory, renderPreview } from "../lib/tauri";
  import type { Copy } from "../lib/i18n.svelte";
  import type { BatchSettings, PreviewDirectoryEntry, PreviewImage, PreviewPair } from "../lib/types";

  let {
    settings,
    copy,
  }: {
    settings: BatchSettings;
    copy: Copy;
  } = $props();

  interface DirectoryNode {
    entry: PreviewDirectoryEntry;
    depth: number;
    expanded: boolean;
    loading: boolean;
    error: string;
    children: DirectoryNode[];
    nextOffset: number | null;
  }

  type VisibleRow = { kind: "entry"; node: DirectoryNode } | { kind: "more"; node: DirectoryNode };

  const PAGE_LIMIT = 300;
  const TREE_ROW_HEIGHT = 32;
  const TREE_OVERSCAN = 8;

  let rootNodes = $state<DirectoryNode[]>([]);
  let rootNextOffset = $state<number | null>(null);
  let rootLoadingMore = $state(false);
  let treeViewport = $state<HTMLDivElement | null>(null);
  let treeScrollTop = $state(0);
  let treeViewportHeight = $state(0);
  let selectedPath = $state("");
  let selectedFile = $state<PreviewDirectoryEntry | null>(null);
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

  const visibleRows = $derived(flattenVisibleRows(rootNodes));
  const treeStartIndex = $derived(Math.max(0, Math.floor(treeScrollTop / TREE_ROW_HEIGHT) - TREE_OVERSCAN));
  const treeVisibleCount = $derived(Math.ceil(treeViewportHeight / TREE_ROW_HEIGHT) + TREE_OVERSCAN * 2);
  const treeEndIndex = $derived(Math.min(visibleRows.length, treeStartIndex + treeVisibleCount));
  const treeWindowRows = $derived(visibleRows.slice(treeStartIndex, treeEndIndex));
  const selectedItem = $derived(selectedFile);
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
    requestAnimationFrame(() => (zoomCanvas ?? zoomDialog)?.focus({ preventScroll: true }));
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
    if (treeViewport) treeViewportHeight = treeViewport.clientHeight;
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
    rootLoadingMore = false;
    treeError = "";
    previewError = "";
    try {
      const next = await loadPreviewDirectory(settings, null, 0, PAGE_LIMIT);
      if (currentRequest !== treeRequestId) return;
      rootNodes = next.entries.map((entry) => createDirectoryNode(entry, 0));
      rootNextOffset = next.nextOffset ?? null;
      treeScrollTop = 0;
      if (!selectedFile || !next.entries.some((entry) => entry.kind === "file" && entry.path === selectedFile?.path)) {
        resetPreviewSelection();
        const firstFile = next.entries.find((entry) => entry.kind === "file");
        if (firstFile) selectFile(firstFile);
      }
    } catch (error) {
      if (currentRequest !== treeRequestId) return;
      rootNodes = [];
      rootNextOffset = null;
      resetPreviewSelection();
      treeError = String(error);
    } finally {
      if (currentRequest === treeRequestId) treeLoading = false;
    }
  }

  function selectFile(entry: PreviewDirectoryEntry) {
    if (selectedPath === entry.path && previewLoading) return;
    selectedFile = entry;
    selectedPath = entry.path;
    loadSelectedPreview(entry.path);
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

  function createDirectoryNode(entry: PreviewDirectoryEntry, depth: number): DirectoryNode {
    return {
      children: [],
      depth,
      entry,
      error: "",
      expanded: false,
      loading: false,
      nextOffset: null,
    };
  }

  function flattenVisibleRows(nodes: DirectoryNode[]): VisibleRow[] {
    const rows: VisibleRow[] = [];
    for (const node of nodes) {
      rows.push({ kind: "entry", node });
      if (node.entry.kind === "directory" && node.expanded) {
        rows.push(...flattenVisibleRows(node.children));
        if (node.nextOffset !== null || node.loading) rows.push({ kind: "more", node });
      }
    }
    if (rootNextOffset !== null) {
      rows.push({
        kind: "more",
        node: {
          children: rootNodes,
          depth: 0,
          entry: { kind: "directory", name: copy.previewTitle, path: "", rel: "" },
          error: "",
          expanded: true,
          loading: rootLoadingMore,
          nextOffset: rootNextOffset,
        },
      });
    }
    return rows;
  }

  async function toggleDirectory(node: DirectoryNode) {
    if (node.entry.kind !== "directory") return;
    node.expanded = !node.expanded;
    rootNodes = [...rootNodes];
    if (node.expanded && node.children.length === 0 && node.nextOffset === null && !node.loading) {
      await loadDirectoryChildren(node, 0);
    }
  }

  async function loadDirectoryChildren(node: DirectoryNode, offset: number) {
    node.loading = true;
    node.error = "";
    rootNodes = [...rootNodes];
    try {
      const page = await loadPreviewDirectory(settings, node.entry.path || null, offset, PAGE_LIMIT);
      const nextChildren = page.entries.map((entry) => createDirectoryNode(entry, node.depth + 1));
      node.children = offset === 0 ? nextChildren : [...node.children, ...nextChildren];
      node.nextOffset = page.nextOffset ?? null;
    } catch (error) {
      node.error = errorMessage(error);
    } finally {
      node.loading = false;
      rootNodes = [...rootNodes];
    }
  }

  async function loadRootMore() {
    if (rootNextOffset === null || rootLoadingMore) return;
    rootLoadingMore = true;
    try {
      const page = await loadPreviewDirectory(settings, null, rootNextOffset, PAGE_LIMIT);
      rootNodes = [...rootNodes, ...page.entries.map((entry) => createDirectoryNode(entry, 0))];
      rootNextOffset = page.nextOffset ?? null;
    } catch (error) {
      treeError = errorMessage(error);
    } finally {
      rootLoadingMore = false;
    }
  }

  function handleTreeScroll(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLElement)) return;
    treeScrollTop = target.scrollTop;
    treeViewportHeight = target.clientHeight;
  }

  function measureTreeViewport(node: HTMLDivElement) {
    treeViewportHeight = node.clientHeight;
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

  function releasePointerFocus(event: PointerEvent) {
    if (event.pointerType === "mouse" || event.pointerType === "touch") {
      (event.currentTarget as HTMLButtonElement | null)?.blur();
    }
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

  function isImageEntry(entry: PreviewDirectoryEntry): boolean {
    return entry.kind === "file" && /\.(avif|gif|jpe?g|png|tiff?|webp)$/i.test(entry.name);
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<section class="preview-layout">
  <aside class="preview-sidebar">
    <div class="preview-sidebar-head">
      <h2>{copy.previewTitle}</h2>
    </div>

    <div bind:this={treeViewport} class="preview-tree" aria-label={copy.previewTreeLabel} onscroll={handleTreeScroll}>
      {#if treeLoading}
        <div class="preview-state">{copy.previewLoading}</div>
      {:else if treeError}
        <div class="preview-state error">{treeError}</div>
      {:else if visibleRows.length === 0}
        <div class="preview-state">{copy.previewEmpty}</div>
      {:else}
        <div class="preview-tree-virtual" style={`height: ${visibleRows.length * TREE_ROW_HEIGHT}px;`}>
          {#each treeWindowRows as row, index (`${row.kind}-${row.node.entry.path}-${treeStartIndex + index}`)}
            <div
              class="preview-tree-row"
              style={`top: ${(treeStartIndex + index) * TREE_ROW_HEIGHT}px;`}
            >
              {#if row.kind === "more"}
                <button
                  class="preview-tree-more"
                  disabled={row.node.loading}
                  style={`padding-left: ${10 + row.node.depth * 16}px;`}
                  onclick={() => row.node.entry.path ? loadDirectoryChildren(row.node, row.node.nextOffset ?? 0) : loadRootMore()}
                >
                  <span>{row.node.loading ? copy.previewLoading : copy.previewLoadMore}</span>
                </button>
              {:else if row.node.entry.kind === "directory"}
                <button
                  class="preview-tree-folder"
                  style={`padding-left: ${10 + row.node.depth * 16}px;`}
                  onclick={() => toggleDirectory(row.node)}
                >
                  <span class="preview-tree-disclosure">{row.node.expanded ? "▾" : "▸"}</span>
                  <span class="preview-tree-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24">
                      <path d="M3 6.5h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
                      <path d="M3 8.5h18" />
                    </svg>
                  </span>
                  <span>{row.node.entry.name}</span>
                </button>
              {:else}
                <button
                  class:active={selectedPath === row.node.entry.path}
                  class="preview-tree-file"
                  style={`padding-left: ${10 + row.node.depth * 16}px;`}
                  onclick={() => selectFile(row.node.entry)}
                >
                  <span class="preview-tree-disclosure" aria-hidden="true"></span>
                  <span class="preview-tree-icon" aria-hidden="true">
                    {#if isImageEntry(row.node.entry)}
                      <svg viewBox="0 0 24 24">
                        <path d="M4 5.5h16v13H4z" />
                        <path d="m7 15 3-3 3 3 2-2 3 3" />
                        <circle cx="9" cy="9" r="1.2" />
                      </svg>
                    {:else}
                      <svg viewBox="0 0 24 24">
                        <path d="M7 3.5h7l4 4v13H7z" />
                        <path d="M14 3.5v4h4" />
                      </svg>
                    {/if}
                  </span>
                  <span>{row.node.entry.name}</span>
                </button>
              {/if}
            </div>
          {/each}
        </div>
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
                <button type="button" aria-label={copy.previewZoomOut} title={copy.previewZoomOut} onpointerup={releasePointerFocus} onclick={() => previewZoomOut("before")}>
                  <Minus aria-hidden="true" />
                </button>
                <button type="button" aria-label={copy.previewZoomReset} title={copy.previewZoomReset} onpointerup={releasePointerFocus} onclick={resetPreviewPanzooms}>{previewPercent}</button>
                <button type="button" aria-label={copy.previewZoomIn} title={copy.previewZoomIn} onpointerup={releasePointerFocus} onclick={() => previewZoomIn("before")}>
                  <Plus aria-hidden="true" />
                </button>
                <button type="button" aria-label={copy.previewZoom} title={copy.previewZoom} onpointerup={releasePointerFocus} onclick={() => openZoom("before")}>
                  <Search aria-hidden="true" />
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
                <button type="button" aria-label={copy.previewZoomOut} title={copy.previewZoomOut} onpointerup={releasePointerFocus} onclick={() => previewZoomOut("after")}>
                  <Minus aria-hidden="true" />
                </button>
                <button type="button" aria-label={copy.previewZoomReset} title={copy.previewZoomReset} onpointerup={releasePointerFocus} onclick={resetPreviewPanzooms}>{previewPercent}</button>
                <button type="button" aria-label={copy.previewZoomIn} title={copy.previewZoomIn} onpointerup={releasePointerFocus} onclick={() => previewZoomIn("after")}>
                  <Plus aria-hidden="true" />
                </button>
                <button type="button" aria-label={copy.previewZoom} title={copy.previewZoom} onpointerup={releasePointerFocus} onclick={() => openZoom("after")}>
                  <Search aria-hidden="true" />
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
    tabindex="-1"
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
        <button type="button" aria-label={copy.previewZoomOut} title={copy.previewZoomOut} onclick={zoomOut}>
          <Minus aria-hidden="true" />
        </button>
        <button type="button" aria-label={copy.previewZoomReset} title={copy.previewZoomReset} onclick={resetZoomCanvas}>
          {zoomPercent}
        </button>
        <button type="button" aria-label={copy.previewZoomIn} title={copy.previewZoomIn} onclick={zoomIn}>
          <Plus aria-hidden="true" />
        </button>
        <button class="preview-zoom-close" type="button" aria-label={copy.closePreviewZoom} onclick={closeZoom}>
          <X aria-hidden="true" />
        </button>
      </div>
    </header>
    <div bind:this={zoomCanvas} class="preview-zoom-canvas" role="presentation" tabindex="-1">
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
