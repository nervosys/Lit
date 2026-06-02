<script lang="ts">
  import type { StatusData, FileDiff } from "../types";
  import { stageFiles, errMsg } from "../tauri";

  let {
    status,
    diffData,
    selectedFile,
    onSelectFile,
    onRefresh,
    onError,
  }: {
    status: StatusData | null;
    diffData: FileDiff[];
    selectedFile: FileDiff | null;
    onSelectFile: (file: FileDiff) => void;
    onRefresh: () => void;
    onError?: (message: string) => void;
  } = $props();

  let activeTab: "changes" | "staged" = $state("changes");

  let changedFiles = $derived(
    diffData.length > 0
      ? diffData
      : [
          ...(status?.modified ?? []).map(
            (p): FileDiff => ({
              path: p,
              status: "modified",
              hunks: [],
              old_hash: null,
              new_hash: null,
              is_binary: false,
              additions: 0,
              deletions: 0,
            })
          ),
          ...(status?.untracked ?? []).map(
            (p): FileDiff => ({
              path: p,
              status: "added",
              hunks: [],
              old_hash: null,
              new_hash: null,
              is_binary: false,
              additions: 0,
              deletions: 0,
            })
          ),
        ]
  );

  let stagedFiles = $derived(
    (status?.staged ?? []).map(
      (p): FileDiff => ({
        path: p,
        status: "modified",
        hunks: [],
        old_hash: null,
        new_hash: null,
        is_binary: false,
        additions: 0,
        deletions: 0,
      })
    )
  );

  function statusIcon(s: string) {
    switch (s) {
      case "added":
        return "A";
      case "deleted":
        return "D";
      default:
        return "M";
    }
  }

  function statusColor(s: string) {
    switch (s) {
      case "added":
        return "var(--safe)";
      case "deleted":
        return "var(--danger)";
      default:
        return "var(--warn)";
    }
  }

  function fileName(path: string) {
    return path.split("/").pop() ?? path;
  }

  function dirPath(path: string) {
    const parts = path.split("/");
    return parts.length > 1 ? parts.slice(0, -1).join("/") + "/" : "";
  }

  async function stageAll() {
    const paths = changedFiles.map((f) => f.path);
    if (paths.length === 0) return;
    try {
      await stageFiles(paths);
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function stageFile(path: string) {
    try {
      await stageFiles([path]);
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }
</script>

<div class="file-changes">
  <div class="panel-header">
    <div class="tab-bar">
      <button
        class="tab" class:active={activeTab === "changes"}
        onclick={() => (activeTab = "changes")}
      >
        Changes
        {#if changedFiles.length > 0}
          <span class="tab-count">{changedFiles.length}</span>
        {/if}
      </button>
      <button
        class="tab" class:active={activeTab === "staged"}
        onclick={() => (activeTab = "staged")}
      >
        Staged
        {#if stagedFiles.length > 0}
          <span class="tab-count">{stagedFiles.length}</span>
        {/if}
      </button>
    </div>
    {#if activeTab === "changes" && changedFiles.length > 0}
      <button class="btn-stage-all" title="Stage all" onclick={stageAll}>
        Stage all
      </button>
    {/if}
  </div>

  <div class="file-list">
    {#if activeTab === "changes"}
      {#each changedFiles as file}
        <div
          class="file-item"
          class:selected={selectedFile?.path === file.path}
          onclick={() => onSelectFile(file)}
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') onSelectFile(file); }}
          role="button"
          tabindex="0"
        >
          <span class="file-status" style="color: {statusColor(file.status)}">{statusIcon(file.status)}</span>
          <span class="file-info">
            <span class="file-name truncate">{fileName(file.path)}</span>
            <span class="file-dir truncate text-mute">{dirPath(file.path)}</span>
          </span>
          {#if file.additions > 0 || file.deletions > 0}
            <span class="file-stats">
              {#if file.additions > 0}<span class="text-safe">+{file.additions}</span>{/if}
              {#if file.deletions > 0}<span class="text-danger">-{file.deletions}</span>{/if}
            </span>
          {/if}
          <button
            class="btn-stage-file"
            title="Stage file"
            onclick={(e: MouseEvent) => { e.stopPropagation(); stageFile(file.path); }}
          >+</button>
        </div>
      {:else}
        <div class="empty-state">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="var(--text-mute)" stroke-width="1" stroke-linecap="round">
            <path d="M9 12l2 2 4-4"/>
            <circle cx="12" cy="12" r="10"/>
          </svg>
          <span>Working tree clean</span>
        </div>
      {/each}
    {:else}
      {#each stagedFiles as file}
        <button
          class="file-item"
          class:selected={selectedFile?.path === file.path}
          onclick={() => onSelectFile(file)}
        >
          <span class="file-status" style="color: var(--safe)">S</span>
          <span class="file-info">
            <span class="file-name truncate">{fileName(file.path)}</span>
            <span class="file-dir truncate text-mute">{dirPath(file.path)}</span>
          </span>
        </button>
      {:else}
        <div class="empty-state">
          <span>No staged files</span>
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .file-changes {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-6) var(--space-12);
    border-bottom: 1px solid var(--border-1);
    flex-shrink: 0;
    gap: var(--space-8);
  }

  .tab-bar {
    display: flex;
    gap: var(--space-2);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-8);
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-3);
    border-radius: var(--radius-sm);
    transition: all var(--transition-fast);
  }
  .tab:hover {
    background: var(--bg-hover);
    color: var(--text-2);
  }
  .tab.active {
    background: var(--bg-3);
    color: var(--text-1);
  }

  .tab-count {
    font-size: 10px;
    min-width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    background: var(--bg-4);
    color: var(--text-2);
    font-weight: 600;
    padding: 0 4px;
  }

  .btn-stage-all {
    font-size: var(--text-xs);
    padding: var(--space-2) var(--space-8);
    border-radius: var(--radius-sm);
    color: var(--accent-text);
    background: var(--accent-soft);
    font-weight: 500;
    transition: all var(--transition-fast);
    white-space: nowrap;
  }
  .btn-stage-all:hover {
    background: var(--accent);
    color: #fff;
  }

  .file-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4);
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    width: 100%;
    padding: var(--space-4) var(--space-8);
    border-radius: var(--radius-sm);
    text-align: left;
    transition: all var(--transition-fast);
  }
  .file-item:hover {
    background: var(--bg-hover);
  }
  .file-item.selected {
    background: var(--accent-soft);
  }

  .file-status {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 700;
    width: 16px;
    text-align: center;
    flex-shrink: 0;
  }

  .file-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .file-name {
    font-size: var(--text-sm);
    line-height: 1.3;
  }
  .file-dir {
    font-size: var(--text-xs);
    line-height: 1.3;
  }

  .file-stats {
    display: flex;
    gap: var(--space-4);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    flex-shrink: 0;
  }

  .btn-stage-file {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: 700;
    color: var(--safe);
    opacity: 0;
    transition: all var(--transition-fast);
  }
  .file-item:hover .btn-stage-file {
    opacity: 1;
  }
  .btn-stage-file:hover {
    background: var(--safe-soft);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-8);
    padding: var(--space-24);
    color: var(--text-mute);
    font-size: var(--text-sm);
  }
</style>
