<script lang="ts">
  import Columns2 from '@lucide/svelte/icons/columns-2';
  import ListChecks from '@lucide/svelte/icons/list-checks';
  import ListTodo from '@lucide/svelte/icons/list-todo';
  import LogOut from '@lucide/svelte/icons/log-out';
  import PanelLeft from '@lucide/svelte/icons/panel-left';
  import { onMount, tick, type Snippet } from 'svelte';

  import type { SessionInfo } from '../api/types';
  import type { ProtectedRoute } from '../routes';
  import Sidebar from './Sidebar.svelte';
  import TaskBoard from './TaskBoard.svelte';
  import * as Alert from './ui/alert';
  import { Button } from './ui/button';
  import * as Sheet from './ui/sheet';
  import * as ToggleGroup from './ui/toggle-group';

  let {
    current,
    immersive = false,
    session,
    refreshToken = 0,
    children,
    onNavigate,
    onLogout,
    notice = null,
    onDismissNotice,
  }: {
    current: ProtectedRoute;
    immersive?: boolean;
    session: SessionInfo;
    refreshToken?: number;
    children: Snippet;
    onNavigate: (route: ProtectedRoute, replace?: boolean) => void;
    onLogout: () => void | Promise<void>;
    notice?: string | null;
    onDismissNotice: () => void;
  } = $props();

  type WorkspaceLayout = 'notes' | 'split' | 'todo';

  let compact = $state(false);
  let mobile = $state(false);
  let sidebarCollapsed = $state(false);
  let todoOpen = $state(false);
  let topbarHidden = $state(false);
  let workspaceLayout = $state<WorkspaceLayout>('split');
  let todoButton = $state<HTMLButtonElement | null>(null);
  let lastScrollTop = 0;
  let scrollDelta = 0;

  onMount(() => {
    sidebarCollapsed = window.localStorage.getItem('locus:sidebar-collapsed') === 'true';
    const compactMedia = window.matchMedia('(max-width: 1199px)');
    const mobileMedia = window.matchMedia('(max-width: 767px)');
    const updateCompact = () => {
      compact = compactMedia.matches;
      if (!compact) todoOpen = false;
    };
    const updateMobile = () => {
      mobile = mobileMedia.matches;
      if (!mobile) topbarHidden = false;
    };
    updateCompact();
    updateMobile();
    compactMedia.addEventListener('change', updateCompact);
    mobileMedia.addEventListener('change', updateMobile);
    return () => {
      compactMedia.removeEventListener('change', updateCompact);
      mobileMedia.removeEventListener('change', updateMobile);
    };
  });

  $effect(() => {
    current;
    todoOpen = false;
    topbarHidden = false;
    lastScrollTop = 0;
    scrollDelta = 0;
    if (mobile && current === 'home') queueMicrotask(() => onNavigate('notes', true));
  });

  async function openTodo(): Promise<void> {
    if (current !== 'home') {
      onNavigate('home');
      await tick();
    }
    todoOpen = true;
  }

  function handleTodoOpenChange(open: boolean): void {
    todoOpen = open;
    if (!open) requestAnimationFrame(() => todoButton?.focus());
  }

  function toggleSidebar(): void {
    sidebarCollapsed = !sidebarCollapsed;
    window.localStorage.setItem('locus:sidebar-collapsed', String(sidebarCollapsed));
  }

  function handleWorkspaceScroll(event: Event): void {
    const scrollTop = (event.currentTarget as HTMLElement).scrollTop;
    if (!mobile) {
      lastScrollTop = scrollTop;
      scrollDelta = 0;
      return;
    }

    const delta = scrollTop - lastScrollTop;
    scrollDelta = delta > 0 ? Math.max(0, scrollDelta) + delta : Math.min(0, scrollDelta) + delta;

    if (scrollTop <= 12) {
      topbarHidden = false;
      scrollDelta = 0;
    } else if (scrollTop > 48 && scrollDelta >= 12) {
      topbarHidden = true;
      scrollDelta = 0;
    } else if (scrollDelta <= -8) {
      topbarHidden = false;
      scrollDelta = 0;
    }
    lastScrollTop = scrollTop;
  }
</script>

