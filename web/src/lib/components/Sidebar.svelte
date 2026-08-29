<script lang="ts">
  import { tick } from 'svelte';

  import type { SessionInfo } from '../api/types';
  import type { ProtectedRoute } from '../routes';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    current,
    session,
    onNavigate,
    onLogout,
    todoOpen,
    blocked = false,
  }: {
    current: ProtectedRoute;
    session: SessionInfo;
    onNavigate: (route: ProtectedRoute) => void;
    onLogout: () => void | Promise<void>;
    todoOpen: boolean;
    blocked?: boolean;
  } = $props();

  const primaryItems: Array<{
    label: string;
    route: ProtectedRoute;
    icon: IconName;
  }> = [
    { icon: 'today', label: 'Workspace', route: 'home' },
    { icon: 'note', label: 'Memos', route: 'notes' },
    { icon: 'library', label: 'Library', route: 'library' },
    { icon: 'tasks', label: 'Tasks', route: 'tasks' },
    { icon: 'archive', label: 'Archive', route: 'archive' },
  ];

  let moreOpen = $state(false);
  let moreButton = $state<HTMLButtonElement>();
  let archiveMenuItem = $state<HTMLAnchorElement>();

  $effect(() => {
    current;
    moreOpen = false;
  });

  $effect(() => {
    if (blocked) moreOpen = false;
  });

  function openRoute(event: MouseEvent, route: ProtectedRoute): void {
    event.preventDefault();
    moreOpen = false;
    onNavigate(route);
  }

  function focusSearch(): void {
    onNavigate('notes');
    setTimeout(() => window.dispatchEvent(new Event('locus:focus-search')), 0);
  }

  async function toggleMore(): Promise<void> {
    if (moreOpen) {
      moreOpen = false;
      await tick();
      moreButton?.focus();
      return;
    }
    moreOpen = true;
    await tick();
    archiveMenuItem?.focus();
  }

  async function closeMore(): Promise<void> {
    if (!moreOpen) return;
    moreOpen = false;
    await tick();
    moreButton?.focus();
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (moreOpen && event.key === 'Escape') {
      event.preventDefault();
      void closeMore();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

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

  {#if moreOpen}
    <button
      aria-label="Close more navigation"
      class="mobile-more-backdrop"
      onclick={() => void closeMore()}
      tabindex="-1"
      type="button"
    ></button>
    <div aria-label="More navigation" class="mobile-more-menu" role="menu">
      <p>More</p>
      <a
        aria-current={current === 'archive' ? 'page' : undefined}
        bind:this={archiveMenuItem}
        href="/archive"
        onclick={(event) => openRoute(event, 'archive')}
        role="menuitem"
      >
        <Icon name="archive" />
        <span>Archive</span>
      </a>
    </div>
  {/if}

  <nav class="primary-nav">
    {#each primaryItems as item}
      <a
        aria-current={current === item.route ? 'page' : undefined}
        class:active={current === item.route}
        class:mobile-overflow-item={item.route === 'archive'}
        class="nav-item"
        href={item.route === 'home' ? '/' : `/${item.route}`}
        onclick={(event) => openRoute(event, item.route)}
        title={item.label}
      >
        <Icon name={item.icon} />
        <span>{item.label}</span>
      </a>
    {/each}
    <button
      aria-expanded={moreOpen}
      aria-haspopup="menu"
      aria-label="More navigation"
      bind:this={moreButton}
      class:active={current === 'archive'}
      class="nav-item mobile-more-trigger"
      onclick={() => void toggleMore()}
      type="button"
    >
      <Icon name="more" />
      <span>More</span>
    </button>
  </nav>

  <div class="sidebar-divider"></div>

  <button class="nav-item search-nav" onclick={focusSearch} title="Search memos" type="button">
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
    height: 100dvh;
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

  .mobile-more-trigger,
  .mobile-more-menu,
  .mobile-more-backdrop {
    display: none;
  }

  .nav-item {
    position: relative;
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

  .nav-item:hover,
  .nav-item.active {
    color: var(--color-text);
    background: var(--color-surface-muted);
  }

  .nav-item.active {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
    font-weight: 620;
  }

  .nav-item.active::before {
    position: absolute;
    top: 10px;
    bottom: 10px;
    left: 0;
    width: 2px;
    background: var(--color-accent);
    border-radius: 2px;
    content: '';
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
    .sidebar-user {
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
      height: calc(64px + env(safe-area-inset-bottom));
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
    .sidebar-footer {
      display: none;
    }

    .primary-nav {
      position: relative;
      z-index: 2;
      grid-template-columns: repeat(5, 1fr);
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

    .primary-nav .mobile-overflow-item {
      display: none;
    }

    .primary-nav .mobile-more-trigger {
      display: flex;
    }

    .mobile-more-backdrop {
      position: fixed;
      inset: 0 0 calc(64px + env(safe-area-inset-bottom));
      z-index: 1;
      display: block;
      padding: 0;
      background: color-mix(in oklch, var(--color-text), transparent 82%);
      border: 0;
    }

    .mobile-more-menu {
      position: absolute;
      right: calc(12px + env(safe-area-inset-right));
      bottom: calc(100% + 8px);
      z-index: 3;
      display: grid;
      width: min(240px, calc(100vw - 24px));
      padding: 8px;
      background: var(--color-surface);
      border: 1px solid var(--color-border);
      border-radius: var(--radius-surface);
      box-shadow: var(--shadow-floating);
    }

    .mobile-more-menu p {
      padding: 6px 9px 7px;
      margin: 0;
      color: var(--color-text-muted);
      font-size: 10px;
      font-weight: 650;
      letter-spacing: 0.04em;
      text-transform: uppercase;
    }

    .mobile-more-menu a {
      display: flex;
      min-height: 44px;
      gap: 10px;
      align-items: center;
      padding: 8px 10px;
      border-radius: var(--radius-control);
      font-size: 13px;
      font-weight: 600;
    }

    .mobile-more-menu a:hover,
    .mobile-more-menu a[aria-current='page'] {
      color: var(--color-accent-hover);
      background: var(--color-accent-soft);
    }
  }
</style>
