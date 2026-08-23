<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte';

  import type { SessionInfo } from '../api/types';
  import type { ProtectedRoute } from '../routes';
  import { formatTodayLabel } from '../utils/date';
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

  let compact = $state(false);
  let todayOpen = $state(false);
  let todayButton = $state<HTMLButtonElement>();
  let drawerClose = $state<HTMLButtonElement>();
  let drawer = $state<HTMLElement>();

  onMount(() => {
    const media = window.matchMedia('(max-width: 1199px)');
    const update = () => {
      compact = media.matches;
      if (!compact) todayOpen = false;
    };
    update();
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  });

  $effect(() => {
    current;
    todayOpen = false;
  });

  $effect(() => {
    if (todayOpen) requestAnimationFrame(() => drawerClose?.focus());
  });

  async function openToday(): Promise<void> {
    if (current !== 'home') {
      onNavigate('home');
      await tick();
    }
    todayOpen = true;
  }

  function closeToday(restoreFocus = true): void {
    todayOpen = false;
    if (restoreFocus) requestAnimationFrame(() => todayButton?.focus());
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (!todayOpen) return;
    if (event.defaultPrevented || document.querySelector('dialog[open]')) return;
    if (event.key === 'Escape') {
      closeToday();
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

<div class:has-today={current === 'home'} class:today-open={todayOpen} class="app-shell">
  <a class="skip-link" href="#main-content" inert={compact && todayOpen}>Skip to content</a>
  {#if notice}
    <div
      class="global-notice"
      hidden={compact && todayOpen}
      inert={compact && todayOpen}
      role="alert"
    >
      <span>{notice}</span>
      <button class="button secondary" onclick={onDismissNotice} type="button">Dismiss</button>
    </div>
  {/if}
  <Sidebar
    blocked={compact && todayOpen}
    {current}
    {onLogout}
    {onNavigate}
    onOpenToday={openToday}
    {session}
    {todayOpen}
  />

  <div class="workspace-column" inert={compact && todayOpen}>
    <header class="compact-topbar">
      <a
        href="/"
        onclick={(event) => {
          event.preventDefault();
          onNavigate('home');
        }}>Locus</a
      >
      <span>{current === 'home' ? 'Notes' : current === 'tasks' ? 'Tasks' : 'Archive'}</span>
      <div class="compact-topbar-actions">
        {#if current === 'home'}
          <button
            bind:this={todayButton}
            class="button secondary"
            onclick={() => void openToday()}
            type="button">Today</button
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
      aria-label="Close Today panel"
      class:visible={todayOpen}
      class="drawer-backdrop"
      onclick={() => closeToday()}
      tabindex="-1"
      type="button"
    ></button>
    <aside
      aria-hidden={compact && !todayOpen}
      aria-label="Today tasks"
      aria-modal={compact ? 'true' : undefined}
      class:open={todayOpen}
      class="today-rail"
      bind:this={drawer}
      id="today-panel"
      inert={compact && !todayOpen}
      role={compact ? 'dialog' : 'complementary'}
    >
      <header class="today-header">
        <div>
          <h2>Today</h2>
          <p>{formatTodayLabel(session.workspace.today)}</p>
        </div>
        <button
          aria-label="Close Today panel"
          bind:this={drawerClose}
          class="icon-button drawer-close"
          onclick={() => closeToday()}
          type="button"><Icon name="close" /></button
        >
      </header>
      <TaskBoard mode="today" {refreshToken} today={session.workspace.today} />
    </aside>
  {/if}
</div>

<style>
  .app-shell {
    display: grid;
    height: 100%;
    min-height: 0;
    grid-template-columns: 224px minmax(0, 1fr);
    overflow: hidden;
  }

  .app-shell.has-today {
    grid-template-columns: 224px minmax(0, 1fr) 336px;
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

  .today-rail {
    position: sticky;
    top: 0;
    z-index: 18;
    display: flex;
    min-width: 0;
    height: 100%;
    flex-direction: column;
    padding: 32px 24px 22px;
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--color-surface);
    border-left: 1px solid var(--color-border);
    scrollbar-width: none;
  }

  .today-rail::-webkit-scrollbar {
    display: none;
  }

  .today-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding-bottom: 22px;
  }

  .today-header h2 {
    margin-bottom: 2px;
    font-size: 22px;
    font-weight: 670;
    letter-spacing: -0.02em;
  }

  .today-header p {
    margin-bottom: 0;
    color: var(--color-accent-hover);
    font-size: 12px;
  }

  .drawer-close,
  .drawer-backdrop {
    display: none;
  }

  .global-notice {
    position: fixed;
    top: 16px;
    right: 16px;
    z-index: 90;
    display: flex;
    max-width: min(440px, calc(100vw - 32px));
    align-items: center;
    gap: 16px;
    padding: 12px 14px;
    color: var(--color-danger);
    background: var(--color-surface);
    border: 1px solid var(--color-danger);
    border-radius: 8px;
    box-shadow: 0 12px 32px color-mix(in oklch, var(--color-text), transparent 88%);
    font-size: 13px;
  }

  .global-notice[hidden] {
    display: none;
  }

  .global-notice .button {
    flex: none;
  }

  @media (max-width: 1199px) {
    .app-shell,
    .app-shell.has-today {
      grid-template-columns: 64px minmax(0, 1fr);
    }

    .compact-topbar {
      position: sticky;
      top: 0;
      z-index: 15;
      display: grid;
      min-height: 56px;
      grid-template-columns: 1fr auto 1fr;
      align-items: center;
      padding: 8px 20px;
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

    .today-rail {
      position: fixed;
      top: 0;
      right: 0;
      z-index: 50;
      width: min(368px, calc(100vw - 64px));
      max-width: 100%;
      transform: translateX(102%);
      box-shadow: -14px 0 44px color-mix(in oklch, var(--color-text), transparent 86%);
      visibility: hidden;
      transition:
        transform 190ms ease,
        visibility 190ms step-end;
    }

    .today-rail.open {
      transform: translateX(0);
      visibility: visible;
      transition:
        transform 190ms ease,
        visibility 0ms step-start;
    }

    .drawer-close {
      display: inline-grid;
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
    .app-shell,
    .app-shell.has-today {
      display: block;
    }

    .compact-topbar {
      padding-inline: 16px;
    }

    .workspace-column {
      height: 100%;
      padding-bottom: 64px;
    }

    .compact-logout {
      display: inline-grid;
    }

    .today-rail {
      width: min(100%, 390px);
      padding: 24px 18px 82px;
    }
  }
</style>
