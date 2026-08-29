import { createRawSnippet, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listTasks } from '../api/tasks';
import type { SessionInfo } from '../api/types';
import AppShell from './AppShell.svelte';
import shellSource from './AppShell.svelte?raw';

vi.mock('../api/tasks', () => ({
  createTask: vi.fn(),
  deleteTask: vi.fn(),
  listTasks: vi.fn(),
  updateTask: vi.fn(),
}));

const session: SessionInfo = {
  user: { uid: 'user-1', username: 'owner' },
  workspace: {
    name: 'Personal',
    role: 'OWNER',
    timezone: 'Asia/Singapore',
    today: '2026-08-23',
    uid: 'workspace-1',
  },
};

beforeEach(() => {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(
      (query: string) =>
        ({
          addEventListener: vi.fn(),
          matches: true,
          media: query,
          removeEventListener: vi.fn(),
        }) as unknown as MediaQueryList,
    ),
  );
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  vi.mocked(listTasks).mockResolvedValue({ items: [] });
});

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('AppShell modal boundaries', () => {
  it('hides mobile navigation chrome while an immersive reader is open', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({ render: () => '<article>Reader</article>' }));
    const component = mount(AppShell, {
      props: {
        children,
        current: 'library',
        immersive: true,
        onDismissNotice: vi.fn(),
        onLogout: vi.fn(),
        onNavigate: vi.fn(),
        session,
      },
      target,
    });

    try {
      await tick();
      expect(target.querySelector('.app-shell')?.classList).toContain('immersive');
      expect(inertState(target.querySelector('.sidebar'))).toBe(true);
      expect(shellSource).toContain('.app-shell.immersive :global(.sidebar)');
      expect(shellSource).toContain('.app-shell.immersive .compact-topbar');
      expect(shellSource).toContain('.app-shell.immersive .workspace-column');
    } finally {
      await unmount(component);
    }
  });

  it('opens mobile More on Archive and returns focus with Escape', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({ render: () => '<h1>Notes</h1>' }));
    const component = mount(AppShell, {
      props: {
        children,
        current: 'notes',
        onDismissNotice: vi.fn(),
        onLogout: vi.fn(),
        onNavigate: vi.fn(),
        session,
      },
      target,
    });

    try {
      await tick();
      const more = target.querySelector<HTMLButtonElement>('.mobile-more-trigger')!;
      more.click();
      const archive = await vi.waitFor(() => {
        const item = document.body.querySelector<HTMLElement>(
          '[role="menuitem"][aria-label="Archive"]',
        );
        expect(item).not.toBeNull();
        return item!;
      });
      expect(document.activeElement?.closest('[role="menu"]')).not.toBeNull();
      expect(more.getAttribute('aria-expanded')).toBe('true');

      document.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
      await vi.waitFor(() =>
        expect(document.body.querySelector('[role="menuitem"][aria-label="Archive"]')).toBeNull(),
      );
      expect(document.activeElement).toBe(more);
      expect(more.getAttribute('aria-expanded')).toBe('false');
    } finally {
      await unmount(component);
    }
  });

  it('isolates the notice while the mobile Todo sheet is open', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({ render: () => '<h1>Notes</h1>' }));
    const component = mount(AppShell, {
      props: {
        children,
        current: 'home',
        notice: 'Refresh failed.',
        onDismissNotice: vi.fn(),
        onLogout: vi.fn(),
        onNavigate: vi.fn(),
        session,
      },
      target,
    });

    try {
      await tick();
      expect(target.querySelector('.sidebar .brand')?.getAttribute('aria-label')).toBe(
        'Locus Desk home',
      );
      const workspaceShortcut = target.querySelector<HTMLAnchorElement>(
        '.sidebar .nav-item[title="Workspace"]',
      );
      expect(workspaceShortcut?.getAttribute('aria-current')).toBe('page');
      expect(target.querySelector('.sidebar .nav-item[title="Memos"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Library"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Todo"]')).toBeNull();
      const todoButton = [
        ...target.querySelectorAll<HTMLButtonElement>('.compact-topbar button'),
      ].find((button) => button.textContent?.trim() === 'Todo')!;
      todoButton.click();
      await vi.waitFor(() =>
        expect(document.body.querySelector('[data-slot="sheet-content"]')).not.toBeNull(),
      );

      expect(todoButton.getAttribute('aria-expanded')).toBe('true');
      const notice = target.querySelector<HTMLElement>('.global-notice');
      expect(notice?.hasAttribute('inert')).toBe(true);
      expect(notice?.hidden).toBe(true);
      document.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
      await vi.waitFor(() =>
        expect(document.body.querySelector('[data-slot="sheet-content"]')).toBeNull(),
      );
      expect(notice?.hasAttribute('inert')).toBe(false);
      expect(notice?.hidden).toBe(false);
    } finally {
      await unmount(component);
    }
  });

  it('switches between the three desktop Workspace layouts without unmounting either surface', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(
        (query: string) =>
          ({
            addEventListener: vi.fn(),
            matches: false,
            media: query,
            removeEventListener: vi.fn(),
          }) as unknown as MediaQueryList,
      ),
    );
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({ render: () => '<h1>Notes</h1>' }));
    const component = mount(AppShell, {
      props: {
        children,
        current: 'home',
        onDismissNotice: vi.fn(),
        onLogout: vi.fn(),
        onNavigate: vi.fn(),
        session,
      },
      target,
    });

    try {
      await vi.waitFor(() =>
        expect(target.querySelector('.workspace-stage')?.classList.contains('layout-split')).toBe(
          true,
        ),
      );
      expect(target.querySelectorAll('.sidebar .nav-item[aria-current="page"]')).toHaveLength(1);
      expect(target.querySelector('.sidebar .nav-item[title="Workspace"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Memos"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Library"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Tasks"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Archive"]')).not.toBeNull();
      expect(target.querySelector('[aria-label="Workspace view"]')).toBeNull();
      const notes = target.querySelector<HTMLElement>('.workspace-column')!;
      const todo = target.querySelector<HTMLElement>('.todo-rail')!;
      const switcher = target.querySelector<HTMLElement>('[aria-label="Workspace layout"]')!;
      const notesOnly = switcher.querySelector<HTMLButtonElement>(
        '[aria-label="Show Memos only"]',
      )!;
      const split = switcher.querySelector<HTMLButtonElement>(
        '[aria-label="Show Memos and Todo"]',
      )!;
      const todoOnly = switcher.querySelector<HTMLButtonElement>('[aria-label="Show Todo only"]')!;

      expect(split.getAttribute('data-state')).toBe('on');
      expect(inertState(notes)).toBe(false);
      expect(inertState(todo)).toBe(false);

      notesOnly.click();
      await tick();
      expect(target.querySelector('.workspace-stage')?.classList.contains('layout-notes')).toBe(
        true,
      );
      expect(notesOnly.getAttribute('data-state')).toBe('on');
      expect(inertState(notes)).toBe(false);
      expect(inertState(todo)).toBe(true);
      expect(target.querySelector('.skip-link')?.getAttribute('href')).toBe('#main-content');

      todoOnly.click();
      await tick();
      expect(target.querySelector('.workspace-stage')?.classList.contains('layout-todo')).toBe(
        true,
      );
      expect(todoOnly.getAttribute('data-state')).toBe('on');
      expect(inertState(notes)).toBe(true);
      expect(inertState(todo)).toBe(false);
      expect(target.querySelector('.skip-link')?.getAttribute('href')).toBe('#todo-panel');

      split.click();
      await tick();
      expect(target.querySelector('.workspace-stage')?.classList.contains('layout-split')).toBe(
        true,
      );
      expect(inertState(notes)).toBe(false);
      expect(inertState(todo)).toBe(false);
      expect(target.querySelector('.workspace-column')).toBe(notes);
      expect(target.querySelector('.todo-rail')).toBe(todo);
    } finally {
      await unmount(component);
    }
  });
});

function inertState(element: Element | null): boolean | undefined {
  return (element as (HTMLElement & { inert?: boolean }) | null)?.inert;
}
