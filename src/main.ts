import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type OutputFormat = "jpg" | "png" | "webp" | "keep";

interface BatchProgress {
  phase: string;
  discovered: number;
  processed: number;
  images: number;
  copied: number;
  skipped: number;
  failed: number;
  totalSrcBytes: number;
  totalDstBytes: number;
  current?: string | null;
  message?: string | null;
  done: boolean;
  cancelled: boolean;
}

interface FailureEntry {
  rel: string;
  message: string;
}

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("Missing app root");
}

app.innerHTML = `
  <main class="shell">
    <header class="titlebar">
      <div class="brand">
        <div>
          <h1>PicTrim</h1>
          <p>批量缩放与格式转换</p>
        </div>
      </div>
      <div class="status-pill">
        <span id="statusTitle">等待开始</span>
        <strong id="progressPct">0%</strong>
      </div>
    </header>

    <section class="panel setup-panel">
      <div class="section-head">
        <h2>参数</h2>
        <p>先选择目录和输出设置，再开始处理。</p>
      </div>

      <div class="paths">
        <label>
          <span>输入目录</span>
          <div class="path-row">
            <input id="inputDir" readonly placeholder="选择包含图片的文件夹" />
            <button id="pickInput" class="secondary">选择</button>
          </div>
        </label>
        <label>
          <span>输出目录</span>
          <div class="path-row">
            <input id="outputDir" readonly placeholder="选择保存结果的文件夹" />
            <button id="pickOutput" class="secondary">选择</button>
          </div>
        </label>
      </div>

      <div class="settings">
        <label>
          <span>最长边</span>
          <input id="maxSide" type="number" min="1" max="50000" value="2000" />
        </label>
        <label>
          <span>质量</span>
          <input id="quality" type="number" min="1" max="100" value="85" />
        </label>
        <label>
          <span>并发数</span>
          <input id="concurrency" type="number" min="1" max="128" value="20" />
        </label>
        <label>
          <span>输出格式</span>
          <select id="outputFormat">
            <option value="jpg">JPG</option>
            <option value="png">PNG</option>
            <option value="webp">WebP</option>
            <option value="keep">保持原格式</option>
          </select>
        </label>
        <label class="toggle">
          <input id="copyNonImages" type="checkbox" />
          <span>复制非图片文件</span>
        </label>
        <label class="toggle">
          <input id="skipExisting" type="checkbox" checked />
          <span>跳过已存在文件</span>
        </label>
      </div>

      <div class="actions">
        <button id="startBtn" class="primary">开始处理</button>
        <button id="stopBtn" class="secondary" disabled>停止</button>
      </div>
    </section>

    <section class="panel progress-panel">
      <div class="section-head">
        <h2>处理</h2>
        <p id="statusMessage">等待开始。</p>
      </div>
      <div class="bar"><div id="barFill"></div></div>
      <div id="currentFile" class="current-file">暂无正在处理的文件</div>
      <div class="stats">
        <div><span>发现</span><strong id="statDiscovered">0</strong></div>
        <div><span>已处理</span><strong id="statProcessed">0</strong></div>
        <div><span>图片</span><strong id="statImages">0</strong></div>
        <div><span>跳过</span><strong id="statSkipped">0</strong></div>
        <div><span>失败</span><strong id="statFailed">0</strong></div>
        <div><span>体积</span><strong id="statBytes">0 B</strong></div>
      </div>
    </section>

    <section class="panel log-panel">
      <div class="section-head">
        <h2>输出日志</h2>
        <p>处理过程会显示在这里。</p>
      </div>
      <div id="logs" class="logs">
        <div class="empty">暂无日志。</div>
      </div>
    </section>

    <section id="errorPanel" class="panel error-panel" hidden>
      <div class="section-head error-head">
        <div>
          <h2>错误列表</h2>
          <p id="errorSummary">没有错误。</p>
        </div>
        <span>最多显示 500 条</span>
      </div>
      <div id="failures" class="failures"></div>
    </section>
  </main>
`;

