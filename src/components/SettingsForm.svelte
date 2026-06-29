<script lang="ts">
  import { pickDirectory } from "../lib/tauri";
  import type { BatchSettings } from "../lib/types";

  let {
    settings = $bindable(),
    onstart,
  }: {
    settings: BatchSettings;
    onstart: () => void;
  } = $props();

  const ready = $derived(Boolean(settings.inputDir && settings.outputDir));

  async function pickInput() {
    const dir = await pickDirectory();
    if (dir) settings.inputDir = dir;
  }

  async function pickOutput() {
    const dir = await pickDirectory();
    if (dir) settings.outputDir = dir;
  }
</script>

<section class="panel panel-card setup-panel">
  <div class="setup-head">
    <h2>批量处理图片</h2>
    <p>选择输入与输出目录，设置参数后开始处理。</p>
  </div>

  <div class="paths">
    <label>
      <span>输入目录</span>
      <div class="path-row">
        <input readonly placeholder="选择包含图片的文件夹" value={settings.inputDir} />
        <button class="secondary" onclick={pickInput}>选择</button>
      </div>
    </label>
    <label>
      <span>输出目录</span>
      <div class="path-row">
        <input readonly placeholder="选择保存结果的文件夹" value={settings.outputDir} />
        <button class="secondary" onclick={pickOutput}>选择</button>
      </div>
    </label>
  </div>

  <div class="field-group">
    <h3 class="group-title">图片参数</h3>
    <div class="settings settings-3">
      <label>
        <span>最长边 (px)</span>
        <input type="number" min="1" max="50000" bind:value={settings.maxSide} />
      </label>
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

  <div class="field-group">
    <h3 class="group-title">批处理</h3>
    <div class="settings settings-3">
      <label>
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

  <button class="primary block" onclick={onstart} disabled={!ready}>
    {ready ? "开始处理" : "请选择输入与输出目录"}
  </button>
</section>
