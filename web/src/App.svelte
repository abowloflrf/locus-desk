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
  import { Button } from './lib/components/ui/button';
  import { Spinner } from './lib/components/ui/spinner';
  import ArchivePage from './lib/pages/ArchivePage.svelte';
  import LibraryPage from './lib/pages/LibraryPage.svelte';
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
  let immersive = $state(false);
  let sessionGeneration = 0;
  let sessionRefresh: { controller: AbortController; generation: number } | null = null;

  let pageTitle = $derived(
    route === 'login'
      ? 'Sign in · Locus Desk'
      : route === 'home'
        ? 'Workspace · Locus Desk'
        : route === 'notes'
          ? 'Memos · Locus Desk'
          : route === 'library'
            ? 'Library · Locus Desk'
            : route === 'tasks'
              ? 'Tasks · Locus Desk'
              : 'Archive · Locus Desk',
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
    immersive = false;
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
      navigate('notes');
      setTimeout(() => window.dispatchEvent(new Event('locus:focus-search')), 0);
    }
  }
</script>

<svelte:head><title>{pageTitle}</title></svelte:head>
<svelte:window onkeydown={handleGlobalKeydown} />

{#if authStatus === 'checking'}
  <main
    class="boot-screen grid min-h-dvh place-content-center justify-items-center gap-4 px-8 py-[calc(2rem+env(safe-area-inset-top))] text-center"
    aria-live="polite"
  >
    <span
      class="grid size-8 place-items-center rounded-lg bg-primary text-sm font-bold text-primary-foreground"
      aria-hidden="true">L</span
    >
    <p class="flex items-center gap-2 text-sm text-muted-foreground">
      <Spinner />
      Opening your workspace…
    </p>
  </main>
{:else if authStatus === 'error'}
  <main
    class="boot-screen grid min-h-dvh place-content-center justify-items-center gap-4 px-8 py-[calc(2rem+env(safe-area-inset-top))] text-center"
  >
    <span
      class="grid size-8 place-items-center rounded-lg bg-primary text-sm font-bold text-primary-foreground"
      aria-hidden="true">L</span
    >
    <div class="grid max-w-md gap-1.5">
      <h1 class="text-xl font-semibold tracking-tight">Workspace unavailable</h1>
      <p class="text-sm leading-6 text-muted-foreground">{authError}</p>
    </div>
    <Button onclick={() => void checkSession()}>Try again</Button>
  </main>
{:else if authStatus === 'anonymous' || !session}
  <LoginPage onLogin={handleLogin} />
{:else}
  <AppShell
    current={route as ProtectedRoute}
    {immersive}
    notice={authError}
    onDismissNotice={() => (authError = null)}
    onLogout={handleLogout}
    onNavigate={(next) => navigate(next)}
    refreshToken={taskRefreshToken}
    {session}
  >
    {#if route === 'tasks'}
      <TasksPage refreshToken={taskRefreshToken} {session} />
    {:else if route === 'library'}
      <LibraryPage onImmersiveChange={(open) => (immersive = open)} {session} />
    {:else if route === 'archive'}
      <ArchivePage {session} />
    {:else}
      <NotesPage {session} />
    {/if}
  </AppShell>
{/if}