const inputDir = getInput("inputDir");
const outputDir = getInput("outputDir");
const maxSide = getInput("maxSide");
const quality = getInput("quality");
const concurrency = getInput("concurrency");
const outputFormat = getSelect("outputFormat");
const copyNonImages = getInput("copyNonImages");
const skipExisting = getInput("skipExisting");
const startBtn = getButton("startBtn");
const stopBtn = getButton("stopBtn");
const statusTitle = getText("statusTitle");
const statusMessage = getText("statusMessage");
const progressPct = getText("progressPct");
const barFill = getText("barFill");
const currentFile = getText("currentFile");
const failures = getText("failures");
const logs = getText("logs");
const errorPanel = getText("errorPanel");
const errorSummary = getText("errorSummary");
const logEntries: string[] = [];
let lastLogSignature = "";

document.querySelector("#pickInput")?.addEventListener("click", () => pickDirectory(inputDir));
document.querySelector("#pickOutput")?.addEventListener("click", () => pickDirectory(outputDir));
startBtn.addEventListener("click", startBatch);
stopBtn.addEventListener("click", stopBatch);

void listen<BatchProgress>("batch-progress", (event) => {
  renderProgress(event.payload);
});

void listen<FailureEntry[]>("batch-failures", (event) => {
  renderFailures(event.payload);
});

async function pickDirectory(target: HTMLInputElement) {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择目录",
  });
  if (typeof selected === "string") {
    target.value = selected;
  }
}

async function startBatch() {
  if (!inputDir.value || !outputDir.value) {
    statusTitle.textContent = "目录未选择";
    statusMessage.textContent = "请先选择输入目录和输出目录。";
    addLog("请先选择输入目录和输出目录。");
    return;
  }

  setRunning(true);
  resetLogs();
  renderFailures([]);
  addLog(
    `开始处理: 最长边 ${toNumber(maxSide.value, 2000)} px, 质量 ${toNumber(quality.value, 85)}%, 并发 ${toNumber(
      concurrency.value,
      20,
    )}, 格式 ${formatLabel(outputFormat.value as OutputFormat)}。`,
  );
  try {
    await invoke("start_batch", {
      settings: {
        inputDir: inputDir.value,
        outputDir: outputDir.value,
        maxSide: toNumber(maxSide.value, 2000),
        quality: toNumber(quality.value, 85),
        concurrency: toNumber(concurrency.value, 20),
        outputFormat: outputFormat.value as OutputFormat,
        copyNonImages: copyNonImages.checked,
        skipExisting: skipExisting.checked,
      },
    });
  } catch (error) {
    setRunning(false);
    statusTitle.textContent = "无法开始";
    statusMessage.textContent = String(error);
    addLog(`无法开始: ${String(error)}`);
  }
}

async function stopBatch() {
  stopBtn.disabled = true;
  statusMessage.textContent = "正在停止，已开始的图片会先完成写入。";
  addLog("正在停止，已开始的图片会先完成写入。");
  await invoke("cancel_batch");
}

function renderProgress(progress: BatchProgress) {
  const pct =
    progress.discovered > 0
      ? Math.min(100, Math.round((progress.processed / progress.discovered) * 100))
      : 0;

  statusTitle.textContent = phaseTitle(progress);
  statusMessage.textContent = progress.message ?? "正在处理图片。";
  progressPct.textContent = `${pct}%`;
  barFill.style.width = `${pct}%`;
  currentFile.textContent = progress.current ? `当前文件: ${progress.current}` : "暂无正在处理的文件";

  setText("statDiscovered", progress.discovered);
  setText("statProcessed", progress.processed);
  setText("statImages", progress.images);
  setText("statSkipped", progress.skipped);
  setText("statFailed", progress.failed);
  setText("statBytes", `${formatBytes(progress.totalSrcBytes)} -> ${formatBytes(progress.totalDstBytes)}`);
  addProgressLog(progress);

  if (progress.done || progress.phase === "error") {
    setRunning(false);
  }
}

