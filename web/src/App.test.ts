import { mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import App from './App.svelte';
import { getSession, login, logout } from './lib/api/auth';
import { listNotes, listTags } from './lib/api/notes';
import { listTasks } from './lib/api/tasks';
import type { SessionInfo } from './lib/api/types';

vi.mock('./lib/api/auth', () => ({
  getSession: vi.fn(),
  login: vi.fn(),
  logout: vi.fn(),
}));

vi.mock('./lib/api/notes', () => ({
  createNote: vi.fn(),
  deleteNote: vi.fn(),
  listNotes: vi.fn(),
  listTags: vi.fn(),
  updateNote: vi.fn(),
}));

vi.mock('./lib/api/tasks', () => ({
  createTask: vi.fn(),
  deleteTask: vi.fn(),
  listTasks: vi.fn(),
  updateTask: vi.fn(),
}));

const oldSession = session('old-owner', '2026-08-23');
const newSession = session('new-owner', '2026-08-24');

beforeEach(() => {
  window.history.replaceState({}, '', '/');
  vi.stubGlobal(
    'matchMedia',
    vi.fn(
      (query: string) =>
        ({
          addEventListener: vi.fn(),
          addListener: vi.fn(),
          dispatchEvent: vi.fn(),
          matches: false,
          media: query,
          onchange: null,
          removeEventListener: vi.fn(),
          removeListener: vi.fn(),
        }) as unknown as MediaQueryList,
    ),
  );
  vi.mocked(listNotes).mockResolvedValue({ items: [], page: 1, pageSize: 30, total: 0 });
  vi.mocked(listTags).mockResolvedValue({ items: [] });
  vi.mocked(listTasks).mockResolvedValue({ items: [] });
  vi.mocked(login).mockResolvedValue(newSession);
  vi.mocked(logout).mockResolvedValue();
});

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('App session generations', () => {
  it.each(['success', 'failure'] as const)(
    'ignores a stale refresh %s after logout and a new login',
    async (outcome) => {
      const staleRefresh = deferred<SessionInfo>();
      vi.mocked(getSession)
        .mockResolvedValueOnce(oldSession)
        .mockReturnValueOnce(staleRefresh.promise);
      const { component, target } = mountApp();

      try {
        await vi.waitFor(() => expect(currentUsername(target)).toBe('old-owner'));
        window.dispatchEvent(new Event('focus'));
        await vi.waitFor(() => expect(getSession).toHaveBeenCalledTimes(2));
        const staleSignal = vi.mocked(getSession).mock.calls[1]?.[0];

        target.querySelector<HTMLButtonElement>('[aria-label="Sign out"]')?.click();
        await vi.waitFor(() => expect(target.querySelector('.login-page')).not.toBeNull());
        expect(staleSignal?.aborted).toBe(true);
        await signIn(target);
        await vi.waitFor(() => expect(currentUsername(target)).toBe('new-owner'));

        if (outcome === 'success') staleRefresh.resolve(oldSession);
        else staleRefresh.reject(new Error('Stale refresh failed.'));
        await staleRefresh.promise.catch(() => undefined);
        await Promise.resolve();

        expect(currentUsername(target)).toBe('new-owner');
        expect(target.querySelector('.global-notice')).toBeNull();
      } finally {
        await unmount(component);
      }
    },
  );

  it('clears an earlier refresh notice after a successful login', async () => {
    vi.mocked(getSession)
      .mockResolvedValueOnce(oldSession)
      .mockRejectedValueOnce(new Error('Workspace refresh failed.'));
    const { component, target } = mountApp();

    try {
      await vi.waitFor(() => expect(currentUsername(target)).toBe('old-owner'));
      window.dispatchEvent(new Event('focus'));
      await vi.waitFor(() =>
        expect(target.querySelector('.global-notice')?.textContent).toContain(
          'Workspace refresh failed.',
        ),
      );

      target.querySelector<HTMLButtonElement>('[aria-label="Sign out"]')?.click();
      await vi.waitFor(() => expect(target.querySelector('.login-page')).not.toBeNull());
      await signIn(target);

      await vi.waitFor(() => expect(currentUsername(target)).toBe('new-owner'));
      expect(target.querySelector('.global-notice')).toBeNull();
    } finally {
      await unmount(component);
    }
  });
});

describe('App navigation focus', () => {
  it('offers a skip link and focuses main content after SPA navigation', async () => {
    vi.mocked(getSession).mockResolvedValue(oldSession);
    const { component, target } = mountApp();

    try {
      await vi.waitFor(() => expect(currentUsername(target)).toBe('old-owner'));
      const skipLink = target.querySelector<HTMLAnchorElement>('.skip-link')!;
      expect(skipLink.getAttribute('href')).toBe('#main-content');
      skipLink.focus();
      expect(document.activeElement).toBe(skipLink);

      target.querySelector<HTMLAnchorElement>('.nav-item[title="Notes"]')?.click();
      await vi.waitFor(() => expect(window.location.pathname).toBe('/notes'));
      await vi.waitFor(() => expect(document.activeElement?.id).toBe('main-content'));
      expect(target.querySelector('.todo-rail')).toBeNull();

      target.querySelector<HTMLAnchorElement>('.nav-item[title="Workspace"]')?.click();
      await vi.waitFor(() => expect(window.location.pathname).toBe('/'));
      await vi.waitFor(() => expect(target.querySelector('.todo-rail')).not.toBeNull());

      target.querySelector<HTMLAnchorElement>('.nav-item[title="Tasks"]')?.click();
      await vi.waitFor(() => expect(window.location.pathname).toBe('/tasks'));
      await vi.waitFor(() => expect(document.activeElement?.id).toBe('main-content'));

      target.querySelector<HTMLAnchorElement>('.nav-item[title="Archive"]')?.click();
      await vi.waitFor(() => expect(window.location.pathname).toBe('/archive'));
      await vi.waitFor(() => expect(document.activeElement?.id).toBe('main-content'));
    } finally {
      await unmount(component);
    }
  });

  it('focuses main content on popstate without pushing another history entry', async () => {
    vi.mocked(getSession).mockResolvedValue(oldSession);
    const { component, target } = mountApp();
    const pushState = vi.spyOn(window.history, 'pushState');

    try {
      await vi.waitFor(() => expect(currentUsername(target)).toBe('old-owner'));
      target.querySelector<HTMLAnchorElement>('.skip-link')?.focus();
      window.history.replaceState({}, '', '/tasks');
      window.dispatchEvent(new PopStateEvent('popstate'));

      await vi.waitFor(() => expect(target.querySelector('h1')?.textContent).toBe('Tasks'));
      await vi.waitFor(() => expect(document.activeElement?.id).toBe('main-content'));
      expect(pushState).not.toHaveBeenCalled();
    } finally {
      pushState.mockRestore();
      await unmount(component);
    }
  });
});

function mountApp(): { component: ReturnType<typeof mount>; target: HTMLDivElement } {
  const target = document.createElement('div');
  document.body.append(target);
  return { component: mount(App, { target }), target };
}

async function signIn(target: HTMLElement): Promise<void> {
  const username = target.querySelector<HTMLInputElement>('input[autocomplete="username"]')!;
  const password = target.querySelector<HTMLInputElement>(
    'input[autocomplete="current-password"]',
  )!;
  username.value = 'new-owner';
  username.dispatchEvent(new Event('input', { bubbles: true }));
  password.value = 'new password';
  password.dispatchEvent(new Event('input', { bubbles: true }));
  target.querySelector<HTMLButtonElement>('.login-button')?.click();
  await vi.waitFor(() => expect(login).toHaveBeenCalledOnce());
}

function currentUsername(target: HTMLElement): string | undefined {
  return target.querySelector('.sidebar-user strong')?.textContent ?? undefined;
}

function session(username: string, today: string): SessionInfo {
  return {
    user: { uid: `user-${username}`, username },
    workspace: {
      name: 'Personal',
      role: 'OWNER',
      timezone: 'Asia/Singapore',
      today,
      uid: `workspace-${username}`,
    },
  };
}

function deferred<T>(): {
  promise: Promise<T>;
  reject: (cause: unknown) => void;
  resolve: (value: T) => void;
} {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}
