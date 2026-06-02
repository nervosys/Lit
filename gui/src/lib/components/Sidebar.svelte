<script lang="ts">
  import type { SidebarView, Theme } from "../types";

  let {
    activeView = $bindable("workspace"),
    theme,
    onToggleTheme,
  }: {
    activeView: SidebarView;
    theme: Theme;
    onToggleTheme: () => void;
  } = $props();

  const navItems: { id: SidebarView; label: string; icon: string }[] = [
    { id: "workspace", label: "Workspace", icon: "workspace" },
    { id: "branches", label: "Branches", icon: "branch" },
    { id: "stacks", label: "Stacks", icon: "stack" },
    { id: "stash", label: "Stash", icon: "stash" },
    { id: "history", label: "History", icon: "history" },
  ];
</script>

<nav class="sidebar">
  <div class="nav-top">
    <div class="sidebar-logo" title="Lit VCS">
      <svg width="24" height="24" viewBox="0 0 32 32" fill="none">
        <rect width="32" height="32" rx="6" fill="var(--accent)"/>
        <path d="M10 8v16M10 8l6 4-6 4M18 16h4M18 20h4M18 24h4" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </div>

    {#each navItems as item}
      <button
        class="nav-btn"
        class:active={activeView === item.id}
        title={item.label}
        onclick={() => (activeView = item.id)}
      >
        {#if item.icon === "workspace"}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="7" height="7" rx="1"/>
            <rect x="14" y="3" width="7" height="7" rx="1"/>
            <rect x="3" y="14" width="7" height="7" rx="1"/>
            <rect x="14" y="14" width="7" height="7" rx="1"/>
          </svg>
        {:else if item.icon === "branch"}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="6" y1="3" x2="6" y2="15"/>
            <circle cx="18" cy="6" r="3"/>
            <circle cx="6" cy="18" r="3"/>
            <path d="M18 9a9 9 0 0 1-9 9"/>
          </svg>
        {:else if item.icon === "stack"}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 2L2 7l10 5 10-5-10-5z"/>
            <path d="M2 17l10 5 10-5"/>
            <path d="M2 12l10 5 10-5"/>
          </svg>
        {:else if item.icon === "stash"}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
            <line x1="3.3" y1="7" x2="12" y2="12"/>
            <line x1="12" y1="22" x2="12" y2="12"/>
            <line x1="20.7" y1="7" x2="12" y2="12"/>
          </svg>
        {:else if item.icon === "history"}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12 6 12 12 16 14"/>
          </svg>
        {/if}
      </button>
    {/each}
  </div>

  <div class="nav-bottom">
    <button class="nav-btn" title="Toggle theme" onclick={onToggleTheme}>
      {#if theme === "dark"}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="5"/>
          <line x1="12" y1="1" x2="12" y2="3"/>
          <line x1="12" y1="21" x2="12" y2="23"/>
          <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/>
          <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
          <line x1="1" y1="12" x2="3" y2="12"/>
          <line x1="21" y1="12" x2="23" y2="12"/>
          <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/>
          <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
        </svg>
      {:else}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
        </svg>
      {/if}
    </button>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    width: var(--sidebar-width);
    background: var(--bg-1);
    border-right: 1px solid var(--border-1);
    flex-shrink: 0;
    padding: var(--space-8) 0;
  }

  .nav-top, .nav-bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-4);
  }

  .sidebar-logo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    margin-bottom: var(--space-12);
  }

  .nav-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-md);
    color: var(--text-3);
    transition: all var(--transition-fast);
    position: relative;
  }
  .nav-btn:hover {
    background: var(--bg-hover);
    color: var(--text-1);
  }
  .nav-btn.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .nav-btn.active::before {
    content: "";
    position: absolute;
    left: -8px;
    width: 3px;
    height: 16px;
    background: var(--accent);
    border-radius: 0 2px 2px 0;
  }
</style>
