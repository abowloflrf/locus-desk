<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte';

  import type { SessionInfo } from '../api/types';
  import type { ProtectedRoute } from '../routes';
  import Icon from './Icon.svelte';
  import Sidebar from './Sidebar.svelte';
  import TaskBoard from './TaskBoard.svelte';

  let {
    current,
    session,
    refreshToken = 0,
    children,
    onNavigate,
    onLogout,
    notice = null,
    onDismissNotice,
  }: {
    current: ProtectedRoute;
    session: SessionInfo;
    refreshToken?: number;
    children: Snippet;
    onNavigate: (route: ProtectedRoute) => void;
    onLogout: () => void | Promise<void>;
    notice?: string | null;
    onDismissNotice: () => void;
  } = $props();

  type WorkspaceLayout = 'notes' | 'split' | 'todo';

  let compact = $state(false);
  let todoOpen = $state(false);
  let workspaceLayout = $state<WorkspaceLayout>('split');
  let todoButton = $state<HTMLButtonElement>();
  let drawerClose = $state<HTMLButtonElement>();
  let drawer = $state<HTMLElement>();

  onMount(() => {
    const media = window.matchMedia('(max-width: 1199px)');
    const update = () => {
      compact = media.matches;
      if (!compact) todoOpen = false;
    };
    update();
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  });

  $effect(() => {
    current;
    todoOpen = false;
  });

  $effect(() => {
    if (todoOpen) requestAnimationFrame(() => drawerClose?.focus());
  });

  async function openTodo(): Promise<void> {
    if (current !== 'home') {
      onNavigate('home');
      await tick();
    }
    todoOpen = true;
  }

  function closeTodo(restoreFocus = true): void {
    todoOpen = false;
    if (restoreFocus) requestAnimationFrame(() => todoButton?.focus());
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (!todoOpen) return;
    if (event.defaultPrevented || document.querySelector('dialog[open]')) return;
    if (event.key === 'Escape') {
      closeTodo();
      return;
    }
    if (event.key !== 'Tab' || !drawer) return;

    const focusable = [
      ...drawer.querySelectorAll<HTMLElement>('button, input, textarea, [href]'),
    ].filter((element) => !element.hasAttribute('disabled') && element.tabIndex !== -1);
    const first = focusable.at(0);
    const last = focusable.at(-1);
    if (!first || !last) return;

    if (!drawer.contains(document.activeElement)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<div class:todo-open={todoOpen} class="app-shell">
  <a
    class="skip-link"
    href={!compact && current === 'home' && workspaceLayout === 'todo'
      ? '#todo-panel'
      : '#main-content'}
    inert={compact && todoOpen}>Skip to content</a
  >
  {#if notice}
    <div
      class="global-notice"
      hidden={compact && todoOpen}
      inert={compact && todoOpen}
      role="alert"
    >
      <span>{notice}</span>
      <button class="button secondary" onclick={onDismissNotice} type="button">Dismiss</button>
    </div>
  {/if}
  <Sidebar blocked={compact && todoOpen} {current} {onLogout} {onNavigate} {session} {todoOpen} />

  <div
    class:has-workspace-layout={current === 'home'}
    class:layout-notes={current === 'home' && workspaceLayout === 'notes'}
    class:layout-split={current === 'home' && workspaceLayout === 'split'}
    class:layout-todo={current === 'home' && workspaceLayout === 'todo'}
    class="workspace-stage"
  >
    <div
      class="workspace-column"
      inert={(compact && todoOpen) ||
        (!compact && current === 'home' && workspaceLayout === 'todo')}
    >
      <header class="compact-topbar">
        <a
          href="/"
          onclick={(event) => {
            event.preventDefault();
            onNavigate('home');
          }}>Locus</a
        >
        <span
          >{current === 'home'
            ? 'Workspace'
            : current === 'notes'
              ? 'Memos'
              : current === 'tasks'
                ? 'Tasks'
                : 'Archive'}</span
        >
        <div class="compact-topbar-actions">
          {#if current === 'home'}
            <button
              bind:this={todoButton}
              aria-controls="todo-panel"
              aria-expanded={todoOpen}
              class="button secondary"
              onclick={() => void openTodo()}
              type="button">Todo</button
            >
          {/if}
          <button
            aria-label="Sign out"
            class="icon-button compact-logout"
            onclick={() => void onLogout()}
            title="Sign out"
            type="button"
          >
            <Icon name="logout" />
          </button>
        </div>
      </header>
      <main class="workspace-main" id="main-content" tabindex="-1">{@render children()}</main>
    </div>

    {#if current === 'home'}
      <button
        aria-label="Close Todo panel"
        class:visible={todoOpen}
        class="drawer-backdrop"
        onclick={() => closeTodo()}
        tabindex="-1"
        type="button"
      ></button>
      <aside
        aria-hidden={compact ? !todoOpen : workspaceLayout === 'notes'}
        aria-label="Todo tasks"
        aria-modal={compact ? 'true' : undefined}
        class:open={todoOpen}
        class="todo-rail"
        bind:this={drawer}
        id="todo-panel"
        inert={compact ? !todoOpen : workspaceLayout === 'notes'}
        role={compact ? 'dialog' : 'complementary'}
        tabindex="-1"
      >
        <div class="todo-content">
          <header class="todo-header">
            <h2 class="sr-only">Todo</h2>
            <button
              aria-label="Close Todo panel"
              bind:this={drawerClose}
              class="icon-button drawer-close"
              onclick={() => closeTodo()}
              type="button"><Icon name="close" /></button
            >
          </header>
          <TaskBoard mode="todo" {refreshToken} today={session.workspace.today} />
        </div>
      </aside>

      <div
        aria-label="Workspace layout"
        class:show-notes={workspaceLayout === 'notes'}
        class:show-split={workspaceLayout === 'split'}
        class:show-todo={workspaceLayout === 'todo'}
        class="workspace-layout-switcher"
        role="group"
      >
        <button
          aria-label="Show Memos only"
          aria-pressed={workspaceLayout === 'notes'}
          onclick={() => (workspaceLayout = 'notes')}
          title="Memos only"
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 24 18">
            <rect height="15" rx="2.5" width="21" x="1.5" y="1.5"></rect>
            <path d="M5.5 6.5h13M5.5 10h9"></path>
          </svg>
        </button>
        <button
          aria-label="Show Memos and Todo"
          aria-pressed={workspaceLayout === 'split'}
          onclick={() => (workspaceLayout = 'split')}
          title="Memos and Todo"
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 24 18">
            <rect height="15" rx="2.5" width="21" x="1.5" y="1.5"></rect>
            <path d="M15.5 2v14M5.5 6.5h6M5.5 10h4M18.5 6.5h1M18.5 10h1"></path>
          </svg>
        </button>
        <button
          aria-label="Show Todo only"
          aria-pressed={workspaceLayout === 'todo'}
          onclick={() => (workspaceLayout = 'todo')}
          title="Todo only"
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 24 18">
            <rect height="15" rx="2.5" width="21" x="1.5" y="1.5"></rect>
            <path d="m5 6.5 1 1 1.8-2M10.5 6.5h8M5 11l1 1 1.8-2M10.5 11h8"></path>
          </svg>
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .app-shell {
    display: grid;
    height: 100%;
    min-height: 0;
    grid-template-columns: 224px minmax(0, 1fr);
    overflow: hidden;
  }

  .workspace-stage {
    position: relative;
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-columns: minmax(0, 1fr);
    overflow: hidden;
  }

  .todo-content {
    width: 100%;
    min-width: 0;
  }

  .skip-link {
    position: fixed;
    top: 12px;
    left: 12px;
    z-index: 110;
    padding: 9px 13px;
    color: var(--color-surface);
    background: var(--color-text);
    border-radius: 7px;
    font-size: 13px;
    font-weight: 650;
    transform: translateY(calc(-100% - 20px));
    transition: transform 120ms ease;
  }

  .skip-link:focus-visible {
    transform: translateY(0);
  }

  .workspace-column {
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-width: none;
  }

  .workspace-column::-webkit-scrollbar {
    display: none;
  }

  .compact-topbar {
    display: none;
  }

  .compact-topbar-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .compact-logout {
    display: none;
  }

  .workspace-main {
    min-width: 0;
  }

  .todo-rail {
    position: sticky;
    top: 0;
    z-index: 18;
    display: flex;
    min-width: 0;
    height: 100%;
    flex-direction: column;
    padding: 32px 24px 76px;
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--color-surface);
    border-left: 1px solid var(--color-border);
    scrollbar-width: none;
  }

  .todo-rail::-webkit-scrollbar {
    display: none;
  }

  .todo-header {
    display: flex;
    justify-content: flex-end;
  }

  .drawer-close,
  .drawer-backdrop {
    display: none;
  }

  .workspace-layout-switcher {
    display: none;
  }

  .global-notice {
    position: fixed;
    top: calc(16px + env(safe-area-inset-top));
    right: calc(16px + env(safe-area-inset-right));
    z-index: 90;
    display: flex;
    max-width: min(440px, calc(100vw - 32px));
    align-items: center;
    gap: 16px;
    padding: 12px 14px;
    color: var(--color-danger);
    background: var(--color-surface);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-input);
    box-shadow: var(--shadow-floating);
    font-size: 13px;
  }

  .global-notice[hidden] {
    display: none;
  }

  .global-notice .button {
    flex: none;
  }

  @media (min-width: 1200px) {
    .workspace-stage.has-workspace-layout {
      grid-template-columns: calc(100% - 336px) 336px;
      transition: grid-template-columns 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
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
        padding 200ms cubic-bezier(0.2, 0.8, 0.2, 1),
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
        padding 200ms cubic-bezier(0.2, 0.8, 0.2, 1),
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

    .workspace-layout-switcher {
      position: absolute;
      bottom: 18px;
      left: 50%;
      z-index: 30;
      display: flex;
      gap: 2px;
      padding: 3px;
      background: color-mix(in oklch, var(--color-surface), transparent 4%);
      border: 1px solid var(--color-border);
      border-radius: var(--radius-surface);
      box-shadow: var(--shadow-soft);
      transform: translateX(-50%);
    }

    .workspace-layout-switcher::before {
      position: absolute;
      top: 3px;
      left: 3px;
      width: 40px;
      height: 40px;
      background: var(--color-accent-soft);
      border-radius: var(--radius-control);
      content: '';
      transition: transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
    }

    .workspace-layout-switcher.show-split::before {
      transform: translateX(42px);
    }

    .workspace-layout-switcher.show-todo::before {
      transform: translateX(84px);
    }

    .workspace-layout-switcher button {
      position: relative;
      z-index: 1;
      display: grid;
      width: 40px;
      height: 40px;
      padding: 0;
      color: var(--color-text-muted);
      background: transparent;
      border: 0;
      border-radius: var(--radius-control);
      place-items: center;
      transition:
        color 150ms ease,
        transform 150ms ease;
    }

    .workspace-layout-switcher button:hover {
      color: var(--color-text);
    }

    .workspace-layout-switcher button:active {
      transform: scale(0.94);
    }

    .workspace-layout-switcher button[aria-pressed='true'] {
      color: var(--color-accent-hover);
    }

    .workspace-layout-switcher svg {
      width: 24px;
      height: 18px;
      fill: none;
      stroke: currentColor;
      stroke-linecap: round;
      stroke-linejoin: round;
      stroke-width: 1.35;
    }
  }

  @media (max-width: 1199px) {
    .app-shell {
      grid-template-columns: 64px minmax(0, 1fr);
    }

    .workspace-stage {
      display: block;
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
      background: color-mix(in oklch, var(--color-canvas), transparent 6%);
      border-bottom: 1px solid var(--color-border);
      backdrop-filter: blur(12px);
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
      position: fixed;
      top: 0;
      right: 0;
      z-index: 50;
      width: min(368px, calc(100vw - 64px));
      max-width: 100%;
      height: 100dvh;
      transform: translateX(102%);
      box-shadow: -14px 0 44px color-mix(in oklch, var(--color-text), transparent 86%);
      visibility: hidden;
      transition:
        transform 190ms ease,
        visibility 190ms step-end;
    }

    .todo-rail.open {
      transform: translateX(0);
      visibility: visible;
      transition:
        transform 190ms ease,
        visibility 0ms step-start;
    }

    .drawer-close {
      display: inline-grid;
    }

    .todo-header {
      padding-bottom: 14px;
    }

    .drawer-backdrop {
      position: fixed;
      inset: 0;
      z-index: 45;
      display: block;
      padding: 0;
      background: color-mix(in oklch, var(--color-text), transparent 78%);
      border: 0;
      opacity: 0;
      pointer-events: none;
      transition: opacity 190ms ease;
    }

    .drawer-backdrop.visible {
      opacity: 1;
      pointer-events: auto;
    }
  }

  @media (max-width: 767px) {
    .app-shell {
      display: block;
    }

    .compact-topbar {
      padding-right: calc(16px + env(safe-area-inset-right));
      padding-left: calc(16px + env(safe-area-inset-left));
    }

    .workspace-column {
      height: 100%;
      padding-bottom: calc(64px + env(safe-area-inset-bottom));
    }

    .compact-logout {
      display: inline-grid;
    }

    .todo-rail {
      width: min(100%, 390px);
      padding: calc(24px + env(safe-area-inset-top)) calc(18px + env(safe-area-inset-right))
        calc(82px + env(safe-area-inset-bottom)) calc(18px + env(safe-area-inset-left));
    }
  }
</style>
