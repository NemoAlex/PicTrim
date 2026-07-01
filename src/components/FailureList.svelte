<script lang="ts">
  import { localizeBackendMessage, type Copy } from "../lib/i18n.svelte";
  import type { FailureEntry } from "../lib/types";

  let { failures, copy }: { failures: FailureEntry[]; copy: Copy } = $props();
</script>

{#if failures.length > 0}
  <section class="panel panel-card error-panel">
    <div class="section-head error-head">
      <h2>{copy.failuresTitle}</h2>
      <span>{copy.failuresSubtitle(failures.length)}</span>
    </div>
    <div class="failures">
      {#each failures.slice(0, 500) as entry}
        <div class="failure">
          <strong>{entry.rel}</strong>
          <span>{localizeBackendMessage(entry.message, copy)}</span>
        </div>
      {/each}
    </div>
  </section>
{/if}
