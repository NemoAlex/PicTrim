<script lang="ts">
  import { onMount } from "svelte";
  import SettingsForm from "./components/SettingsForm.svelte";
  import ProgressStats from "./components/ProgressStats.svelte";
  import LogList from "./components/LogList.svelte";
  import FailureList from "./components/FailureList.svelte";
  import { formatLabel, phaseTitle, toNumber } from "./lib/format";
  import { cancelBatch, onFailures, onProgress, startBatch } from "./lib/tauri";
  import type { BatchProgress, BatchSettings, FailureEntry } from "./lib/types";

  let settings = $state<BatchSettings>({
    inputDir: "",
    outputDir: "",
    maxSide: 2000,
    quality: 85,
    concurrency: 20,
    outputFormat: "jpg",
    copyNonImages: false,
    skipExisting: true,
  });

  let running = $state(false);
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

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    onProgress(handleProgress).then((unlisten) => unlisteners.push(unlisten));
    onFailures(handleFailures).then((unlisten) => unlisteners.push(unlisten));
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  });

  function addLog(message: string) {
    const time = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    logs = [...logs, `[${time}] ${message}`].slice(-400);
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
    currentFile = next.current ? `当前文件: ${next.current}` : "";
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
    if (!settings.inputDir || !settings.outputDir) {
      statusTitle = "目录未选择";
      statusMessage = "请先选择输入目录和输出目录。";
      addLog("请先选择输入目录和输出目录。");
      return;
    }

    running = true;
    logs = [];
    lastLogSignature = "";
    failures = [];
    progress = null;

    const payload: BatchSettings = {
      ...settings,
      maxSide: toNumber(settings.maxSide, 2000),
      quality: toNumber(settings.quality, 85),
      concurrency: toNumber(settings.concurrency, 20),
    };

    addLog(
      `开始处理: 最长边 ${payload.maxSide} px, 质量 ${payload.quality}%, 并发 ${payload.concurrency}, 格式 ${formatLabel(payload.outputFormat)}。`,
    );

    try {
      await startBatch(payload);
    } catch (error) {
      running = false;
      statusTitle = "无法开始";
      statusMessage = String(error);
      addLog(`无法开始: ${String(error)}`);
    }
  }

  async function handleStop() {
    statusMessage = "正在停止，已开始的图片会先完成写入。";
    addLog("正在停止，已开始的图片会先完成写入。");
    await cancelBatch();
  }
</script>

<main class="shell">
  <header class="titlebar">
    <div class="brand">
      <div>
        <h1>PicTrim</h1>
        <p>批量缩放与格式转换</p>
      </div>
    </div>
    <div class="status-pill">
      <span class="status-dot {statusKind}"></span>
      <span>{statusTitle}</span>
    </div>
  </header>

  <SettingsForm bind:settings {running} onstart={handleStart} onstop={handleStop} />
  <ProgressStats {statusMessage} {currentFile} {progress} />
  <LogList {logs} />
  <FailureList {failures} />
</main>
