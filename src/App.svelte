<script lang="ts">
  import { onMount, tick } from "svelte";
  import SettingsForm from "./components/SettingsForm.svelte";
  import SummaryBar from "./components/SummaryBar.svelte";
  import ProgressStats from "./components/ProgressStats.svelte";
  import LogList from "./components/LogList.svelte";
  import FailureList from "./components/FailureList.svelte";
  import PreviewView from "./components/PreviewView.svelte";
  import { toNumber } from "./lib/format";
  import { getCopy, getCopyForLanguage, locale, localizeBackendMessage, outputFormatLabel, phaseTitle, setLanguage, supportedLanguages, syncDocumentLanguage } from "./lib/i18n.svelte";
  import type { Language } from "./lib/i18n.svelte";
  import { cancelBatch, loadSettings, onFailures, onProgress, saveSettings, startBatch } from "./lib/tauri";
  import type { BatchProgress, BatchSettings, FailureEntry } from "./lib/types";

  const DEFAULT_CONCURRENCY = navigator.hardwareConcurrency || 8;
  const DEFAULT_SETTINGS: BatchSettings = {
    inputSources: [],
    outputDir: "",
    resizeMode: "fitLongestSide",
    maxSide: 2000,
    width: 2000,
    height: 2000,
    allowUpscale: false,
    cropHorizontal: "center",
    cropVertical: "center",
    rotation: "auto",
    thumbnail: false,
    quality: 85,
    concurrency: DEFAULT_CONCURRENCY,
    outputFormat: "keep",
    copyNonImages: false,
    skipExisting: true,
  };

  let settings = $state<BatchSettings>({ ...DEFAULT_SETTINGS });

  let running = $state(false);
  let settingsLoaded = $state(false);
  const copy = $derived(getCopy());
  let statusTitle = $state("");
  let statusMessage = $state("");
  let currentFile = $state("");
  let progress = $state<BatchProgress | null>(null);
  let failures = $state<FailureEntry[]>([]);
  let logs = $state<string[]>([]);
  let lastLogSignature = "";

  let statusKind = $derived(
    running
      ? "running"
      : !progress
        ? "idle"
        : progress.phase === "error"
          ? "error"
          : progress.cancelled
            ? "cancelled"
            : progress.done
              ? "done"
              : "idle",
  );

  let previewOpen = $state(false);
  let view = $derived(running || progress ? "run" : previewOpen ? "preview" : "config");
  const startReady = $derived(settings.inputSources.length > 0 && Boolean(settings.outputDir));
  let starting = $state(false);
  let resetLocked = $state(false);
  let resetLockTimer: number | undefined;
  let configContentEl = $state<HTMLDivElement>();
  let scrollIndicatorVisible = $state(false);
  let scrollIndicatorNeeded = $state(false);
  let scrollThumbTop = $state(0);
  let scrollThumbHeight = $state(48);
  let scrollHideTimer: number | undefined;
  let languageMenuOpen = $state(false);

  onMount(() => {
    syncDocumentLanguage();
    const unlisteners: Array<() => void> = [];
    const handleResize = () => updateScrollIndicator();
    onProgress(handleProgress).then((unlisten) => unlisteners.push(unlisten));
    onFailures(handleFailures).then((unlisten) => unlisteners.push(unlisten));
    window.addEventListener("resize", handleResize);
    loadSettings()
      .then((saved) => {
        if (saved) settings = normalizeSettings(saved);
      })
      .catch((error) => addLog(copy.readSettingsFailed(String(error))))
      .finally(() => {
        settingsLoaded = true;
      });
    return () => {
      for (const unlisten of unlisteners) unlisten();
      window.removeEventListener("resize", handleResize);
      if (scrollHideTimer) window.clearTimeout(scrollHideTimer);
    };
  });

  $effect(() => {
    const snapshot = JSON.stringify(settings);
    if (!settingsLoaded) return;
    const timer = window.setTimeout(() => {
      saveSettings(JSON.parse(snapshot)).catch((error) => addLog(copy.saveSettingsFailed(String(error))));
    }, 350);
    return () => window.clearTimeout(timer);
  });

  $effect(() => {
    copy;
    if (!progress) {
      statusTitle = copy.waitTitle;
      statusMessage = copy.waitMessage;
      return;
    }
    statusTitle = phaseTitle(progress, copy);
    statusMessage = progress.message ? localizeBackendMessage(progress.message, copy) : copy.processingImages;
  });

  $effect(() => {
    view;
    settings.inputSources.length;
    showScrollIndicator(false);
    tick().then(() => updateScrollIndicator());
  });

  function normalizeSettings(saved: BatchSettings): BatchSettings {
    const savedResizeMode = saved.resizeMode as string;
    const resizeMode = saved.thumbnail || savedResizeMode === "fillCrop" ? "fixedCrop" : saved.resizeMode;
    return {
      ...DEFAULT_SETTINGS,
      ...saved,
      inputSources: Array.isArray(saved.inputSources) ? saved.inputSources : [],
      resizeMode,
      thumbnail: false,
      concurrency: toNumber(saved.concurrency, DEFAULT_CONCURRENCY),
      maxSide: toNumber(saved.maxSide, DEFAULT_SETTINGS.maxSide),
      width: toNumber(saved.width, DEFAULT_SETTINGS.width),
      height: toNumber(saved.height, DEFAULT_SETTINGS.height),
      quality: toNumber(saved.quality, DEFAULT_SETTINGS.quality),
    };
  }

  function addLog(message: string) {
    const time = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    logs = [...logs, `[${time}] ${message}`].slice(-100);
  }

  function handleFailures(entries: FailureEntry[]) {
    failures = entries;
    if (entries.length > 0) {
      addLog(copy.failureListUpdated(entries.length));
    }
  }

  function handleProgress(next: BatchProgress) {
    progress = next;
    statusTitle = phaseTitle(next, copy);
    statusMessage = next.message ? localizeBackendMessage(next.message, copy) : copy.processingImages;
    currentFile = next.current ?? "";
    addProgressLog(next);
    if (next.done || next.phase === "error") {
      running = false;
    }
  }

  function addProgressLog(next: BatchProgress) {
    const label = phaseTitle(next, copy);
    const message = next.current ?? (next.message ? localizeBackendMessage(next.message, copy) : "");
    const signature = `${next.phase}|${next.processed}|${next.discovered}|${message}|${next.done}|${next.cancelled}`;
    if (signature === lastLogSignature) return;

    lastLogSignature = signature;
    if (next.done) {
      addLog(next.cancelled ? copy.processingStopped : copy.processingDone);
      return;
    }
    if (next.phase === "scanning") {
      addLog(next.message ? localizeBackendMessage(next.message, copy) : copy.scanningInput);
      return;
    }
    if (next.current) {
      addLog(`${label}: ${next.current}`);
      return;
    }
    if (next.message) {
      addLog(localizeBackendMessage(next.message, copy));
    }
  }

  async function handleStart() {
    if (starting || running) return;
    if (settings.inputSources.length === 0 || !settings.outputDir) {
      statusTitle = copy.sourceMissingTitle;
      statusMessage = copy.sourceMissingMessage;
      addLog(copy.sourceMissingMessage);
      return;
    }

    starting = true;
    running = true;
    previewOpen = false;
    resetLocked = true;
    if (resetLockTimer) window.clearTimeout(resetLockTimer);
    resetLockTimer = window.setTimeout(() => {
      resetLocked = false;
      resetLockTimer = undefined;
    }, 1000);
    logs = [];
    lastLogSignature = "";
    failures = [];
    progress = null;

    const payload: BatchSettings = {
      ...settings,
      maxSide: toNumber(settings.maxSide, 2000),
      width: toNumber(settings.width, 2000),
      height: toNumber(settings.height, 2000),
      quality: toNumber(settings.quality, 85),
      concurrency: toNumber(settings.concurrency, DEFAULT_CONCURRENCY),
    };

    addLog(
      copy.startLog(payload.inputSources.length, payload.quality, payload.concurrency, outputFormatLabel(payload.outputFormat, copy)),
    );

    try {
      await startBatch(payload);
    } catch (error) {
      running = false;
      statusTitle = copy.startFailedTitle;
      const message = localizeBackendMessage(String(error), copy);
      statusMessage = message;
      addLog(copy.startFailedLog(message));
    } finally {
      starting = false;
    }
  }

  async function handleStop() {
    statusMessage = copy.stoppingMessage;
    addLog(copy.stoppingMessage);
    await cancelBatch();
  }

  function chooseLanguage(language: Language) {
    setLanguage(language);
    languageMenuOpen = false;
  }

  function closeLanguageMenuOnOutsideClick(event: MouseEvent) {
    if (!languageMenuOpen) return;
    const target = event.target;
    if (target instanceof Element && target.closest(".language-menu-wrap")) return;
    languageMenuOpen = false;
  }

  function handleReset() {
    previewOpen = false;
    progress = null;
    failures = [];
    logs = [];
    lastLogSignature = "";
    statusTitle = copy.waitTitle;
    statusMessage = copy.waitMessage;
    currentFile = "";
  }

  function handlePreview() {
    if (!startReady || starting || running) return;
    previewOpen = true;
  }

  function handleBackToConfig() {
    previewOpen = false;
  }

  function updateScrollIndicator() {
    if (!configContentEl || view !== "config") {
      scrollIndicatorNeeded = false;
      return;
    }

    const { clientHeight, scrollHeight, scrollTop } = configContentEl;
    scrollIndicatorNeeded = scrollHeight > clientHeight + 1;
    if (!scrollIndicatorNeeded) {
      scrollIndicatorVisible = false;
      return;
    }

    scrollThumbHeight = Math.max(48, (clientHeight / scrollHeight) * clientHeight);
    scrollThumbTop = ((clientHeight - scrollThumbHeight) * scrollTop) / (scrollHeight - clientHeight);
  }

  function showScrollIndicator(autoHide = true) {
    updateScrollIndicator();
    if (!scrollIndicatorNeeded) return;
    scrollIndicatorVisible = true;
    if (scrollHideTimer) window.clearTimeout(scrollHideTimer);
    if (autoHide) {
      scrollHideTimer = window.setTimeout(() => {
        scrollIndicatorVisible = false;
        scrollHideTimer = undefined;
      }, 900);
    }
  }
