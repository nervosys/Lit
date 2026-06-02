<script lang="ts">
  import Sidebar from "./lib/components/Sidebar.svelte";
  import BranchPanel from "./lib/components/BranchPanel.svelte";
  import FileChanges from "./lib/components/FileChanges.svelte";
  import DiffViewer from "./lib/components/DiffViewer.svelte";
  import CommitPanel from "./lib/components/CommitPanel.svelte";
  import CommitLog from "./lib/components/CommitLog.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import UndoTimeline from "./lib/components/UndoTimeline.svelte";
  import StashPanel from "./lib/components/StashPanel.svelte";
  import type { SidebarView, StatusData, FileDiff, CommitEntry, BranchEntry } from "./lib/types";
  import { getStatus, listBranches, getLog, getDiff, errMsg } from "./lib/tauri";

  let activeView: SidebarView = $state("workspace");
  let theme: "dark" | "light" = $state("dark");

  let status: StatusData | null = $state(null);
  let branches: BranchEntry[] = $state([]);
  let commits: CommitEntry[] = $state([]);
  let currentBranch: string | null = $state(null);
  let diffData: FileDiff[] = $state([]);
  let selectedFile: FileDiff | null = $state(null);
  let error: string | null = $state(null);

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", theme);
  }

  async function refresh() {
    error = null;
    try {
      const [s, b, l, d] = await Promise.all([
        getStatus(),
        listBranches(),
        getLog(50),
        getDiff(false),
      ]);
      status = s;
      branches = b.branches ?? [];
      commits = l.commits ?? [];
      currentBranch = s.branch;
      diffData = d.files ?? [];
      if (diffData.length > 0 && !selectedFile) {
        selectedFile = diffData[0];
      }
    } catch (e: any) {
      error = errMsg(e);
    }
  }

  function selectFile(file: FileDiff) {
    selectedFile = file;
  }

  function reportError(message: string) {
    error = message;
  }

  // Initial load
  refresh();
</script>

<div class="app-layout">
  <Sidebar bind:activeView {theme} onToggleTheme={toggleTheme} />

  <div class="main-area">
    <div class="top-bar">
      <div class="top-bar-left">
        <span class="logo">Lit</span>
        {#if currentBranch}
          <span class="branch-badge">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M11.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zm-2.25.75a2.25 2.25 0 1 1 3 2.122V6.5a1.5 1.5 0 0 1-1.5 1.5H9.5a1.5 1.5 0 0 0-1.5 1.5v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A3 3 0 0 1 9.5 6.5H11a.5.5 0 0 0 .5-.5v-1.128A2.251 2.251 0 0 1 9.5 3.25zM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zM4.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5z"/>
            </svg>
            {currentBranch}
          </span>
        {/if}
      </div>
      <div class="top-bar-actions">
        <button class="btn-icon" title="Refresh (Ctrl+R)" onclick={refresh}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
            <path d="M13.5 2v4h-4M2.5 14v-4h4"/>
            <path d="M2.51 6.5A5.5 5.5 0 0 1 8 2.5a5.5 5.5 0 0 1 4.38 2.16l1.12 1.34M13.49 9.5A5.5 5.5 0 0 1 8 13.5a5.5 5.5 0 0 1-4.38-2.16l-1.12-1.34" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>
    </div>

    {#if error}
      <div class="error-banner">
        <span>{error}</span>
        <button onclick={() => error = null}>✕</button>
      </div>
    {/if}

    <div class="content-area">
      {#if activeView === "workspace"}
        <div class="workspace-layout">
          <div class="panel panel-branches">
            <BranchPanel {branches} {currentBranch} onRefresh={refresh} onError={reportError} />
          </div>
          <div class="panel panel-files">
            <FileChanges {status} {diffData} {selectedFile} onSelectFile={selectFile} onRefresh={refresh} onError={reportError} />
          </div>
          <div class="panel panel-diff">
            <DiffViewer file={selectedFile} />
          </div>
          <div class="panel panel-commit">
            <CommitPanel {status} onRefresh={refresh} onError={reportError} />
          </div>
        </div>
      {:else if activeView === "branches"}
        <div class="workspace-layout">
          <div class="panel panel-branches" style="flex: 0 0 300px;">
            <BranchPanel {branches} {currentBranch} onRefresh={refresh} onError={reportError} />
          </div>
          <div class="panel panel-diff" style="flex: 1;">
            <CommitLog {commits} {currentBranch} onRefresh={refresh} onError={reportError} />
          </div>
        </div>
      {:else if activeView === "stash"}
        <div class="workspace-layout">
          <div class="panel panel-branches" style="flex: 0 0 360px;">
            <StashPanel onRefresh={refresh} onError={reportError} />
          </div>
          <div class="panel panel-diff" style="flex: 1;">
            <CommitLog {commits} {currentBranch} onRefresh={refresh} onError={reportError} />
          </div>
        </div>
      {:else if activeView === "history"}
        <div class="workspace-layout">
          <div class="panel" style="flex: 1;">
            <UndoTimeline onError={reportError} />
          </div>
        </div>
      {:else if activeView === "stacks"}
        <div class="workspace-layout">
          <div class="panel panel-branches" style="flex: 0 0 300px;">
            <BranchPanel {branches} {currentBranch} onRefresh={refresh} showStacks={true} onError={reportError} />
          </div>
          <div class="panel panel-diff" style="flex: 1;">
            <CommitLog {commits} {currentBranch} onRefresh={refresh} onError={reportError} />
          </div>
        </div>
      {/if}
    </div>

    <StatusBar {status} {currentBranch} {theme} />
  </div>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--bg-0);
  }

  .top-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 var(--space-12);
    background: var(--bg-1);
    border-bottom: 1px solid var(--border-1);
    flex-shrink: 0;
    -webkit-app-region: drag;
  }

  .top-bar-left {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    -webkit-app-region: no-drag;
  }

  .logo {
    font-size: var(--text-lg);
    font-weight: 700;
    color: var(--accent);
    letter-spacing: -0.5px;
  }

  .branch-badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-8);
    background: var(--bg-3);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-family: var(--font-mono);
    color: var(--text-2);
  }

  .top-bar-actions {
    display: flex;
    gap: var(--space-4);
    -webkit-app-region: no-drag;
  }

  .btn-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    color: var(--text-2);
    transition: all var(--transition-fast);
  }
  .btn-icon:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }

  .error-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-8) var(--space-16);
    background: var(--danger-soft);
    color: var(--danger-text);
    font-size: var(--text-sm);
    border-bottom: 1px solid var(--danger);
  }
  .error-banner button {
    color: var(--danger-text);
    opacity: 0.7;
  }
  .error-banner button:hover {
    opacity: 1;
  }

  .content-area {
    flex: 1;
    overflow: hidden;
  }

  .workspace-layout {
    display: flex;
    height: 100%;
  }

  .panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border-right: 1px solid var(--border-1);
    overflow: hidden;
  }
  .panel:last-child {
    border-right: none;
  }

  .panel-branches {
    flex: 0 0 220px;
  }
  .panel-files {
    flex: 0 0 280px;
  }
  .panel-diff {
    flex: 1;
    min-width: 200px;
  }
  .panel-commit {
    flex: 0 0 var(--commit-panel-width);
  }
</style>
