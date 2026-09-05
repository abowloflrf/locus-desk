<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import BookOpen from '@lucide/svelte/icons/book-open';
  import FileText from '@lucide/svelte/icons/file-text';
  import LayoutDashboard from '@lucide/svelte/icons/layout-dashboard';
  import ListTodo from '@lucide/svelte/icons/list-todo';
  import LogOut from '@lucide/svelte/icons/log-out';
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
  import Search from '@lucide/svelte/icons/search';
  import type { Component } from 'svelte';

  import type { SessionInfo } from '../api/types';
  import type { ProtectedRoute } from '../routes';
  import { Button } from './ui/button';
  import * as Kbd from './ui/kbd';

  let {
    current,
    session,
    onNavigate,
    onLogout,
    onToggleCollapsed,
    todoOpen,
    blocked = false,
    collapsed = false,
    mobile = false,
  }: {
    current: ProtectedRoute;
    session: SessionInfo;
    onNavigate: (route: ProtectedRoute) => void;
    onLogout: () => void | Promise<void>;
    onToggleCollapsed: () => void;
    todoOpen: boolean;
    blocked?: boolean;
    collapsed?: boolean;
    mobile?: boolean;
  } = $props();

  const primaryItems: Array<{
    label: string;
    route: ProtectedRoute;
    icon: Component;
  }> = [
    { icon: LayoutDashboard, label: 'Workspace', route: 'home' },
    { icon: FileText, label: 'Memos', route: 'notes' },
    { icon: ListTodo, label: 'Tasks', route: 'tasks' },
    { icon: BookOpen, label: 'Library', route: 'library' },
    { icon: Archive, label: 'Archive', route: 'archive' },
  ];

  function openRoute(event: MouseEvent, route: ProtectedRoute): void {
    event.preventDefault();
    onNavigate(route);
  }

  function focusSearch(): void {
    onNavigate('notes');
    setTimeout(() => window.dispatchEvent(new Event('locus:focus-search')), 0);
  }
</script>

<aside
  class:collapsed
  class:todo-open={todoOpen}
  class="sidebar"
  aria-label="Main navigation"
  inert={blocked}
