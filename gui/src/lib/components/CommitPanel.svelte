<script lang="ts">
  import type { StatusData } from "../types";
  import { createCommit, amendCommit, errMsg } from "../tauri";

  let {
    status,
    onRefresh,
    onError,
  }: {
    status: StatusData | null;
    onRefresh: () => void;
    onError?: (message: string) => void;
  } = $props();

  let message = $state("");
  let isAmend = $state(false);
  let committing = $state(false);

  let canCommit = $derived(
    message.trim().length > 0 && (status?.staged?.length ?? 0) > 0 && !committing
  );

  let summary = $derived(message.split("\n")[0] ?? "");
  let charCount = $derived(summary.length);
  let charColor = $derived(
    charCount > 72 ? "var(--danger)" : charCount > 50 ? "var(--warn)" : "var(--text-mute)"
  );

  async function handleCommit() {
    if (!canCommit) return;
    committing = true;
    try {
      if (isAmend) {
        await amendCommit(message.trim());
      } else {
        await createCommit(message.trim());
      }
      message = "";
      isAmend = false;
      onRefresh();
    } catch (e) {
      onError?.(errMsg(e));
    } finally {
      committing = false;
    }
  }
</script>

<div class="commit-panel">
  <div class="panel-header">
    <span class="panel-title">Commit</span>
    <label class="amend-toggle">
      <input type="checkbox" bind:checked={isAmend} />
      <span>Amend</span>
    </label>
  </div>

  <div class="commit-form">
    <div class="message-area">
      <textarea
        class="commit-message"
        placeholder="Commit message..."
        bind:value={message}
        rows={4}
        onkeydown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            handleCommit();
          }
        }}
      ></textarea>
      <span class="char-count" style="color: {charColor}">{charCount}</span>
    </div>

    <button
      class="btn-commit"
      disabled={!canCommit}
      onclick={handleCommit}
    >
      {#if committing}
        Committing...
      {:else if isAmend}
        Amend Commit
      {:else}
        Commit {#if status?.staged?.length}({status.staged.length} file{status.staged.length !== 1 ? "s" : ""}){/if}
      {/if}
    </button>
  </div>

  <div class="commit-info">
    {#if status}
      <div class="info-row">
        <span class="info-label">Staged</span>
        <span class="info-value">{status.staged.length}</span>
      </div>
      <div class="info-row">
        <span class="info-label">Modified</span>
        <span class="info-value">{status.modified.length}</span>
      </div>
      <div class="info-row">
        <span class="info-label">Untracked</span>
        <span class="info-value">{status.untracked.length}</span>
      </div>
    {/if}
    {#if status?.head}
      <div class="info-row" style="margin-top: var(--space-8);">
        <span class="info-label">HEAD</span>
        <span class="info-value mono" title={status.head}>{status.head.slice(0, 12)}</span>
      </div>
    {/if}
  </div>

  <div class="shortcuts">
    <span class="shortcut-hint">Ctrl+Enter to commit</span>
  </div>
</div>

<style>
  .commit-panel {
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

  .amend-toggle {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    font-size: var(--text-xs);
    color: var(--text-3);
    cursor: pointer;
  }
  .amend-toggle input {
    width: 14px;
    height: 14px;
    accent-color: var(--accent);
  }

  .commit-form {
    padding: var(--space-12);
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .message-area {
    position: relative;
  }

  .commit-message {
    width: 100%;
    resize: vertical;
    min-height: 80px;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: 1.5;
    padding: var(--space-8) var(--space-10);
    background: var(--bg-2);
    border: 1px solid var(--border-1);
    border-radius: var(--radius-md);
    color: var(--text-1);
  }
  .commit-message:focus {
    border-color: var(--accent);
    outline: none;
  }
  .commit-message::placeholder {
    color: var(--text-mute);
  }

  .char-count {
    position: absolute;
    bottom: var(--space-6);
    right: var(--space-8);
    font-size: 10px;
    font-family: var(--font-mono);
  }

  .btn-commit {
    padding: var(--space-8) var(--space-16);
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-weight: 600;
    transition: all var(--transition-fast);
  }
  .btn-commit:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-commit:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .commit-info {
    padding: var(--space-12);
    border-top: 1px solid var(--border-1);
    flex: 1;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    padding: var(--space-4) 0;
    font-size: var(--text-xs);
  }
  .info-label {
    color: var(--text-3);
  }
  .info-value {
    color: var(--text-2);
    font-weight: 500;
  }

  .shortcuts {
    padding: var(--space-8) var(--space-12);
    border-top: 1px solid var(--border-1);
    flex-shrink: 0;
  }
  .shortcut-hint {
    font-size: 10px;
    color: var(--text-mute);
  }
</style>
