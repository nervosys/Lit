<script lang="ts">
  import type { CommitEntry, SearchMatch } from "../types";
  import { searchCommits, rewordCommit, errMsg } from "../tauri";

  let {
    commits,
    currentBranch,
    onRefresh,
    onError,
  }: {
    commits: CommitEntry[];
    currentBranch: string | null;
    onRefresh?: () => void;
    onError?: (message: string) => void;
  } = $props();

  let selected: string | null = $state(null);

  let searchQuery = $state("");
  let searchResults: SearchMatch[] | null = $state(null);
  let searching = $state(false);

  let rewordingHash: string | null = $state(null);
  let rewordText = $state("");

  async function runSearch(e: Event) {
    e.preventDefault();
    const q = searchQuery.trim();
    if (!q) {
      searchResults = null;
      return;
    }
    searching = true;
    try {
      const res = await searchCommits(q);
      searchResults = res.matches ?? [];
    } catch (err) {
      onError?.(errMsg(err));
    } finally {
      searching = false;
    }
  }

  function clearSearch() {
    searchQuery = "";
    searchResults = null;
  }

  function startReword(commit: CommitEntry) {
    rewordingHash = commit.hash;
    rewordText = firstLine(commit.message);
  }

  function cancelReword() {
    rewordingHash = null;
    rewordText = "";
  }

  async function saveReword(e: Event) {
    e.preventDefault();
    const msg = rewordText.trim();
    if (!msg) return;
    try {
      await rewordCommit(msg);
      cancelReword();
      onRefresh?.();
    } catch (err) {
      onError?.(errMsg(err));
    }
  }

  function formatTime(ts: number) {
    const d = new Date(ts * 1000);
    const now = Date.now();
    const diff = now - d.getTime();
    if (diff < 60_000) return "just now";
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
    if (diff < 604_800_000) return `${Math.floor(diff / 86_400_000)}d ago`;
    return d.toLocaleDateString();
  }

  function firstLine(msg: string) {
    return msg.split("\n")[0] ?? msg;
  }

  function initials(author: string) {
    return author
      .split(/[\s@]+/)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase() ?? "")
      .join("");
  }
</script>

