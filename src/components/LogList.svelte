<script lang="ts">
  import type { Copy } from "../lib/i18n.svelte";

  let { logs, copy }: { logs: string[]; copy: Copy } = $props();

  let container = $state<HTMLDivElement | null>(null);

  $effect(() => {
    // Re-run when the log list grows, then stick to the bottom.
    logs.length;
    if (container) {
      container.scrollTop = container.scrollHeight;
    }
  });
</script>

<section class="panel panel-card log-panel">
  <div class="section-head">
    <h2>{copy.logsTitle}</h2>
  </div>
  <div class="logs" bind:this={container}>
    {#if logs.length === 0}
      <div class="empty">{copy.emptyLogs}</div>
    {:else}
      {#each logs as line}
        <div class="log-line">{line}</div>
      {/each}
    {/if}
  </div>
</section>
