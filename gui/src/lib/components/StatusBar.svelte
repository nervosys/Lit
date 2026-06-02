<script lang="ts">
  import type { StatusData, Theme } from "../types";

  let {
    status,
    currentBranch,
    theme,
  }: {
    status: StatusData | null;
    currentBranch: string | null;
    theme: Theme;
  } = $props();

  let totalChanges = $derived(
    (status?.staged.length ?? 0) +
    (status?.modified.length ?? 0) +
    (status?.untracked.length ?? 0)
  );
</script>

<footer class="status-bar">
  <div class="status-left">
    {#if currentBranch}
      <span class="status-item">
        <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
          <path d="M11.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zm-2.25.75a2.25 2.25 0 1 1 3 2.122V6.5a1.5 1.5 0 0 1-1.5 1.5H9.5a1.5 1.5 0 0 0-1.5 1.5v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A3 3 0 0 1 9.5 6.5H11a.5.5 0 0 0 .5-.5v-1.128A2.251 2.251 0 0 1 9.5 3.25zM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zM4.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5z"/>
        </svg>
        {currentBranch}
      </span>
    {/if}
    {#if status && !status.clean}
      <span class="status-item changes">
        {totalChanges} change{totalChanges !== 1 ? "s" : ""}
      </span>
    {:else if status?.clean}
      <span class="status-item clean">✓ Clean</span>
    {/if}
  </div>

  <div class="status-right">
    <span class="status-item">Lit v1.0.0</span>
  </div>
</footer>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 24px;
    padding: 0 var(--space-12);
    background: var(--accent);
    color: rgba(255, 255, 255, 0.9);
    font-size: 11px;
    flex-shrink: 0;
    user-select: none;
  }

  .status-left, .status-right {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .status-item.clean {
    color: rgba(255, 255, 255, 0.75);
  }
  .status-item.changes {
    color: #fcd34d;
  }
</style>
