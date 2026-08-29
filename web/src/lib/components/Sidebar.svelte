<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import BookOpen from '@lucide/svelte/icons/book-open';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
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
  import * as DropdownMenu from './ui/dropdown-menu';
  import * as Kbd from './ui/kbd';
  import { Separator } from './ui/separator';

  let {
    current,
    session,
    onNavigate,
    onLogout,
    onToggleCollapsed,
    todoOpen,
    blocked = false,
    collapsed = false,
  }: {
    current: ProtectedRoute;
    session: SessionInfo;
    onNavigate: (route: ProtectedRoute) => void;
    onLogout: () => void | Promise<void>;
    onToggleCollapsed: () => void;
    todoOpen: boolean;
    blocked?: boolean;
    collapsed?: boolean;
  } = $props();

  const primaryItems: Array<{
    label: string;
    route: ProtectedRoute;
    icon: Component;
  }> = [
    { icon: LayoutDashboard, label: 'Workspace', route: 'home' },
    { icon: FileText, label: 'Memos', route: 'notes' },
    { icon: BookOpen, label: 'Library', route: 'library' },
    { icon: ListTodo, label: 'Tasks', route: 'tasks' },
    { icon: Archive, label: 'Archive', route: 'archive' },
  ];

  let moreOpen = $state(false);
  let moreButton = $state<HTMLButtonElement | null>(null);

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

  function handleMoreOpenChange(open: boolean): void {
    moreOpen = open;
    if (!open) requestAnimationFrame(() => moreButton?.focus());
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

  <nav class="primary-nav">
    {#each primaryItems as item}
      {@const NavIcon = item.icon}
      <a
        aria-current={current === item.route ? 'page' : undefined}
        class:active={current === item.route}
        class:mobile-overflow-item={item.route === 'archive'}
        class="nav-item"
        href={item.route === 'home' ? '/' : `/${item.route}`}
        onclick={(event) => openRoute(event, item.route)}
        title={item.label}
      >
        <NavIcon />
        <span>{item.label}</span>
      </a>
    {/each}
    <DropdownMenu.Root onOpenChange={handleMoreOpenChange} open={moreOpen}>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            aria-label="More navigation"
            bind:ref={moreButton}
            aria-current={current === 'archive' ? 'page' : undefined}
            class="mobile-more-trigger hidden max-[767px]:flex"
            data-current={current === 'archive'}
            size="mobile-nav"
            variant="mobile-nav"
          >
            <Ellipsis data-icon="inline-start" />
            <span>More</span>
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      {#if moreOpen}
        <DropdownMenu.Content align="end" class="w-56" forceMount side="top" sideOffset={10}>
          <DropdownMenu.Label>More</DropdownMenu.Label>
          <DropdownMenu.Group>
            <DropdownMenu.Item aria-label="Archive" onclick={() => onNavigate('archive')}>
              <Archive />
              Archive
            </DropdownMenu.Item>
          </DropdownMenu.Group>
        </DropdownMenu.Content>
      {/if}
    </DropdownMenu.Root>
  </nav>

  <Separator class="sidebar-divider" />

  <Button
    aria-label="Search memos"
    class="h-auto w-full justify-start gap-[11px] px-[10px] py-2 text-muted-foreground max-[1199px]:justify-center max-[1199px]:px-0 max-[767px]:hidden"
    onclick={focusSearch}
    title="Search memos"
    variant="ghost"
  >
    <Search />
    <span class="sidebar-search-label max-[1199px]:hidden">Search</span>
    <Kbd.Root class="sidebar-search-kbd ml-auto max-[1199px]:hidden">⌘K</Kbd.Root>
  </Button>

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
    padding: 28px 18px 20px;
    overflow-y: auto;
    background: var(--card);
    border-right: 1px solid var(--border);
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
    font-size: 17px;
    font-weight: 680;
    letter-spacing: -0.02em;
  }

  .sidebar-header {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 35px;
  }

  .sidebar :global(.sidebar-toggle) {
    flex: none;
  }

  .brand-mark {
    display: grid;
    width: 32px;
    height: 32px;
    flex: none;
    color: var(--card);
    background: var(--primary);
    border-radius: 8px;
    font-weight: 700;
    place-items: center;
  }

  .primary-nav {
    display: grid;
    gap: 3px;
  }

  .nav-item {
    position: relative;
    display: flex;
    width: 100%;
    min-height: 38px;
    gap: 11px;
    align-items: center;
    padding: 8px 10px;
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

  .nav-item.active::before {
    position: absolute;
    top: 10px;
    bottom: 10px;
    left: 0;
    width: 2px;
    background: var(--primary);
    border-radius: 2px;
    content: '';
  }

  .sidebar :global(.sidebar-divider) {
    margin: 20px 10px;
  }

  .sidebar-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 2px 0;
    margin-top: auto;
    border-top: 1px solid var(--border);
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
    color: var(--muted-foreground);
    font-size: 11px;
    text-transform: capitalize;
  }

  .sidebar.collapsed {
    padding: 20px 9px 16px;
  }

  .sidebar.collapsed .sidebar-header {
    flex-direction: column;
    gap: 10px;
    margin-bottom: 25px;
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

  .sidebar.collapsed :global(.sidebar-divider) {
    margin-inline: 4px;
  }

  .sidebar.collapsed .sidebar-footer {
    justify-content: center;
  }

  .user-avatar {
    display: grid;
    width: 30px;
    height: 30px;
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
      padding: 20px 9px 16px;
    }

    .brand {
      justify-content: center;
      padding: 0;
      margin-bottom: 29px;
    }

    .sidebar-header {
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

    .sidebar :global(.sidebar-divider) {
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
      height: calc(
        var(--mobile-navigation-content-height, 58px) +
          var(--mobile-navigation-safe-space, max(4px, calc(env(safe-area-inset-bottom) - 4px)))
      );
      padding: 0 calc(8px + env(safe-area-inset-right))
        var(--mobile-navigation-safe-space, max(4px, calc(env(safe-area-inset-bottom) - 4px)))
        calc(8px + env(safe-area-inset-left));
      overflow: visible;
      background: var(--background);
      border-top: 1px solid var(--border);
      border-right: 0;
    }

    .sidebar.todo-open {
      z-index: 40;
    }

    .sidebar-header,
    .sidebar :global(.sidebar-divider),
    .sidebar-footer {
      display: none;
    }

    .primary-nav {
      position: relative;
      z-index: 2;
      height: var(--mobile-navigation-content-height, 58px);
      grid-template-columns: repeat(5, 1fr);
      gap: 0;
    }

    .primary-nav .nav-item {
      display: flex;
      min-height: var(--mobile-navigation-content-height, 58px);
      flex-direction: column;
      gap: 4px;
      padding: 8px 2px 6px;
      color: var(--muted-foreground);
      background: transparent;
      border-radius: 0;
      font-size: 11px;
      font-weight: 450;
      line-height: 1;
    }

    .primary-nav .nav-item > span {
      display: inline;
    }

    .primary-nav .nav-item :global(svg) {
      width: 20px;
      height: 20px;
      stroke-width: 1.9;
    }

    .primary-nav .nav-item:hover {
      color: var(--foreground);
      background: transparent;
    }

    .primary-nav .nav-item.active,
    .primary-nav .nav-item.active:hover {
      color: var(--primary);
      background: transparent;
      font-weight: 620;
    }

    .primary-nav .nav-item.active::before,
    .primary-nav :global(.mobile-more-trigger[data-current='true'])::before {
      position: absolute;
      top: -1px;
      right: auto;
      bottom: auto;
      left: 50%;
      width: 24px;
      height: 2px;
      background: var(--primary);
      border-radius: 0 0 2px 2px;
      content: '';
      transform: translateX(-50%);
    }

    .primary-nav :global(.mobile-more-trigger) {
      position: relative;
    }

    .primary-nav .nav-item:active,
    .primary-nav :global(.mobile-more-trigger:active) {
      transform: translateY(1px);
    }

    .primary-nav .nav-item:focus-visible {
      outline: 2px solid var(--ring);
      outline-offset: -4px;
    }

    .primary-nav .mobile-overflow-item {
      display: none;
    }
  }
</style>