<div class:immersive class:sidebar-collapsed={sidebarCollapsed} class="app-shell">
  <a
    class="skip-link"
    href={!compact && current === 'home' && workspaceLayout === 'todo'
      ? '#todo-panel'
      : '#main-content'}
    inert={compact && todoOpen}>Skip to content</a
  >
  {#if notice}
    <Alert.Root
      class="global-notice fixed top-[calc(1rem+env(safe-area-inset-top))] right-[calc(1rem+env(safe-area-inset-right))] z-90 flex w-auto max-w-[min(27.5rem,calc(100vw-2rem))] items-center gap-4 shadow-none"
      hidden={(compact && todoOpen) || immersive}
      inert={(compact && todoOpen) || immersive ? true : undefined}
      variant="destructive"
    >
      <Alert.Description>{notice}</Alert.Description>
      <Button class="ml-auto shrink-0" onclick={onDismissNotice} size="sm" variant="ghost">
        Dismiss
      </Button>
    </Alert.Root>
  {/if}
  <Sidebar
    blocked={(compact && todoOpen) || immersive}
    collapsed={sidebarCollapsed}
    {current}
    {mobile}
    {onLogout}
    {onNavigate}
    onToggleCollapsed={toggleSidebar}
    {session}
    {todoOpen}
  />

  <div
    class:has-workspace-layout={current === 'home'}
    class:layout-notes={current === 'home' && workspaceLayout === 'notes'}
    class:layout-split={current === 'home' && workspaceLayout === 'split'}
    class:layout-todo={current === 'home' && workspaceLayout === 'todo'}
    class="workspace-stage"
  >
    {#if current === 'home' && !compact}
      <header class="workspace-toolbar">
        <span>Workspace</span>
        <ToggleGroup.Root
          aria-label="Workspace layout"
          bind:value={workspaceLayout}
          class="shrink-0"
          size="sm"
          type="single"
          variant="outline"
        >
          <ToggleGroup.Item aria-label="Show Memos only" title="Memos only" value="notes">
            <PanelLeft />
          </ToggleGroup.Item>
          <ToggleGroup.Item aria-label="Show Memos and Todo" title="Memos and Todo" value="split">
            <Columns2 />
          </ToggleGroup.Item>
          <ToggleGroup.Item aria-label="Show Todo only" title="Todo only" value="todo">
            <ListChecks />
          </ToggleGroup.Item>
        </ToggleGroup.Root>
      </header>
    {/if}
    <div
      class="workspace-column"
      inert={(compact && todoOpen) ||
        (!compact && current === 'home' && workspaceLayout === 'todo')}
      onscroll={handleWorkspaceScroll}
    >
      <header class:topbar-hidden={topbarHidden} class="compact-topbar">
        <a
          href={mobile ? '/notes' : '/'}
          onclick={(event) => {
            event.preventDefault();
            onNavigate(mobile ? 'notes' : 'home');
          }}>Locus</a
        >
        <span
          >{current === 'home'
            ? 'Workspace'
            : current === 'notes'
              ? 'Memos'
              : current === 'library'
                ? 'Library'
                : current === 'tasks'
                  ? 'Tasks'
                  : 'Archive'}</span
        >
        <div class="compact-topbar-actions">
          {#if current === 'home' && !mobile}
            <Button
              bind:ref={todoButton}
              aria-controls="todo-panel"
              aria-expanded={todoOpen}
              class="todo-trigger"
              onclick={() => void openTodo()}
              type="button"
            >
              <ListTodo data-icon="inline-start" />
              Todo
            </Button>
          {/if}
          <Button
            aria-label="Sign out"
            class="hidden max-[767px]:inline-flex"
            onclick={() => void onLogout()}
            size="icon"
            title="Sign out"
            type="button"
            variant="ghost"
          >
            <LogOut />
          </Button>
        </div>
      </header>
      <main class="workspace-main" id="main-content" tabindex="-1">{@render children()}</main>
    </div>

    {#if current === 'home' && !compact}
      <aside
        aria-hidden={workspaceLayout === 'notes'}
        aria-label="Todo tasks"
        class="todo-rail"
        id="todo-panel"
        inert={workspaceLayout === 'notes'}
        tabindex="-1"
      >
        <div class="todo-content">
          <h2 class="sr-only">Todo</h2>
          <TaskBoard mode="todo" {refreshToken} today={session.workspace.today} />
        </div>
      </aside>
    {/if}
  </div>
</div>

{#if compact && !mobile && current === 'home'}
  <Sheet.Root bind:open={todoOpen} onOpenChange={handleTodoOpenChange}>
    <Sheet.Content
      class="w-full max-w-[24.375rem] overflow-y-auto p-6 pb-[calc(5rem+env(safe-area-inset-bottom))] sm:max-w-sm"
    >
      <Sheet.Header class="sr-only">
        <Sheet.Title>Todo</Sheet.Title>
        <Sheet.Description>Review and manage your current tasks.</Sheet.Description>
      </Sheet.Header>
      <TaskBoard mode="todo" {refreshToken} today={session.workspace.today} />
    </Sheet.Content>
  </Sheet.Root>
{/if}

<style>
  .app-shell {
    --mobile-navigation-content-height: 64px;
    --mobile-navigation-safe-space: max(8px, env(safe-area-inset-bottom));

    display: grid;
    height: 100%;
    min-height: 0;
    grid-template-columns: 200px minmax(0, 1fr);
    overflow: hidden;
  }

  .app-shell.sidebar-collapsed {
    grid-template-columns: 64px minmax(0, 1fr);
  }

  .skip-link {
    position: fixed;
    top: 12px;
    left: 12px;
    z-index: 110;
    padding: 9px 13px;
    color: var(--background);
    background: var(--foreground);
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 650;
    transform: translateY(calc(-100% - 20px));
    transition: transform 120ms ease;
  }

  .skip-link:focus-visible {
    transform: translateY(0);
  }

  .workspace-stage {
    position: relative;
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-columns: minmax(0, 1fr);
    overflow: hidden;
  }

  .workspace-toolbar {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 24px;
    border-bottom: 1px solid var(--border);
    font-size: 14px;
    font-weight: 600;
  }

  .workspace-column {
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-width: none;
  }

  .workspace-column::-webkit-scrollbar,
  .todo-rail::-webkit-scrollbar {
    display: none;
  }

  .workspace-main {
    min-width: 0;
  }

  .workspace-main:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .compact-topbar {
    display: none;
  }

  .compact-topbar-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .todo-rail {
    position: sticky;
    top: 0;
    z-index: 18;
    display: flex;
    min-width: 0;
    height: 100%;
    flex-direction: column;
    padding: 20px 16px 32px;
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--card);
    border-left: 1px solid var(--border);
    scrollbar-width: none;
  }

  .todo-content {
    width: 100%;
    min-width: 0;
  }

  @media (min-width: 1200px) {
    .workspace-stage.has-workspace-layout {
      grid-template-rows: auto minmax(0, 1fr);
      grid-template-columns: calc(100% - 320px) 320px;
    }

    .workspace-stage.layout-notes {
      grid-template-columns: 100% 0%;
    }

    .workspace-stage.layout-todo {
      grid-template-columns: 0% 100%;
    }

    .workspace-stage.has-workspace-layout .workspace-column,
    .workspace-stage.has-workspace-layout .todo-rail {
      opacity: 1;
      transform: translateX(0);
      visibility: visible;
      transition:
        opacity 160ms ease,
        transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1),
        border-color 160ms ease,
        visibility 0ms step-start;
    }

    .workspace-stage.layout-notes .todo-rail {
      padding-right: 0;
      padding-left: 0;
      border-left-color: transparent;
      opacity: 0;
      pointer-events: none;
      transform: translateX(16px);
      visibility: hidden;
      transition:
        opacity 140ms ease,
        transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1),
        border-color 160ms ease,
        visibility 200ms step-end;
    }

    .workspace-stage.layout-todo .workspace-column {
      opacity: 0;
      pointer-events: none;
      transform: translateX(-16px);
      visibility: hidden;
      transition:
        opacity 140ms ease,
        transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1),
        visibility 200ms step-end;
    }

    .workspace-stage.layout-todo .todo-rail {
      border-left-color: transparent;
    }

    .workspace-stage.layout-todo .todo-content {
      width: min(100%, 720px);
      margin-inline: auto;
    }
  }

  @media (max-width: 1199px) {
    .app-shell,
    .app-shell.sidebar-collapsed {
      grid-template-columns: 64px minmax(0, 1fr);
    }

    .workspace-stage {
      display: block;
      height: 100%;
    }

    .workspace-column {
      height: 100%;
    }

    .compact-topbar {
      position: sticky;
      top: 0;
      z-index: 15;
      display: grid;
      min-height: calc(56px + env(safe-area-inset-top));
      grid-template-columns: 1fr auto 1fr;
      align-items: center;
      padding: calc(8px + env(safe-area-inset-top)) 20px 8px;
      background: var(--background);
      border-bottom: 1px solid var(--border);
      transition: transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
      will-change: transform;
    }

    .compact-topbar > a {
      justify-self: start;
      font-weight: 680;
    }

    .compact-topbar > span {
      font-size: 13px;
      font-weight: 620;
    }

    .compact-topbar > :last-child {
      justify-self: end;
    }

    .todo-rail {
      display: none;
    }
  }

  @media (max-width: 767px) {
    .app-shell {
      display: block;
    }

    .compact-topbar {
      min-height: calc(48px + env(safe-area-inset-top));
      padding-top: env(safe-area-inset-top);
      padding-bottom: 0;
      padding-right: calc(16px + env(safe-area-inset-right));
      padding-left: calc(16px + env(safe-area-inset-left));
    }

    .compact-topbar.topbar-hidden {
      transform: translateY(-100%);
    }

    .compact-topbar-actions :global(button) {
      min-width: 44px;
      min-height: 44px;
    }

    .workspace-column {
      padding-bottom: calc(
        var(--mobile-navigation-content-height) + var(--mobile-navigation-safe-space)
      );
    }

    .app-shell.immersive :global(.sidebar),
    .app-shell.immersive .compact-topbar {
      display: none;
    }

    .app-shell.immersive .workspace-column {
      padding-bottom: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .skip-link,
    .app-shell,
    .compact-topbar,
    .workspace-stage.has-workspace-layout .workspace-column,
    .workspace-stage.has-workspace-layout .todo-rail {
      transition-duration: 0.01ms;
    }
  }
</style>
