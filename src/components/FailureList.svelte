<script lang="ts">
  import type { FailureEntry } from "../lib/types";

  let { failures }: { failures: FailureEntry[] } = $props();
</script>

{#if failures.length > 0}
  <section class="panel panel-card error-panel">
    <div class="section-head error-head">
      <h2>错误列表</h2>
      <span>{failures.length} 个文件处理失败</span>
    </div>
    <div class="failures">
      {#each failures.slice(0, 500) as entry}
        <div class="failure">
          <strong>{entry.rel}</strong>
          <span>{entry.message}</span>
        </div>
      {/each}
    </div>
  </section>
{/if}
