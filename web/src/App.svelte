<script lang="ts">
  import { onMount, tick } from 'svelte';

  import { getSession, login, logout } from './lib/api/auth';
  import {
    advanceAuthGeneration,
    ApiClientError,
    errorMessage,
    setUnauthorizedHandler,
  } from './lib/api/client';
  import type { SessionInfo } from './lib/api/types';
  import AppShell from './lib/components/AppShell.svelte';
  import ArchivePage from './lib/pages/ArchivePage.svelte';
  import LoginPage from './lib/pages/LoginPage.svelte';
  import NotesPage from './lib/pages/NotesPage.svelte';
  import TasksPage from './lib/pages/TasksPage.svelte';
  import {
    pathForRoute,
    routeFromPath,
    safeReturnPath,
    type AppRoute,
    type ProtectedRoute,
  } from './lib/routes';

  type AuthStatus = 'checking' | 'anonymous' | 'authenticated' | 'error';

  let route = $state<AppRoute>(routeFromPath(window.location.pathname));
  let authStatus = $state<AuthStatus>('checking');
  let session = $state<SessionInfo | null>(null);
  let authError = $state<string | null>(null);
  let returnPath = $state(safeReturnPath(window.location.pathname));
  let taskRefreshToken = $state(0);
  let sessionGeneration = 0;
  let sessionRefresh: { controller: AbortController; generation: number } | null = null;

  let pageTitle = $derived(
    route === 'login'
      ? 'Sign in · Locus Desk'
      : route === 'tasks'
        ? 'Tasks · Locus Desk'
        : route === 'archive'
          ? 'Archive · Locus Desk'
          : 'Notes · Locus Desk',
  );

  $effect(() => {
    if (authStatus === 'authenticated' && route === 'login') {
      queueMicrotask(() => navigate('home', true));
    }
  });

  onMount(() => {
    setUnauthorizedHandler(handleUnauthorized);
    const handlePopState = () => {
      const next = routeFromPath(window.location.pathname);
      const focusMain = authStatus === 'authenticated' && route !== next;
      route = next;
      if (focusMain) void focusMainContent();
    };
    const handleFocus = () => {
      if (authStatus === 'authenticated') void refreshSession();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible' && authStatus === 'authenticated') {
        void refreshSession();
      }
    };
    const handleTasksChanged = () => {
      taskRefreshToken += 1;
    };

    window.addEventListener('focus', handleFocus);
    window.addEventListener('popstate', handlePopState);
    window.addEventListener('locus:tasks-changed', handleTasksChanged);
    document.addEventListener('visibilitychange', handleVisibilityChange);
    const sessionRefreshTimer = window.setInterval(() => {
      if (document.visibilityState === 'visible' && authStatus === 'authenticated') {
        void refreshSession();
      }
    }, 60_000);
    void checkSession();

    return () => {
      window.clearInterval(sessionRefreshTimer);
      setUnauthorizedHandler(null);
      advanceSessionGeneration();
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('popstate', handlePopState);
      window.removeEventListener('locus:tasks-changed', handleTasksChanged);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  });

  async function checkSession(): Promise<void> {
    const generation = sessionGeneration;
    authStatus = 'checking';
    authError = null;
    try {
      const current = await getSession();
      if (generation !== sessionGeneration) return;
      session = current;
      authError = null;
      authStatus = 'authenticated';
      if (route === 'login') navigate(routeFromPath(returnPath), true);
    } catch (cause) {
      if (generation !== sessionGeneration) return;
      if (cause instanceof ApiClientError && cause.status === 401) {
        handleUnauthorized();
        return;
      }
      authStatus = 'error';
      authError = errorMessage(cause, 'Unable to reach Locus Desk.');
    }
  }

  async function refreshSession(): Promise<void> {
    const generation = sessionGeneration;
    if (sessionRefresh?.generation === generation) return;
    sessionRefresh?.controller.abort();
    const controller = new AbortController();
    sessionRefresh = { controller, generation };
    try {
      const current = await getSession(controller.signal);
      if (generation !== sessionGeneration) return;
      if (session && current.workspace.today !== session.workspace.today) {
        taskRefreshToken += 1;
      }
      session = current;
      authError = null;
    } catch (cause) {
      if (generation !== sessionGeneration) return;
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (!(cause instanceof ApiClientError && cause.status === 401)) {
        authError = errorMessage(cause, 'Unable to refresh the workspace.');
      }
    } finally {
      if (sessionRefresh?.controller === controller) sessionRefresh = null;
    }
  }

  async function handleLogin(username: string, password: string): Promise<SessionInfo> {
    const generation = sessionGeneration;
    const current = await login({ username, password });
    if (generation !== sessionGeneration) return current;
    advanceSessionGeneration();
    session = current;
    authError = null;
    authStatus = 'authenticated';
    navigate(routeFromPath(returnPath), true);
    return current;
  }

  async function handleLogout(): Promise<void> {
    const generation = sessionGeneration;
    try {
      await logout();
      if (generation !== sessionGeneration) return;
      handleUnauthorized();
    } catch (cause) {
      if (generation !== sessionGeneration) return;
      authError = errorMessage(cause, 'Unable to sign out.');
    }
  }

  function handleUnauthorized(): void {
    advanceSessionGeneration();
    if (route !== 'login') returnPath = pathForRoute(route);
    session = null;
    authStatus = 'anonymous';
    navigate('login', true);
  }

  function advanceSessionGeneration(): void {
    sessionGeneration += 1;
    sessionRefresh?.controller.abort();
    sessionRefresh = null;
    advanceAuthGeneration();
  }

  function navigate(next: AppRoute, replace = false): void {
    const focusMain = authStatus === 'authenticated' && route !== next;
    const path = pathForRoute(next);
    if (replace) window.history.replaceState({}, '', path);
    else if (window.location.pathname !== path) window.history.pushState({}, '', path);
    route = next;
    if (focusMain) void focusMainContent();
  }

  async function focusMainContent(): Promise<void> {
    await tick();
    document.getElementById('main-content')?.focus();
  }

  function handleGlobalKeydown(event: KeyboardEvent): void {
    if (authStatus !== 'authenticated') return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === 'k') {
      event.preventDefault();
      navigate('home');
      setTimeout(() => window.dispatchEvent(new Event('locus:focus-search')), 0);
    }
  }
