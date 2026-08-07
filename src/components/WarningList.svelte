<script lang="ts">
  import { localizeBackendMessage, type Copy } from "../lib/i18n.svelte";
  import type { WarningEntry } from "../lib/types";

  let { warnings, copy }: { warnings: WarningEntry[]; copy: Copy } = $props();
</script>

{#if warnings.length > 0}
  <section class="panel panel-card error-panel warning-panel">
    <div class="section-head error-head">
      <h2>{copy.warningsTitle}</h2>
      <span>{copy.warningsSubtitle(warnings.length)}</span>
    </div>
    <div class="failures">
      {#each warnings.slice(0, 500) as entry}
        <div class="failure">
          <strong>{entry.rel}</strong>
          <span>{localizeBackendMessage(entry.message, copy)}</span>
        </div>
      {/each}
    </div>
  </section>
{/if}
