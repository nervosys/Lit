<script lang="ts">
  import type { OpLogEntry } from "../types";
  import { undoList, undoUndo, undoRedo, errMsg } from "../tauri";

  let {
    onError,
  }: {
    onError?: (message: string) => void;
  } = $props();

  let entries: OpLogEntry[] = $state([]);
  let loading = $state(true);
  let error: string | null = $state(null);

  async function load() {
    loading = true;
    error = null;
    try {
      const result = await undoList();
      entries = result.entries ?? [];
    } catch (e: any) {
      error = typeof e === "string" ? e : e?.message ?? "Failed to load undo history";
    } finally {
      loading = false;
    }
  }

  async function handleUndo() {
    try {
      await undoUndo();
      await load();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function handleRedo() {
    try {
      await undoRedo();
      await load();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  load();
</script>

<div class="undo-timeline">
  <div class="panel-header">
    <span class="panel-title">Undo Timeline</span>
    <div class="undo-actions">
      <button class="btn-undo" onclick={handleUndo} title="Undo last operation">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="1 4 1 10 7 10"/>
          <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"/>
        </svg>
        Undo
      </button>
      <button class="btn-redo" onclick={handleRedo} title="Redo last operation">
        Redo
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="23 4 23 10 17 10"/>
          <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
        </svg>
      </button>
    </div>
  </div>

  <div class="timeline-list">
    {#if loading}
      <div class="empty-state">Loading...</div>
    {:else if error}
      <div class="empty-state error">{error}</div>
    {:else if entries.length === 0}
      <div class="empty-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="var(--text-mute)" stroke-width="0.8" stroke-linecap="round">
          <circle cx="12" cy="12" r="10"/>
          <polyline points="12 6 12 12 16 14"/>
        </svg>
        <span>No operations recorded yet</span>
        <span class="hint">Operations like commit, checkout, and merge will appear here</span>
      </div>
    {:else}
      {#each entries as entry, i}
        <div class="timeline-entry" class:undone={entry.undone}>
          <div class="timeline-graph">
            <div class="tl-line top" class:hidden={i === 0}></div>
            <div class="tl-dot" class:undone={entry.undone}></div>
            <div class="tl-line bottom" class:hidden={i === entries.length - 1}></div>
          </div>
          <div class="timeline-content">
            <div class="tl-operation">{entry.operation}</div>
            <div class="tl-description">{entry.description}</div>
            <div class="tl-time">{entry.timestamp}</div>
          </div>
          {#if entry.undone}
            <span class="undone-badge">undone</span>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .undo-timeline {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-0);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-10) var(--space-16);
    background: var(--bg-1);
    border-bottom: 1px solid var(--border-1);
    flex-shrink: 0;
  }
  .panel-title {
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-3);
  }

  .undo-actions {
    display: flex;
    gap: var(--space-6);
  }
  .btn-undo, .btn-redo {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-8);
    font-size: var(--text-xs);
    font-weight: 500;
    border-radius: var(--radius-sm);
    color: var(--text-2);
    transition: all var(--transition-fast);
  }
  .btn-undo:hover, .btn-redo:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }

  .timeline-list {
    flex: 1;
    overflow-y: auto;
  }

  .timeline-entry {
    display: flex;
    align-items: center;
    gap: var(--space-10);
    padding: var(--space-10) var(--space-16);
    border-bottom: 1px solid var(--border-1);
    transition: background var(--transition-fast);
  }
  .timeline-entry:hover {
    background: var(--bg-hover);
  }
  .timeline-entry.undone {
    opacity: 0.5;
  }

  .timeline-graph {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 16px;
    align-self: stretch;
  }
  .tl-line {
    flex: 1;
    width: 2px;
    background: var(--border-2);
  }
  .tl-line.hidden { background: transparent; }
  .tl-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }
  .tl-dot.undone {
    background: var(--text-mute);
  }

  .timeline-content {
    flex: 1;
    min-width: 0;
  }
  .tl-operation {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-1);
    text-transform: capitalize;
  }
  .tl-description {
    font-size: var(--text-xs);
    color: var(--text-2);
    margin-top: var(--space-2);
  }
  .tl-time {
    font-size: var(--text-xs);
    color: var(--text-mute);
    font-family: var(--font-mono);
    margin-top: var(--space-2);
  }

  .undone-badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--warn-soft);
    color: var(--warn-text);
    font-weight: 600;
    text-transform: uppercase;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-10);
    padding: var(--space-24);
    color: var(--text-mute);
    font-size: var(--text-sm);
    min-height: 200px;
  }
  .empty-state.error {
    color: var(--danger-text);
  }
  .hint {
    font-size: var(--text-xs);
    color: var(--text-mute);
  }
</style>
