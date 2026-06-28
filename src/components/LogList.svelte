<script lang="ts">
  let { logs }: { logs: string[] } = $props();

  let container = $state<HTMLDivElement | null>(null);

  $effect(() => {
    // Re-run when the log list grows, then stick to the bottom.
    logs.length;
    if (container) {
      container.scrollTop = container.scrollHeight;
    }
  });
</script>

<section class="panel log-panel">
  <div class="section-head">
    <h2>输出日志</h2>
  </div>
  <div class="logs" bind:this={container}>
    {#if logs.length === 0}
      <div class="empty">暂无日志。</div>
    {:else}
      {#each logs as line}
        <div class="log-line">{line}</div>
      {/each}
    {/if}
  </div>
</section>
