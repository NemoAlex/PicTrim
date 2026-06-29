<script lang="ts">
  import { onMount } from "svelte";
  import SettingsForm from "./components/SettingsForm.svelte";
  import SummaryBar from "./components/SummaryBar.svelte";
  import ProgressStats from "./components/ProgressStats.svelte";
  import LogList from "./components/LogList.svelte";
  import FailureList from "./components/FailureList.svelte";
  import { formatLabel, phaseTitle, toNumber } from "./lib/format";
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
  let statusTitle = $state("等待开始");
  let statusMessage = $state("等待开始。");
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

  let view = $derived(running || progress ? "run" : "config");
  const startReady = $derived(settings.inputSources.length > 0 && Boolean(settings.outputDir));
  let starting = $state(false);
  let resetLocked = $state(false);
  let resetLockTimer: number | undefined;

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    onProgress(handleProgress).then((unlisten) => unlisteners.push(unlisten));
    onFailures(handleFailures).then((unlisten) => unlisteners.push(unlisten));
    loadSettings()
      .then((saved) => {
        if (saved) settings = normalizeSettings(saved);
      })
      .catch((error) => addLog(`读取设置失败: ${String(error)}`))
      .finally(() => {
        settingsLoaded = true;
      });
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  });

  $effect(() => {
    const snapshot = JSON.stringify(settings);
    if (!settingsLoaded) return;
    const timer = window.setTimeout(() => {
      saveSettings(JSON.parse(snapshot)).catch((error) => addLog(`保存设置失败: ${String(error)}`));
    }, 350);
    return () => window.clearTimeout(timer);
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
      addLog(`发现 ${entries.length} 个错误，已更新错误列表。`);
    }
  }

  function handleProgress(next: BatchProgress) {
    progress = next;
    statusTitle = phaseTitle(next);
    statusMessage = next.message ?? "正在处理图片。";
    currentFile = next.current ?? "";
    addProgressLog(next);
    if (next.done || next.phase === "error") {
      running = false;
    }
  }

  function addProgressLog(next: BatchProgress) {
    const label = phaseTitle(next);
    const message = next.current ?? next.message ?? "";
    const signature = `${next.phase}|${next.processed}|${next.discovered}|${message}|${next.done}|${next.cancelled}`;
    if (signature === lastLogSignature) return;

    lastLogSignature = signature;
    if (next.done) {
      addLog(next.cancelled ? "处理已停止。" : "处理完成。");
      return;
    }
    if (next.phase === "scanning") {
      addLog(next.message ?? "正在扫描输入目录。");
      return;
    }
    if (next.current) {
      addLog(`${label}: ${next.current}`);
      return;
    }
    if (next.message) {
      addLog(next.message);
    }
  }

  async function handleStart() {
    if (starting || running) return;
    if (settings.inputSources.length === 0 || !settings.outputDir) {
      statusTitle = "来源未选择";
      statusMessage = "请先选择输入来源和输出目录。";
      addLog("请先选择输入来源和输出目录。");
      return;
    }

    starting = true;
    running = true;
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
      `开始处理: 来源 ${payload.inputSources.length} 个, 质量 ${payload.quality}%, 并发 ${payload.concurrency}, 格式 ${formatLabel(payload.outputFormat)}。`,
    );

    try {
      await startBatch(payload);
    } catch (error) {
      running = false;
      statusTitle = "无法开始";
      statusMessage = String(error);
      addLog(`无法开始: ${String(error)}`);
    } finally {
      starting = false;
    }
  }

  async function handleStop() {
    statusMessage = "正在停止，已开始的图片会先完成写入。";
    addLog("正在停止，已开始的图片会先完成写入。");
    await cancelBatch();
  }

  function handleReset() {
    progress = null;
    failures = [];
    logs = [];
    lastLogSignature = "";
    statusTitle = "等待开始";
    statusMessage = "等待开始。";
    currentFile = "";
  }
</script>

<main class="shell">
  {#if view === "config"}
    <div class="content content-config">
      <SettingsForm bind:settings />
    </div>
  {:else}
    <div class="content content-run">
      <SummaryBar {settings} />
      <ProgressStats {statusTitle} {statusMessage} {currentFile} {progress} {statusKind} />
      <LogList {logs} />
      <FailureList {failures} />
    </div>
  {/if}

  <footer class="bottombar">
    <div class="bottombar-inner">
      {#if view === "config"}
        <button class="primary bottombar-action bottombar-action-primary" onclick={handleStart} disabled={!startReady || starting}>
          {starting ? "正在开始…" : "开始处理"}
        </button>
      {/if}
      {#if view === "run" && !running}
        <button class="secondary bottombar-action" onclick={handleReset} disabled={resetLocked}>
          返回
        </button>
      {/if}
      {#if view === "run" && running}
        <button class="secondary danger bottombar-action" onclick={handleStop} disabled={resetLocked}>
          停止
        </button>
      {/if}
      <div class="bottombar-spacer"></div>
    </div>
  </footer>
</main>
