<script lang="ts">
  import { formatBytes } from "../lib/format";
  import type { BatchProgress } from "../lib/types";

  let {
    statusMessage,
    currentFile,
    progress,
  }: {
    statusMessage: string;
    currentFile: string;
    progress: BatchProgress | null;
  } = $props();

  const detail = $derived(currentFile || statusMessage);
</script>

<section class="panel progress-panel">
  <div class="section-head">
    <h2>处理</h2>
  </div>
  <div class="current-file">{detail}</div>
  <div class="stats">
    <div><span>发现</span><strong>{progress?.discovered ?? 0}</strong></div>
    <div><span>已处理</span><strong>{progress?.processed ?? 0}</strong></div>
    <div><span>图片</span><strong>{progress?.images ?? 0}</strong></div>
    <div><span>跳过</span><strong>{progress?.skipped ?? 0}</strong></div>
    <div><span>失败</span><strong>{progress?.failed ?? 0}</strong></div>
    <div>
      <span>体积</span>
      <strong>{formatBytes(progress?.totalSrcBytes ?? 0)} -&gt; {formatBytes(progress?.totalDstBytes ?? 0)}</strong>
    </div>
  </div>
</section>
