<script lang="ts">
  import type { StashEntry } from "../types";
  import { stashSave, stashList, stashPop, stashApply, stashDrop, errMsg } from "../tauri";

  let {
    onRefresh,
    onError,
  }: {
    onRefresh: () => void;
    onError?: (message: string) => void;
  } = $props();

  let entries: StashEntry[] = $state([]);
  let stashMessage = $state("");
  let showSaveForm = $state(false);
  let loading = $state(false);

  function formatTime(ts: number) {
    if (!ts) return "";
    const d = new Date(ts * 1000);
    const diff = Date.now() - d.getTime();
    if (diff < 60_000) return "just now";
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
    if (diff < 604_800_000) return `${Math.floor(diff / 86_400_000)}d ago`;
    return d.toLocaleDateString();
  }

  async function load() {
    loading = true;
    try {
      const result = await stashList();
      entries = result.entries ?? [];
    } catch (e) {
      onError?.(errMsg(e));
    } finally {
      loading = false;
    }
  }

  async function handleSave(e: Event) {
    e.preventDefault();
    try {
      await stashSave(stashMessage.trim() || undefined);
      stashMessage = "";
      showSaveForm = false;
      await load();
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function handlePop() {
    try {
      await stashPop();
      await load();
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function handleApply(index: number) {
    try {
      await stashApply(index);
      await load();
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function handleDrop(index: number) {
    try {
      await stashDrop(index);
      await load();
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  $effect(() => {
    load();
  });
</script>

<div class="stash-panel">
  <div class="panel-header">
    <span class="panel-title">Stash</span>
    <div class="header-actions">
      <button class="btn-sm" title="Refresh" onclick={load}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
          <path d="M8 3a5 5 0 1 0 4.546 2.914.75.75 0 0 1 1.364-.626A6.5 6.5 0 1 1 8 1.5a.75.75 0 0 1 0 1.5z"/>
          <path d="M8 .5a.75.75 0 0 1 .53.22l2 2a.75.75 0 0 1 0 1.06l-2 2A.75.75 0 0 1 7.25 5.25V1.25A.75.75 0 0 1 8 .5z"/>
        </svg>
      </button>
      <button class="btn-sm" title="Stash changes" onclick={() => (showSaveForm = !showSaveForm)}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
          <path d="M8 2a.75.75 0 0 1 .75.75v4.5h4.5a.75.75 0 0 1 0 1.5h-4.5v4.5a.75.75 0 0 1-1.5 0v-4.5h-4.5a.75.75 0 0 1 0-1.5h4.5v-4.5A.75.75 0 0 1 8 2z"/>
        </svg>
      </button>
    </div>
  </div>

  {#if showSaveForm}
    <form class="save-form" onsubmit={handleSave}>
      <input
        type="text"
        placeholder="Stash message (optional)..."
        bind:value={stashMessage}
        class="stash-input"
      />
      <button type="submit" class="btn-save">Stash</button>
    </form>
  {/if}

  <div class="stash-list">
    {#if loading && entries.length === 0}
      <div class="empty-text">Loading…</div>
    {:else}
      {#each entries as entry}
        <div class="stash-item">
          <div class="stash-badge mono">stash@{`{${entry.index}}`}</div>
          <div class="stash-content">
            <span class="stash-msg truncate">{entry.message}</span>
            <div class="stash-meta">
              {#if entry.branch}
                <span class="stash-branch mono">{entry.branch}</span>
                <span class="stash-sep">·</span>
              {/if}
              <span class="stash-time">{formatTime(entry.timestamp)}</span>
            </div>
          </div>
          <div class="stash-actions">
            <button class="btn-action" title="Pop (apply + drop)" onclick={handlePop}>
              Pop
            </button>
            <button class="btn-action" title="Apply (keep in stash)" onclick={() => handleApply(entry.index)}>
              Apply
            </button>
            <button class="btn-action danger" title="Drop" onclick={() => handleDrop(entry.index)}>
              <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
                <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06z"/>
              </svg>
            </button>
          </div>
        </div>
      {:else}
        <div class="empty-text">No stash entries</div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .stash-panel {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-10) var(--space-12);
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
  .header-actions {
    display: flex;
    gap: var(--space-4);
  }

  .btn-sm {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm);
    color: var(--text-3);
    transition: all var(--transition-fast);
  }
  .btn-sm:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }

  .save-form {
    display: flex;
    gap: var(--space-6);
    padding: var(--space-8) var(--space-12);
    border-bottom: 1px solid var(--border-1);
  }
  .stash-input {
    flex: 1;
    font-size: var(--text-sm);
    padding: var(--space-4) var(--space-8);
  }
  .btn-save {
    padding: var(--space-4) var(--space-10);
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: 500;
    transition: background var(--transition-fast);
  }
  .btn-save:hover {
    background: var(--accent-hover);
  }

  .stash-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4) var(--space-8);
  }

  .stash-item {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    padding: var(--space-8);
    border-radius: var(--radius-sm);
    border-bottom: 1px solid var(--border-1);
    transition: background var(--transition-fast);
  }
  .stash-item:hover {
    background: var(--bg-hover);
  }

  .stash-badge {
    font-size: var(--text-xs);
    color: var(--accent-text);
    background: var(--accent-soft);
    padding: var(--space-2) var(--space-6);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .stash-content {
    flex: 1;
    min-width: 0;
  }
  .stash-msg {
    display: block;
    font-size: var(--text-sm);
    color: var(--text-1);
    font-weight: 500;
  }
  .stash-meta {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin-top: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text-mute);
  }
  .stash-branch {
    color: var(--text-3);
  }
  .stash-sep {
    color: var(--text-mute);
  }

  .stash-actions {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    flex-shrink: 0;
    opacity: 0;
    transition: opacity var(--transition-fast);
  }
  .stash-item:hover .stash-actions {
    opacity: 1;
  }
  .btn-action {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-2) var(--space-8);
    height: 24px;
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-2);
    background: var(--bg-3);
    transition: all var(--transition-fast);
  }
  .btn-action:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }
  .btn-action.danger:hover {
    background: var(--danger-soft);
    color: var(--danger-text);
  }

  .empty-text {
    text-align: center;
    color: var(--text-mute);
    font-size: var(--text-sm);
    padding: var(--space-20);
  }
</style>
