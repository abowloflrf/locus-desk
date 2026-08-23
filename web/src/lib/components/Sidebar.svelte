<script lang="ts">
  import type { SessionInfo } from '../api/types';
  import type { ProtectedRoute } from '../routes';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    current,
    session,
    onNavigate,
    onWorkspaceViewChange,
    onLogout,
    todoOpen,
    workspaceView,
    blocked = false,
  }: {
    current: ProtectedRoute;
    session: SessionInfo;
    onNavigate: (route: ProtectedRoute) => void;
    onWorkspaceViewChange: (view: 'split' | 'notes' | 'todo') => void;
    onLogout: () => void | Promise<void>;
    todoOpen: boolean;
    workspaceView: 'split' | 'notes' | 'todo';
    blocked?: boolean;
  } = $props();

  const primaryItems: Array<{
    label: string;
    route?: ProtectedRoute;
    icon: IconName;
    disabled?: boolean;
  }> = [
    { icon: 'note', label: 'Workspace', route: 'home' },
    { icon: 'tasks', label: 'Tasks', route: 'tasks' },
    { icon: 'archive', label: 'Archive', route: 'archive' },
    { disabled: true, icon: 'library', label: 'Library' },
    { disabled: true, icon: 'reader', label: 'Reader' },
    { disabled: true, icon: 'chat', label: 'Chat' },
  ];

  function openRoute(event: MouseEvent, route: ProtectedRoute): void {
    event.preventDefault();
    onNavigate(route);
  }

  function activateItem(event: MouseEvent, item: { route?: ProtectedRoute }): void {
    event.preventDefault();
    if (item.route) onNavigate(item.route);
  }

  function focusSearch(): void {
    if (workspaceView === 'todo') onWorkspaceViewChange('notes');
    onNavigate('home');
    setTimeout(() => window.dispatchEvent(new Event('locus:focus-search')), 0);
  }
</script>

