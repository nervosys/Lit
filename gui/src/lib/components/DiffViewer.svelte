<script lang="ts">
  import type { FileDiff } from "../types";

  let { file }: { file: FileDiff | null } = $props();

  function lineNumber(n: number) {
    return String(n).padStart(4, " ");
  }
</script>

<div class="diff-viewer">
  {#if file}
    <div class="diff-header">
      <span class="diff-path mono">{file.path}</span>
      <span class="diff-stats">
        {#if file.additions > 0}
          <span class="text-safe">+{file.additions}</span>
        {/if}
        {#if file.deletions > 0}
          <span class="text-danger">-{file.deletions}</span>
        {/if}
      </span>
    </div>

    {#if file.is_binary}
      <div class="binary-notice">Binary file — cannot display diff</div>
    {:else if file.hunks.length === 0}
      <div class="no-diff">No diff available — file may be untracked or unchanged</div>
    {:else}
      <div class="diff-content">
        {#each file.hunks as hunk, i}
          <div class="hunk-header">
            <span class="mono">@@ -{hunk.old_start},{hunk.old_count} +{hunk.new_start},{hunk.new_count} @@</span>
          </div>
          {#each hunk.lines as line}
            <div
              class="diff-line"
              class:add={line.kind === "add"}
              class:remove={line.kind === "remove"}
              class:context={line.kind === "context"}
            >
              <span class="line-gutter">
                {#if line.kind === "remove" || line.kind === "context"}
                  <span class="line-num">{""}</span>
                {:else}
                  <span class="line-num"></span>
                {/if}
                {#if line.kind === "add" || line.kind === "context"}
                  <span class="line-num">{""}</span>
                {:else}
                  <span class="line-num"></span>
                {/if}
              </span>
              <span class="line-indicator">
                {#if line.kind === "add"}+{:else if line.kind === "remove"}-{:else}{" "}{/if}
              </span>
              <span class="line-content">{line.content}</span>
            </div>
          {/each}
          {#if i < file.hunks.length - 1}
            <div class="hunk-separator"></div>
          {/if}
        {/each}
      </div>
    {/if}
  {:else}
    <div class="no-selection">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--text-mute)" stroke-width="0.8" stroke-linecap="round">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
        <line x1="16" y1="13" x2="8" y2="13"/>
        <line x1="16" y1="17" x2="8" y2="17"/>
        <polyline points="10 9 9 9 8 9"/>
      </svg>
      <span>Select a file to view diff</span>
    </div>
  {/if}
</div>

<style>
  .diff-viewer {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-0);
    overflow: hidden;
  }

  .diff-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-8) var(--space-16);
    background: var(--bg-1);
    border-bottom: 1px solid var(--border-1);
    flex-shrink: 0;
  }

  .diff-path {
    font-size: var(--text-sm);
    color: var(--text-1);
    font-weight: 500;
  }

  .diff-stats {
    display: flex;
    gap: var(--space-8);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .diff-content {
    flex: 1;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .hunk-header {
    padding: var(--space-4) var(--space-16);
    background: var(--diff-hunk-bg);
    color: var(--text-3);
    font-size: var(--text-xs);
    border-top: 1px solid var(--border-1);
    border-bottom: 1px solid var(--border-1);
  }

  .hunk-separator {
    height: var(--space-8);
    background: var(--bg-0);
  }

  .diff-line {
    display: flex;
    white-space: pre;
    min-height: 20px;
    align-items: stretch;
  }
  .diff-line.add {
    background: var(--diff-add-bg);
  }
  .diff-line.remove {
    background: var(--diff-del-bg);
  }

  .line-gutter {
    display: flex;
    flex-shrink: 0;
    width: 72px;
    user-select: none;
    border-right: 1px solid var(--border-1);
  }
  .line-num {
    width: 36px;
    padding: 0 var(--space-4);
    text-align: right;
    color: var(--text-mute);
    font-size: var(--text-xs);
  }

  .line-indicator {
    width: 20px;
    text-align: center;
    flex-shrink: 0;
    user-select: none;
  }
  .diff-line.add .line-indicator {
    color: var(--diff-add-text);
    font-weight: 700;
  }
  .diff-line.remove .line-indicator {
    color: var(--diff-del-text);
    font-weight: 700;
  }

  .line-content {
    flex: 1;
    padding-right: var(--space-16);
  }
  .diff-line.add .line-content {
    color: var(--diff-add-text);
  }
  .diff-line.remove .line-content {
    color: var(--diff-del-text);
  }

  .binary-notice, .no-diff {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--text-mute);
    font-size: var(--text-sm);
  }

  .no-selection {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-12);
    flex: 1;
    color: var(--text-mute);
    font-size: var(--text-sm);
  }
</style>