>
  <div class="sidebar-header">
    <a
      aria-label="Locus Desk home"
      class="brand"
      href="/"
      onclick={(event) => openRoute(event, 'home')}
    >
      <span class="brand-mark" aria-hidden="true">L</span>
      <span class="brand-name">Locus Desk</span>
    </a>
    <Button
      aria-expanded={!collapsed}
      aria-label={collapsed ? 'Expand navigation' : 'Collapse navigation'}
      class="sidebar-toggle"
      onclick={onToggleCollapsed}
      size="icon"
      title={collapsed ? 'Expand navigation' : 'Collapse navigation'}
      variant="ghost"
    >
      {#if collapsed}<PanelLeftOpen />{:else}<PanelLeftClose />{/if}
    </Button>
  </div>

  <Button
    aria-label="Search memos"
    class="sidebar-search w-full justify-start max-[1199px]:justify-center max-[767px]:hidden"
    onclick={focusSearch}
    title="Search memos"
    variant="ghost"
  >
    <Search aria-hidden="true" />
    <span class="sidebar-search-label max-[1199px]:hidden">Search</span>
    <Kbd.Root class="sidebar-search-kbd ml-auto max-[1199px]:hidden">⌘K</Kbd.Root>
  </Button>

  <nav class="primary-nav">
    {#each primaryItems as item}
      {#if !mobile || (item.route !== 'home' && item.route !== 'archive')}
        {@const NavIcon = item.icon}
        <a
          aria-current={current === item.route ? 'page' : undefined}
          class:active={current === item.route}
          class="nav-item"
          href={item.route === 'home' ? '/' : `/${item.route}`}
          onclick={(event) => openRoute(event, item.route)}
          title={item.label}
        >
          <NavIcon aria-hidden="true" />
          <span>{item.label}</span>
        </a>
      {/if}
    {/each}
  </nav>

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
    <Button
      aria-label="Sign out"
      onclick={() => void onLogout()}
      size="icon"
      title="Sign out"
      variant="ghost"
    >
      <LogOut />
    </Button>
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
    padding: 12px;
    overflow-y: auto;
    background: var(--sidebar);
    border-right: 0;
    scrollbar-width: none;
  }

  .sidebar::-webkit-scrollbar {
    display: none;
  }

  .brand {
    display: flex;
    min-height: 40px;
    gap: 8px;
    align-items: center;
    padding: 0;
    font-size: 13px;
    font-weight: 680;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }

  .sidebar-header {
    display: flex;
    gap: 4px;
    min-height: 36px;
    align-items: center;
    justify-content: space-between;
    padding-left: 8px;
    margin-bottom: 16px;
  }

  .sidebar :global(.sidebar-toggle) {
    flex: none;
  }

  .brand-mark {
    display: grid;
    width: 24px;
    height: 24px;
    flex: none;
    color: var(--card);
    background: var(--primary);
    border-radius: 8px;
    font-weight: 700;
    place-items: center;
  }

  .primary-nav {
    display: grid;
    gap: 4px;
  }

  .nav-item {
    position: relative;
    display: flex;
    width: 100%;
    min-height: 36px;
    gap: 8px;
    align-items: center;
    padding: 8px;
    line-height: 20px;
    color: var(--muted-foreground);
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
    color: var(--foreground);
    background: var(--muted);
  }

  .nav-item.active {
    color: var(--primary);
    background: var(--accent);
    font-weight: 620;
  }

  .nav-item :global(svg) {
    width: 16px;
    height: 16px;
    flex: none;
    stroke-width: 1.75;
  }

  .sidebar :global(.sidebar-search) {
    height: 36px;
    min-height: 36px;
    gap: 8px;
    padding: 8px;
    margin-bottom: 12px;
    color: var(--muted-foreground);
    font-size: 13px;
    font-weight: 400;
  }

  .sidebar :global(.sidebar-search svg) {
    stroke-width: 1.75;
  }

  .sidebar-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 0 0;
    margin-top: auto;
  }

  .sidebar-user {
    display: flex;
    min-width: 0;
    gap: 8px;
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
    color: var(--muted-foreground);
    font-size: 11px;
    text-transform: capitalize;
  }

  .sidebar.collapsed {
    padding: 12px 10px;
  }

  .sidebar.collapsed .sidebar-header {
    flex-direction: column;
    gap: 4px;
    padding-left: 0;
    margin-bottom: 12px;
  }

  .sidebar.collapsed .brand {
    justify-content: center;
    padding: 0;
  }

  .sidebar.collapsed .brand-name,
  .sidebar.collapsed .nav-item > span,
  .sidebar.collapsed .sidebar-user,
  .sidebar.collapsed :global(.sidebar-search-label),
  .sidebar.collapsed :global(.sidebar-search-kbd) {
    display: none;
  }

  .sidebar.collapsed .nav-item,
  .sidebar.collapsed > :global(button) {
    justify-content: center;
    padding-inline: 0;
  }

  .sidebar.collapsed .sidebar-footer {
    justify-content: center;
  }

  .user-avatar {
    display: grid;
    width: 24px;
    height: 24px;
    flex: none;
    color: var(--primary);
    background: var(--accent);
    border-radius: 50%;
    font-size: 12px;
    font-weight: 700;
    place-items: center;
  }

  @media (max-width: 1199px) {
    .sidebar {
      padding: 12px 10px;
    }

    .brand {
      justify-content: center;
      padding: 0;
      margin-bottom: 0;
    }

    .sidebar-header {
      padding-left: 0;
      margin-bottom: 16px;
      justify-content: center;
    }

    .sidebar :global(.sidebar-toggle) {
      display: none;
    }

    .brand-name,
    .nav-item > span,
    .sidebar-user {
      display: none;
    }

    .nav-item {
      justify-content: center;
      padding-inline: 0;
    }

    .sidebar-footer {
      justify-content: center;
    }
  }

  @media (pointer: coarse) and (min-width: 768px) {
    .nav-item,
    .sidebar :global(.sidebar-search) {
      min-height: 44px;
    }
  }

  @media (max-width: 767px) {
    .sidebar.collapsed,
    .sidebar {
      position: fixed;
      top: auto;
      right: 0;
      bottom: 0;
      left: 0;
      z-index: 40;
      display: block;
      width: 100%;
      height: calc(
        var(--mobile-navigation-content-height, 64px) +
          var(--mobile-navigation-safe-space, max(8px, env(safe-area-inset-bottom)))
      );
      padding: 0 calc(12px + env(safe-area-inset-right))
        var(--mobile-navigation-safe-space, max(8px, env(safe-area-inset-bottom)))
        calc(12px + env(safe-area-inset-left));
      overflow: visible;
      background: var(--sidebar);
      border-top: 1px solid var(--border);
    }

    .sidebar-header,
    .sidebar-footer {
      display: none;
    }

    .primary-nav {
      position: relative;
      z-index: 2;
      width: min(100%, 440px);
      height: var(--mobile-navigation-content-height, 64px);
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 2px;
      padding: 5px;
      margin-inline: auto;
      overflow: hidden;
      background: var(--sidebar);
      border: 0;
      border-radius: 0;
    }

    .primary-nav .nav-item {
      display: flex;
      min-height: 52px;
      flex-direction: column;
      gap: 4px;
      padding: 6px 3px 5px;
      color: var(--muted-foreground);
      background: transparent;
      border: 0;
      border-radius: 8px;
      font-size: 12px;
      font-weight: 520;
      line-height: 1;
      transition:
        color 180ms ease,
        background-color 180ms ease,
        transform 160ms cubic-bezier(0.2, 0.8, 0.2, 1);
    }

    .primary-nav .nav-item > span {
      display: inline;
    }

    .primary-nav .nav-item :global(svg) {
      width: 20px;
      height: 20px;
      stroke-width: 1.8;
    }

    @media (hover: hover) {
      .primary-nav .nav-item:hover {
        color: var(--foreground);
        background: color-mix(in oklab, var(--muted) 48%, transparent);
      }
    }

    .primary-nav .nav-item.active,
    .primary-nav .nav-item.active:hover {
      color: var(--foreground);
      background: var(--muted);
      font-weight: 620;
    }

    .primary-nav .nav-item:active {
      transform: scale(0.97);
    }

    .primary-nav .nav-item:focus-visible {
      outline: 2px solid var(--foreground);
      outline-offset: -3px;
    }
  }
</style>
