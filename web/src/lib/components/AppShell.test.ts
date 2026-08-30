import { createRawSnippet, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listTasks } from '../api/tasks';
import type { SessionInfo } from '../api/types';
import AppShell from './AppShell.svelte';
import shellSource from './AppShell.svelte?raw';
import sidebarSource from './Sidebar.svelte?raw';
import toggleGroupSource from './ui/toggle-group/toggle-group.svelte?raw';

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

  it('keeps mobile navigation focused on Memos, Library, and Tasks', async () => {
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
      expect(target.querySelector('.mobile-more-trigger')).toBeNull();
      expect(target.textContent).not.toContain('More');
      expect(target.querySelector('.nav-item[title="Workspace"]')).toBeNull();
      expect(target.querySelector('.nav-item[title="Memos"]')).not.toBeNull();
      expect(target.querySelector('.nav-item[title="Library"]')).not.toBeNull();
      expect(target.querySelector('.nav-item[title="Tasks"]')).not.toBeNull();
      expect(target.querySelector('.nav-item[title="Archive"]')).toBeNull();
      expect(sidebarSource).toContain('grid-template-columns: repeat(3, minmax(0, 1fr))');
    } finally {
      await unmount(component);
    }
  });

  it('isolates the notice while the compact Todo sheet is open', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(
        (query: string) =>
          ({
            addEventListener: vi.fn(),
            matches: query === '(max-width: 1199px)',
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

  it('redirects the mobile Workspace route to Memos', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({ render: () => '<h1>Workspace</h1>' }));
    const onNavigate = vi.fn();
    const component = mount(AppShell, {
      props: {
        children,
        current: 'home',
        onDismissNotice: vi.fn(),
        onLogout: vi.fn(),
        onNavigate,
        session,
      },
      target,
    });

    try {
      await vi.waitFor(() => expect(onNavigate).toHaveBeenCalledWith('notes', true));
      expect(target.querySelector('.nav-item[title="Workspace"]')).toBeNull();
      expect(target.querySelector('.todo-trigger')).toBeNull();
      expect(document.body.querySelector('[data-slot="sheet-content"]')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('hides the mobile top bar on downward scroll and restores it on upward scroll', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({ render: () => '<h1>Memos</h1>' }));
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
      const scroller = target.querySelector<HTMLElement>('.workspace-column')!;
      const topbar = target.querySelector('.compact-topbar')!;

      scroller.scrollTop = 80;
      scroller.dispatchEvent(new Event('scroll'));
      await tick();
      expect(topbar.classList).toContain('topbar-hidden');

      scroller.scrollTop = 32;
      scroller.dispatchEvent(new Event('scroll'));
      await tick();
      expect(topbar.classList).not.toContain('topbar-hidden');
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

      expect(switcher.classList).toContain('absolute');
      expect(toggleGroupSource).not.toMatch(
        /\[data-variant='workspace'\]\)\s*\{[^}]*position:\s*relative/s,
      );
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