function renderFailures(entries: FailureEntry[]) {
  if (entries.length === 0) {
    errorPanel.hidden = true;
    failures.innerHTML = "";
    errorSummary.textContent = "没有错误。";
    return;
  }

  errorPanel.hidden = false;
  errorSummary.textContent = `${entries.length} 个文件处理失败。`;
  addLog(`发现 ${entries.length} 个错误，已更新错误列表。`);
  failures.innerHTML = entries
    .slice(0, 500)
    .map(
      (entry) => `
        <div class="failure">
          <strong>${escapeHtml(entry.rel)}</strong>
          <span>${escapeHtml(entry.message)}</span>
        </div>
      `,
    )
    .join("");
}

function addProgressLog(progress: BatchProgress) {
  const label = phaseTitle(progress);
  const message = progress.current ?? progress.message ?? "";
  const signature = `${progress.phase}|${progress.processed}|${progress.discovered}|${message}|${progress.done}|${progress.cancelled}`;
  if (signature === lastLogSignature) return;

  lastLogSignature = signature;
  if (progress.done) {
    addLog(progress.cancelled ? "处理已停止。" : "处理完成。");
    return;
  }
  if (progress.phase === "scanning") {
    addLog(progress.message ?? "正在扫描输入目录。");
    return;
  }
  if (progress.current) {
    addLog(`${label}: ${progress.current}`);
    return;
  }
  if (progress.message) {
    addLog(progress.message);
  }
}

function resetLogs() {
  logEntries.length = 0;
  lastLogSignature = "";
  logs.innerHTML = `<div class="empty">暂无日志。</div>`;
}

function addLog(message: string) {
  const timestamp = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  logEntries.push(`[${timestamp}] ${message}`);
  if (logEntries.length > 400) {
    logEntries.shift();
  }
  logs.innerHTML = logEntries.map((entry) => `<div class="log-line">${escapeHtml(entry)}</div>`).join("");
  logs.scrollTop = logs.scrollHeight;
}

function phaseTitle(progress: BatchProgress) {
  if (progress.phase === "scanning") return "正在扫描";
  if (progress.phase === "processing") return "正在处理";
  if (progress.phase === "error") return "处理出错";
  if (progress.cancelled) return "已停止";
  if (progress.done) return "处理完成";
  return "等待开始";
}

function formatLabel(value: OutputFormat) {
  if (value === "jpg") return "JPG";
  if (value === "png") return "PNG";
  if (value === "webp") return "WebP";
  return "保持原格式";
}

function setRunning(running: boolean) {
  startBtn.disabled = running;
  stopBtn.disabled = !running;
  for (const element of document.querySelectorAll<HTMLInputElement | HTMLSelectElement>(
    "input, select, #pickInput, #pickOutput",
  )) {
    element.disabled = running;
  }
}

function toNumber(value: string, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function formatBytes(bytes: number) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 2)} ${units[unit]}`;
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (char) => {
    const map: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#039;",
    };
    return map[char];
  });
}

function setText(id: string, value: string | number) {
  getText(id).textContent = String(value);
}

function getInput(id: string) {
  const element = document.getElementById(id);
  if (!(element instanceof HTMLInputElement)) throw new Error(`Missing input #${id}`);
  return element;
}

function getSelect(id: string) {
  const element = document.getElementById(id);
  if (!(element instanceof HTMLSelectElement)) throw new Error(`Missing select #${id}`);
  return element;
}

function getButton(id: string) {
  const element = document.getElementById(id);
  if (!(element instanceof HTMLButtonElement)) throw new Error(`Missing button #${id}`);
  return element;
}

function getText(id: string) {
  const element = document.getElementById(id);
  if (!(element instanceof HTMLElement)) throw new Error(`Missing element #${id}`);
  return element;
}
