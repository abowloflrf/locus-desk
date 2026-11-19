<script lang="ts">
  import { onMount } from 'svelte';

  import { getSession, login, logout } from './lib/api/auth';
  import {
    advanceAuthGeneration,
    ApiClientError,
    errorMessage,
    setUnauthorizedHandler,
  } from './lib/api/client';
  import type { SessionInfo } from './lib/api/types';
  import type { MemoListView } from './lib/memo-view';
  import { Button } from './lib/components/ui/button';
  import { Spinner } from './lib/components/ui/spinner';
  import LoginPage from './lib/pages/LoginPage.svelte';
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
  let mainContent = $state<HTMLElement>();
  let focusMainRequested = $state(false);
  let focusSearchRequested = $state(false);
  const historyKey = 'locusMemoView';
  const historyStart = Date.now();
  const memoViews = new Map<string, MemoListView>();
  let nextHistoryId = 0;
  let memoView = $state.raw<MemoListView | undefined>(undefined);
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
              : route === 'settings'
                ? 'Settings · Locus Desk'
                : 'Archive · Locus Desk',
  );

  $effect(() => {
    if (authStatus === 'authenticated' && route === 'login') {
      queueMicrotask(() => navigate('home', true));
    }
  });

  $effect(() => {
    if (focusMainRequested && mainContent) {
      mainContent.focus({ preventScroll: true });
      focusMainRequested = false;
    }
  });

  onMount(() => {
    setUnauthorizedHandler(handleUnauthorized);
    memoView = getMemoView(route);
    const handlePopState = () => {
      const next = routeFromPath(window.location.pathname);
      const view = getMemoView(next, route === next ? memoView : undefined);
      const focusMain = authStatus === 'authenticated' && route !== next;
      if (memoView !== view) saveMemoScroll();
      memoView = view;
      focusSearchRequested = false;
      route = next;
      focusMainRequested = focusMain;
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
    memoViews.clear();
    memoView = undefined;
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

  function getMemoView(next: AppRoute, currentView?: MemoListView): MemoListView | undefined {
    if (next !== 'home' && next !== 'notes' && next !== 'archive') return;
    let id: string | undefined = window.history.state?.[historyKey];
    if (!id || !memoViews.has(id)) {
      id = `${historyStart}-${++nextHistoryId}`;
      window.history.replaceState({ ...window.history.state, [historyKey]: id }, '');
      memoViews.set(id, currentView ?? { query: '', tag: '', pages: 1, scrollTop: 0 });
    }
    return memoViews.get(id);
  }

  function saveMemoScroll(): void {
    if (!memoView) return;
    const scroller = document
      .getElementById('main-content')
      ?.closest<HTMLElement>('.workspace-column');
    if (scroller) memoView.scrollTop = scroller.scrollTop;
  }

  function navigate(next: AppRoute, replace = false): void {
    immersive = false;
    focusSearchRequested = false;
    const focusMain = authStatus === 'authenticated' && route !== next;
    const path = pathForRoute(next);
    if (window.location.pathname !== path) {
      saveMemoScroll();
      const keepMemoView = replace && route === 'home' && next === 'notes';
      if (replace) {
        window.history.replaceState(keepMemoView ? window.history.state : {}, '', path);
      } else {
        window.history.pushState({}, '', path);
      }
      memoView = getMemoView(next);
    }
    route = next;
    focusMainRequested = focusMain;
  }

  function focusMemoSearch(): void {
    navigate('notes');
    focusMainRequested = false;
    focusSearchRequested = true;
  }

  function handleGlobalKeydown(event: KeyboardEvent): void {
    if (authStatus !== 'authenticated') return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === 'k') {
      event.preventDefault();
      focusMemoSearch();
    }
  }
</script>

<svelte:head><title>{pageTitle}</title></svelte:head>
<svelte:window onkeydown={handleGlobalKeydown} />

{#if authStatus === 'checking'}
  {@render openingWorkspace()}
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
  {#await import('./lib/components/AppShell.svelte')}
    {@render openingWorkspace()}
  {:then { default: AppShell }}
    <AppShell
      bind:mainContent
      current={route as ProtectedRoute}
      {immersive}
      notice={authError}
      onDismissNotice={() => (authError = null)}
      onLogout={handleLogout}
      onNavigate={navigate}
      onSearch={focusMemoSearch}
      refreshToken={taskRefreshToken}
      {session}
    >
      {#if route === 'tasks'}
        {#await import('./lib/pages/TasksPage.svelte')}
          {@render openingPage()}
        {:then { default: TasksPage }}
          <TasksPage refreshToken={taskRefreshToken} {session} />
        {:catch}
          {@render pageUnavailable()}
        {/await}
      {:else if route === 'library'}
        {#await import('./lib/pages/LibraryPage.svelte')}
          {@render openingPage()}
        {:then { default: LibraryPage }}
          <LibraryPage onImmersiveChange={(open) => (immersive = open)} {session} />
        {:catch}
          {@render pageUnavailable()}
        {/await}
      {:else if route === 'settings'}
        {#await import('./lib/pages/SettingsPage.svelte')}
          {@render openingPage()}
        {:then { default: SettingsPage }}
          <SettingsPage />
        {:catch}
          {@render pageUnavailable()}
        {/await}
      {:else if route === 'archive'}
        {#await import('./lib/pages/ArchivePage.svelte')}
          {@render openingPage()}
        {:then { default: ArchivePage }}
          {#key memoView}
            <ArchivePage listView={memoView} {session} />
          {/key}
        {:catch}
          {@render pageUnavailable()}
        {/await}
      {:else}
        {#await import('./lib/pages/NotesPage.svelte')}
          {@render openingPage()}
        {:then { default: NotesPage }}
          {#key memoView}
            <NotesPage
              listView={memoView}
              onOpenArchive={() => navigate('archive')}
              {focusSearchRequested}
              onSearchFocused={() => (focusSearchRequested = false)}
              {session}
            />
          {/key}
        {:catch}
          {@render pageUnavailable()}
        {/await}
      {/if}
    </AppShell>
  {:catch}
    <main>{@render pageUnavailable()}</main>
  {/await}
{/if}

{#snippet openingWorkspace()}
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
{/snippet}

{#snippet openingPage()}
  <div class="loading-state flex items-center justify-center gap-2" role="status">
    <Spinner />
    Opening page…
  </div>
{/snippet}

{#snippet pageUnavailable()}
  <div class="grid justify-items-center gap-3 px-6 py-12 text-center" role="alert">
    <p class="m-0 text-sm">Unable to load this page.</p>
    <Button onclick={() => window.location.reload()} variant="secondary">Reload page</Button>
  </div>
{/snippet}