<aside class:todo-open={todoOpen} class="sidebar" aria-label="Main navigation" inert={blocked}>
  <a
    aria-label="Locus Desk home"
    class="brand"
    href="/"
    onclick={(event) => openRoute(event, 'home')}
  >
    <span class="brand-mark" aria-hidden="true">L</span>
    <span class="brand-name">Locus Desk</span>
  </a>

  <nav class="primary-nav">
    {#each primaryItems as item}
      {#if item.route && !item.disabled}
        <a
          aria-current={current === item.route ? 'page' : undefined}
          class:active={current === item.route}
          class="nav-item"
          href={item.route === 'home' ? '/' : `/${item.route}`}
          onclick={(event) => activateItem(event, item)}
          title={item.label}
        >
          <Icon name={item.icon} />
          <span>{item.label}</span>
        </a>
      {:else}
        <span
          aria-disabled="true"
          class="nav-item disabled"
          title={`${item.label} is coming later`}
        >
          <Icon name={item.icon} />
          <span>{item.label}</span>
        </span>
      {/if}
      {#if item.route === 'home' && current === 'home'}
        <div aria-label="Workspace view" class="workspace-view-control" role="group">
          <button
            aria-label="Show Notes and Todo"
            aria-pressed={workspaceView === 'split'}
            class:active={workspaceView === 'split'}
            onclick={() => onWorkspaceViewChange('split')}
            type="button">Both</button
          >
          <button
            aria-label="Show Notes only"
            aria-pressed={workspaceView === 'notes'}
            class:active={workspaceView === 'notes'}
            onclick={() => onWorkspaceViewChange('notes')}
            type="button">Notes</button
          >
          <button
            aria-label="Show Todo only"
            aria-pressed={workspaceView === 'todo'}
            class:active={workspaceView === 'todo'}
            onclick={() => onWorkspaceViewChange('todo')}
            type="button">Todo</button
          >
        </div>
      {/if}
    {/each}
  </nav>

  <div class="sidebar-divider"></div>

  <button class="nav-item search-nav" onclick={focusSearch} title="Search notes" type="button">
    <Icon name="search" />
    <span>Search</span>
    <kbd>⌘K</kbd>
  </button>

  <div class="sidebar-footer">
    <div class="sidebar-user">
      <span class="user-avatar" aria-hidden="true"
        >{session.user.username.slice(0, 1).toUpperCase()}</span
      >
      <span>
        <strong>{session.user.username}</strong>
        <small>{session.workspace.role.toLocaleLowerCase()}</small>
      </span>
    </div>
    <button
      aria-label="Sign out"
      class="icon-button"
      onclick={() => void onLogout()}
      title="Sign out"
      type="button"
    >
      <Icon name="logout" />
    </button>
  </div>
</aside>

<style>
  .sidebar {
    position: sticky;
    top: 0;
    z-index: 20;
    display: flex;
    min-width: 0;
    height: 100vh;
    flex-direction: column;
    padding: 28px 18px 20px;
    overflow-y: auto;
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
    scrollbar-width: none;
  }

  .sidebar::-webkit-scrollbar {
    display: none;
  }

  .brand {
    display: flex;
    min-height: 40px;
    gap: 11px;
    align-items: center;
    padding: 0 8px;
    margin-bottom: 35px;
    font-size: 17px;
    font-weight: 680;
    letter-spacing: -0.02em;
  }

  .brand-mark {
    display: grid;
    width: 32px;
    height: 32px;
    flex: none;
    color: var(--color-surface);
    background: var(--color-accent);
    border-radius: 8px;
    font-weight: 700;
    place-items: center;
  }

  .primary-nav {
    display: grid;
    gap: 3px;
  }

  .workspace-view-control {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 2px;
    padding: 3px;
    margin: 1px 8px 7px;
    background: var(--color-surface-muted);
    border-radius: var(--radius-control);
  }

  .workspace-view-control button {
    min-width: 0;
    min-height: 26px;
    padding: 3px 4px;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-radius: 6px;
    font-size: 10px;
  }

  .workspace-view-control button.active {
    color: var(--color-text);
    background: var(--color-surface);
    box-shadow: 0 1px 3px color-mix(in oklch, var(--color-text), transparent 92%);
    font-weight: 620;
  }

  .nav-item {
    display: flex;
    width: 100%;
    min-height: 38px;
    gap: 11px;
    align-items: center;
    padding: 8px 10px;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-radius: 8px;
    font-size: 13px;
    text-align: left;
    transition:
      color 150ms ease,
      background-color 150ms ease;
  }

  .nav-item:hover:not(.disabled),
  .nav-item.active {
    color: var(--color-text);
    background: var(--color-surface-muted);
  }

  .nav-item.active {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
    font-weight: 620;
  }

  .nav-item.disabled {
    opacity: 0.58;
    cursor: default;
  }

  .sidebar-divider {
    height: 1px;
    margin: 20px 10px;
    background: var(--color-border);
  }

  .search-nav kbd {
    margin-left: auto;
  }

  .sidebar-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 2px 0;
    margin-top: auto;
    border-top: 1px solid var(--color-border);
  }

  .sidebar-user {
    display: flex;
    min-width: 0;
    gap: 9px;
    align-items: center;
  }

  .sidebar-user > span:last-child {
    display: grid;
    min-width: 0;
  }

  .sidebar-user strong {
    overflow: hidden;
    font-size: 12px;
    font-weight: 620;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sidebar-user small {
    color: var(--color-text-muted);
    font-size: 11px;
    text-transform: capitalize;
  }

  .user-avatar {
    display: grid;
    width: 30px;
    height: 30px;
    flex: none;
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
    border-radius: 50%;
    font-size: 12px;
    font-weight: 700;
    place-items: center;
  }

  @media (max-width: 1199px) {
    .sidebar {
      padding: 20px 9px 16px;
    }

    .brand {
      justify-content: center;
      padding: 0;
      margin-bottom: 29px;
    }

    .brand-name,
    .nav-item > span,
    .search-nav kbd,
    .sidebar-user,
    .workspace-view-control {
      display: none;
    }

    .nav-item {
      justify-content: center;
      padding-inline: 0;
    }

    .sidebar-divider {
      margin-inline: 4px;
    }

    .sidebar-footer {
      justify-content: center;
    }
  }

  @media (max-width: 767px) {
    .sidebar {
      position: fixed;
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      z-index: 60;
      display: block;
      width: 100%;
      height: 64px;
      padding: 6px 12px calc(6px + env(safe-area-inset-bottom));
      overflow: visible;
      border-top: 1px solid var(--color-border);
      border-right: 0;
    }

    .sidebar.todo-open {
      z-index: 40;
    }

    .brand,
    .sidebar-divider,
    .search-nav,
    .sidebar-footer,
    .primary-nav .nav-item.disabled {
      display: none;
    }

    .primary-nav {
      grid-template-columns: repeat(3, 1fr);
      gap: 4px;
    }

    .primary-nav .nav-item {
      display: flex;
      min-height: 50px;
      flex-direction: column;
      gap: 2px;
      padding: 4px 2px;
      font-size: 10px;
    }

    .primary-nav .nav-item > span {
      display: inline;
    }
  }
</style>
