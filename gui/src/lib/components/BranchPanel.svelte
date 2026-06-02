<script lang="ts">
  import type { BranchEntry } from "../types";
  import { checkoutBranch, createBranch, deleteBranch, stackList, errMsg } from "../tauri";
  import type { StackEntry } from "../types";

  let {
    branches,
    currentBranch,
    onRefresh,
    onError,
    showStacks = false,
  }: {
    branches: BranchEntry[];
    currentBranch: string | null;
    onRefresh: () => void;
    onError?: (message: string) => void;
    showStacks?: boolean;
  } = $props();

  let newBranchName = $state("");
  let showNewBranch = $state(false);
  let stacks: StackEntry[] = $state([]);
  let filter = $state("");

  let filteredBranches = $derived(
    filter
      ? branches.filter((b) => b.name.toLowerCase().includes(filter.toLowerCase()))
      : branches
  );

  async function handleCheckout(name: string) {
    try {
      await checkoutBranch(name);
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function handleCreate(e: Event) {
    e.preventDefault();
    if (!newBranchName.trim()) return;
    try {
      await createBranch(newBranchName.trim());
      newBranchName = "";
      showNewBranch = false;
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function handleDelete(name: string) {
    try {
      await deleteBranch(name);
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  async function loadStacks() {
    try {
      const result = await stackList();
      stacks = result.stacks ?? [];
    } catch (e) {
      onError?.(errMsg(e));
    }
  }

  $effect(() => {
    if (showStacks) loadStacks();
  });
</script>

<div class="branch-panel">
  <div class="panel-header">
    <span class="panel-title">{showStacks ? "Stacks" : "Branches"}</span>
    <button class="btn-sm" title="New branch" onclick={() => (showNewBranch = !showNewBranch)}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
        <path d="M8 2a.75.75 0 0 1 .75.75v4.5h4.5a.75.75 0 0 1 0 1.5h-4.5v4.5a.75.75 0 0 1-1.5 0v-4.5h-4.5a.75.75 0 0 1 0-1.5h4.5v-4.5A.75.75 0 0 1 8 2z"/>
      </svg>
    </button>
  </div>

  {#if showNewBranch}
    <form class="new-branch-form" onsubmit={handleCreate}>
      <input
        type="text"
        placeholder="Branch name..."
        bind:value={newBranchName}
        class="branch-input"
      />
      <button type="submit" class="btn-create" disabled={!newBranchName.trim()}>Create</button>
    </form>
  {/if}

  <div class="filter-box">
    <input type="text" placeholder="Filter..." bind:value={filter} class="filter-input" />
  </div>

  <div class="branch-list">
    {#if showStacks && stacks.length > 0}
      <div class="section-label">Stacked Branches</div>
      {#each stacks as stack}
        <button
          class="branch-item"
          class:current={stack.branch === currentBranch}
          onclick={() => handleCheckout(stack.branch)}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M12 2L2 7l10 5 10-5-10-5z"/>
            <path d="M2 17l10 5 10-5"/>
          </svg>
          <span class="branch-name truncate">{stack.branch}</span>
          <span class="stack-base text-mute">← {stack.base}</span>
        </button>
      {/each}
      <div class="section-label" style="margin-top: var(--space-12);">All Branches</div>
    {/if}

    {#each filteredBranches as branch}
      <div class="branch-item-row">
        <button
          class="branch-item"
          class:current={branch.is_current}
          onclick={() => handleCheckout(branch.name)}
        >
          <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" class="branch-icon">
            <path d="M11.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zm-2.25.75a2.25 2.25 0 1 1 3 2.122V6.5a1.5 1.5 0 0 1-1.5 1.5H9.5a1.5 1.5 0 0 0-1.5 1.5v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A3 3 0 0 1 9.5 6.5H11a.5.5 0 0 0 .5-.5v-1.128A2.251 2.251 0 0 1 9.5 3.25zM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zM4.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5z"/>
          </svg>
          <span class="branch-name truncate">{branch.name}</span>
          {#if branch.is_current}
            <span class="current-badge">HEAD</span>
          {/if}
        </button>
        {#if !branch.is_current}
          <button class="btn-delete" title="Delete branch" onclick={() => handleDelete(branch.name)}>
            <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
              <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06z"/>
            </svg>
          </button>
        {/if}
      </div>
    {:else}
      <div class="empty-text">No branches found</div>
    {/each}
  </div>
</div>

<style>
  .branch-panel {
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

  .new-branch-form {
    display: flex;
    gap: var(--space-6);
    padding: var(--space-8) var(--space-12);
    border-bottom: 1px solid var(--border-1);
  }
  .branch-input {
    flex: 1;
    font-size: var(--text-sm);
    padding: var(--space-4) var(--space-8);
  }
  .btn-create {
    padding: var(--space-4) var(--space-10);
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    font-weight: 500;
    transition: background var(--transition-fast);
  }
  .btn-create:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-create:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .filter-box {
    padding: var(--space-8) var(--space-12);
  }
  .filter-input {
    width: 100%;
    font-size: var(--text-sm);
    padding: var(--space-4) var(--space-8);
  }

  .branch-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4) var(--space-8);
  }

  .section-label {
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-mute);
    padding: var(--space-6) var(--space-8);
  }

  .branch-item-row {
    display: flex;
    align-items: center;
  }

  .branch-item {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    width: 100%;
    padding: var(--space-6) var(--space-8);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    color: var(--text-2);
    text-align: left;
    transition: all var(--transition-fast);
  }
  .branch-item:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }
  .branch-item.current {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .branch-item.current .branch-icon {
    color: var(--accent);
  }

  .branch-name {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .current-badge {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--accent);
    color: #fff;
    letter-spacing: 0.05em;
  }

  .stack-base {
    font-size: var(--text-xs);
    font-family: var(--font-mono);
  }

  .btn-delete {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    color: var(--text-mute);
    opacity: 0;
    transition: all var(--transition-fast);
  }
  .branch-item-row:hover .btn-delete {
    opacity: 1;
  }
  .btn-delete:hover {
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