<div class="commit-log">
  <div class="panel-header">
    <span class="panel-title">Commit History</span>
    {#if currentBranch}
      <span class="branch-label mono">{currentBranch}</span>
    {/if}
  </div>

  <form class="search-bar" onsubmit={runSearch}>
    <svg class="search-icon" width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
      <path d="M11.742 10.344a6.5 6.5 0 1 0-1.397 1.398h-.001q.044.06.098.115l3.85 3.85a1 1 0 0 0 1.415-1.414l-3.85-3.85a1 1 0 0 0-.115-.1zM12 6.5a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0z"/>
    </svg>
    <input
      type="text"
      class="search-input"
      placeholder="Search commits & content…"
      bind:value={searchQuery}
    />
    {#if searchResults !== null}
      <button type="button" class="search-clear" title="Clear" onclick={clearSearch}>✕</button>
    {/if}
  </form>

  {#if searchResults !== null}
    <div class="log-list">
      <div class="search-summary">
        {searching ? "Searching…" : `${searchResults.length} match${searchResults.length === 1 ? "" : "es"} for "${searchQuery}"`}
      </div>
      {#each searchResults as m}
        <div class="match-row">
          <div class="match-head">
            <span class="match-file truncate">{m.file}</span>
            <span class="commit-sep">·</span>
            <span class="match-line mono">L{m.line_number}</span>
            {#if m.commit}
              <span class="commit-sep">·</span>
              <span class="commit-hash mono">{m.commit.slice(0, 8)}</span>
            {/if}
          </div>
          <pre class="match-content mono">{m.content}</pre>
        </div>
      {:else}
        {#if !searching}
          <div class="empty-state"><span class="text-mute">No matches</span></div>
        {/if}
      {/each}
    </div>
  {:else}
    <div class="log-list">
      {#each commits as commit, i}
        <div
          class="commit-row"
          class:selected={selected === commit.hash}
          class:head={commit.is_head}
        >
          <button
            class="commit-click"
            onclick={() => (selected = selected === commit.hash ? null : commit.hash)}
          >
            <div class="commit-graph">
              <div class="graph-line top" class:hidden={i === 0}></div>
              <div class="graph-dot" class:head={commit.is_head}></div>
              <div class="graph-line bottom" class:hidden={i === commits.length - 1}></div>
            </div>

            <div class="commit-content">
              <div class="commit-main">
                <span class="commit-msg truncate">{firstLine(commit.message)}</span>
                {#if commit.is_head}
                  <span class="head-badge">HEAD</span>
                {/if}
              </div>
              <div class="commit-meta">
                <span class="commit-hash mono">{commit.short_hash}</span>
                <span class="commit-sep">·</span>
                <span class="commit-author">{commit.author}</span>
                <span class="commit-sep">·</span>
                <span class="commit-time">{formatTime(commit.timestamp)}</span>
              </div>
            </div>

            <div class="commit-avatar" title={commit.author}>
              {initials(commit.author)}
            </div>
          </button>

          {#if commit.is_head}
            <button class="btn-reword" title="Reword message" onclick={() => startReword(commit)}>
              <svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor">
                <path d="M11.013 1.427a1.75 1.75 0 0 1 2.474 0l1.086 1.086a1.75 1.75 0 0 1 0 2.474l-8.61 8.61c-.21.21-.47.364-.756.445l-3.251.93a.75.75 0 0 1-.927-.928l.929-3.25c.081-.286.235-.547.445-.758l8.61-8.61zm1.414 1.06a.25.25 0 0 0-.354 0L10.811 3.75l1.439 1.44 1.263-1.263a.25.25 0 0 0 0-.354l-1.086-1.086zM11.189 6.25 9.75 4.81l-6.286 6.287a.25.25 0 0 0-.064.108l-.558 1.953 1.953-.558a.25.25 0 0 0 .108-.064L11.189 6.25z"/>
              </svg>
            </button>
          {/if}

          {#if rewordingHash === commit.hash}
            <form class="reword-form" onsubmit={saveReword}>
              <input
                type="text"
                class="reword-input"
                bind:value={rewordText}
                placeholder="New commit message…"
              />
              <button type="submit" class="btn-reword-save" disabled={!rewordText.trim()}>Save</button>
              <button type="button" class="btn-reword-cancel" onclick={cancelReword}>Cancel</button>
            </form>
          {/if}
        </div>
      {:else}
        <div class="empty-state">
          <span class="text-mute">No commits yet</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .commit-log {
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
  .branch-label {
    font-size: var(--text-xs);
    color: var(--text-3);
    padding: var(--space-2) var(--space-6);
    background: var(--bg-3);
    border-radius: var(--radius-sm);
  }

  .log-list {
    flex: 1;
    overflow-y: auto;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    padding: var(--space-8) var(--space-12);
    border-bottom: 1px solid var(--border-1);
    background: var(--bg-1);
    flex-shrink: 0;
  }
  .search-icon {
    color: var(--text-mute);
    flex-shrink: 0;
  }
  .search-input {
    flex: 1;
    font-size: var(--text-sm);
    padding: var(--space-4) var(--space-8);
  }
  .search-clear {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    color: var(--text-mute);
    flex-shrink: 0;
  }
  .search-clear:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }

  .search-summary {
    padding: var(--space-8) var(--space-16);
    font-size: var(--text-xs);
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .match-row {
    padding: var(--space-8) var(--space-16);
    border-bottom: 1px solid var(--border-1);
  }
  .match-head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    font-size: var(--text-xs);
    color: var(--text-3);
    margin-bottom: var(--space-4);
  }
  .match-file {
    color: var(--text-2);
    font-weight: 500;
  }
  .match-line {
    color: var(--accent-text);
  }
  .match-content {
    font-size: var(--text-xs);
    color: var(--text-2);
    background: var(--bg-2);
    padding: var(--space-6) var(--space-8);
    border-radius: var(--radius-sm);
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
  }

  .commit-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    width: 100%;
    transition: background var(--transition-fast);
    border-bottom: 1px solid var(--border-1);
  }
  .commit-row:hover {
    background: var(--bg-hover);
  }
  .commit-row.selected {
    background: var(--accent-soft);
  }

  .commit-click {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    flex: 1;
    min-width: 0;
    padding: var(--space-8) var(--space-16);
    text-align: left;
  }

  .btn-reword {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    margin-right: var(--space-12);
    border-radius: var(--radius-sm);
    color: var(--text-mute);
    opacity: 0;
    transition: all var(--transition-fast);
  }
  .commit-row:hover .btn-reword {
    opacity: 1;
  }
  .btn-reword:hover {
    background: var(--bg-3);
    color: var(--accent-text);
  }

  .reword-form {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    flex-basis: 100%;
    padding: var(--space-8) var(--space-16) var(--space-12);
  }
  .reword-input {
    flex: 1;
    font-size: var(--text-sm);
    padding: var(--space-4) var(--space-8);
  }
  .btn-reword-save {
    padding: var(--space-4) var(--space-10);
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: 500;
  }
  .btn-reword-save:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-reword-cancel {
    padding: var(--space-4) var(--space-10);
    background: var(--bg-3);
    color: var(--text-2);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
  }
  .btn-reword-cancel:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }

  .commit-graph {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 20px;
    flex-shrink: 0;
    align-self: stretch;
  }
  .graph-line {
    flex: 1;
    width: 2px;
    background: var(--border-2);
  }
  .graph-line.hidden {
    background: transparent;
  }
  .graph-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border-3);
    flex-shrink: 0;
    border: 2px solid var(--bg-0);
    box-sizing: content-box;
  }
  .graph-dot.head {
    background: var(--accent);
    width: 10px;
    height: 10px;
  }

  .commit-content {
    flex: 1;
    min-width: 0;
  }
  .commit-main {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    margin-bottom: var(--space-2);
  }
  .commit-msg {
    font-size: var(--text-sm);
    color: var(--text-1);
    font-weight: 500;
  }
  .head-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--accent);
    color: #fff;
    flex-shrink: 0;
    letter-spacing: 0.03em;
  }

  .commit-meta {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    font-size: var(--text-xs);
    color: var(--text-3);
  }
  .commit-hash {
    color: var(--accent-text);
  }
  .commit-sep {
    color: var(--text-mute);
  }

  .commit-avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--bg-3);
    color: var(--text-3);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 600;
    flex-shrink: 0;
    letter-spacing: -0.5px;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-24);
    font-size: var(--text-sm);
  }
</style>