</script>

<svelte:window onclick={closeLanguageMenuOnOutsideClick} onkeydown={(event) => {
  if (event.key === "Escape") languageMenuOpen = false;
}} />

<main class="shell">
  {#if view === "config"}
    <div class="content-scroll-frame" role="presentation" onpointerenter={() => showScrollIndicator(false)} onpointerleave={() => (scrollIndicatorVisible = false)}>
      <div bind:this={configContentEl} class="content content-config" onscroll={() => showScrollIndicator()}>
        <SettingsForm bind:settings {copy} />
      </div>
      {#if scrollIndicatorNeeded}
        <div class:visible={scrollIndicatorVisible} class="scroll-indicator" aria-hidden="true">
          <span style={`height: ${scrollThumbHeight}px; transform: translateY(${scrollThumbTop}px);`}></span>
        </div>
      {/if}
    </div>
  {:else if view === "preview"}
    <div class="content content-preview">
      <PreviewView {settings} {copy} />
    </div>
  {:else}
    <div class="content content-run">
      <SummaryBar {settings} {copy} />
      <ProgressStats {statusTitle} {statusMessage} {currentFile} {progress} {statusKind} {copy} />
      <LogList {logs} {copy} />
      <FailureList {failures} {copy} />
    </div>
  {/if}

  <footer class="bottombar">
    <div class="bottombar-inner">
      {#if view === "config"}
        <button class="primary bottombar-action bottombar-action-primary" onclick={handleStart} disabled={!startReady || starting}>
          {starting ? copy.startingButton : copy.startButton}
        </button>
        <button class="secondary bottombar-action" onclick={handlePreview} disabled={!startReady || starting}>
          {copy.previewButton}
        </button>
      {/if}
      {#if view === "preview"}
        <button class="secondary bottombar-action" onclick={handleBackToConfig}>
          {copy.backButton}
        </button>
      {/if}
      {#if view === "run" && !running}
        <button class="secondary bottombar-action" onclick={handleReset} disabled={resetLocked}>
          {copy.backButton}
        </button>
      {/if}
      {#if view === "run" && running}
        <button class="secondary danger bottombar-action" onclick={handleStop} disabled={resetLocked}>
          {copy.stopButton}
        </button>
      {/if}
      <div class="bottombar-spacer"></div>
      <div class="language-menu-wrap">
        <button
          class:open={languageMenuOpen}
          class="language-toggle"
          onclick={() => (languageMenuOpen = !languageMenuOpen)}
          aria-haspopup="menu"
          aria-expanded={languageMenuOpen}
          aria-label={copy.languageName}
          title={copy.languageName}
        >
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <path d="M2 12h20" />
            <path d="M12 2a15.3 15.3 0 0 1 0 20" />
            <path d="M12 2a15.3 15.3 0 0 0 0 20" />
          </svg>
          <span>{copy.languageName}</span>
        </button>
        {#if languageMenuOpen}
          <div class="language-menu" role="menu">
            {#each supportedLanguages as language}
              <button
                class:active={locale.language === language}
                role="menuitemradio"
                aria-checked={locale.language === language}
                onclick={() => chooseLanguage(language)}
              >
                <span>{getCopyForLanguage(language).languageName}</span>
                <span class="language-check">✓</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </footer>
</main>
