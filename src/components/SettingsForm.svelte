<script lang="ts">
  import { pickDirectory } from "../lib/tauri";
  import type { BatchSettings } from "../lib/types";

  let {
    settings = $bindable(),
    running,
    onstart,
    onstop,
  }: {
    settings: BatchSettings;
    running: boolean;
    onstart: () => void;
    onstop: () => void;
  } = $props();

  async function pickInput() {
    const dir = await pickDirectory();
    if (dir) settings.inputDir = dir;
  }

  async function pickOutput() {
    const dir = await pickDirectory();
    if (dir) settings.outputDir = dir;
  }
</script>

<section class="panel setup-panel">
  <div class="section-head">
    <h2>参数</h2>
  </div>

  <div class="paths">
    <label>
      <span>输入目录</span>
      <div class="path-row">
        <input readonly placeholder="选择包含图片的文件夹" value={settings.inputDir} disabled={running} />
        <button class="secondary" onclick={pickInput} disabled={running}>选择</button>
      </div>
    </label>
    <label>
      <span>输出目录</span>
      <div class="path-row">
        <input readonly placeholder="选择保存结果的文件夹" value={settings.outputDir} disabled={running} />
        <button class="secondary" onclick={pickOutput} disabled={running}>选择</button>
      </div>
    </label>
  </div>

  <div class="settings">
    <label>
      <span>最长边</span>
      <input type="number" min="1" max="50000" bind:value={settings.maxSide} disabled={running} />
    </label>
    <label>
      <span>质量</span>
      <input type="number" min="1" max="100" bind:value={settings.quality} disabled={running} />
    </label>
    <label>
      <span>并发数</span>
      <input type="number" min="1" max="128" bind:value={settings.concurrency} disabled={running} />
    </label>
    <label>
      <span>输出格式</span>
      <select bind:value={settings.outputFormat} disabled={running}>
        <option value="jpg">JPG</option>
        <option value="png">PNG</option>
        <option value="webp">WebP</option>
        <option value="keep">保持原格式</option>
      </select>
    </label>
    <label class="toggle">
      <input type="checkbox" bind:checked={settings.copyNonImages} disabled={running} />
      <span>复制非图片文件</span>
    </label>
    <label class="toggle">
      <input type="checkbox" bind:checked={settings.skipExisting} disabled={running} />
      <span>跳过已存在文件</span>
    </label>
  </div>

  <div class="actions">
    <button class="primary" onclick={onstart} disabled={running}>开始处理</button>
    <button class="secondary" onclick={onstop} disabled={!running}>停止</button>
  </div>
</section>