</script>

<svelte:head><title>{pageTitle}</title></svelte:head>
<svelte:window onkeydown={handleGlobalKeydown} />

{#if authStatus === 'checking'}
  <main class="boot-screen" aria-live="polite">
    <span class="brand-mark" aria-hidden="true">L</span>
    <p>Opening your workspace…</p>
  </main>
{:else if authStatus === 'error'}
  <main class="boot-screen">
    <span class="brand-mark" aria-hidden="true">L</span>
    <h1>Workspace unavailable</h1>
    <p>{authError}</p>
    <button class="button primary" onclick={() => void checkSession()} type="button"
      >Try again</button
    >
  </main>
{:else if authStatus === 'anonymous' || !session}
  <LoginPage onLogin={handleLogin} />
{:else}
  <AppShell
    current={route as ProtectedRoute}
    notice={authError}
    onDismissNotice={() => (authError = null)}
    onLogout={handleLogout}
    onNavigate={(next) => navigate(next)}
    refreshToken={taskRefreshToken}
    {session}
  >
    {#if route === 'tasks'}
      <TasksPage refreshToken={taskRefreshToken} {session} />
    {:else if route === 'archive'}
      <ArchivePage {session} />
    {:else}
      <NotesPage {session} />
    {/if}
  </AppShell>
{/if}

<style>
  .boot-screen {
    display: grid;
    min-height: 100vh;
    align-content: center;
    justify-items: center;
    padding: 32px;
    text-align: center;
  }

  .brand-mark {
    display: grid;
    width: 32px;
    height: 32px;
    color: var(--color-surface);
    background: var(--color-accent);
    border-radius: 8px;
    font-weight: 700;
    place-items: center;
  }

  .boot-screen .brand-mark {
    margin-bottom: 18px;
  }

  .boot-screen h1 {
    margin-bottom: 7px;
    font-size: 22px;
  }

  .boot-screen p {
    max-width: 420px;
    margin-bottom: 20px;
    color: var(--color-text-muted);
  }
</style>
