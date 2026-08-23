import { createRawSnippet, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listTasks } from '../api/tasks';
import type { SessionInfo } from '../api/types';
import AppShell from './AppShell.svelte';

vi.mock('../api/tasks', () => ({
  createTask: vi.fn(),
  deleteTask: vi.fn(),
  listTasks: vi.fn(),
  updateTask: vi.fn(),
}));

const todayTask = {
  completedAt: null,
  createdAt: '2026-08-23T10:00:00.000Z',
  description: '',
  dueDate: '2026-08-23',
  dueTime: null,
  priority: 0 as const,
  sortKey: 1,
  status: 'TODO' as const,
  title: 'Alpha',
  uid: 'task-alpha',
  updatedAt: '2026-08-23T10:00:00.000Z',
};

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
  it('isolates the notice and preserves the Todo drawer while a native dialog is open', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const children = createRawSnippet(() => ({
      render: () => '<dialog aria-label="Nested confirmation" open></dialog>',
    }));
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
      await vi.waitFor(() =>
        expect(target.querySelector('.todo-rail')?.getAttribute('aria-modal')).toBe('true'),
      );
      expect(target.querySelector('.sidebar .brand')?.getAttribute('aria-label')).toBe(
        'Locus Desk home',
      );
      const workspaceShortcut = target.querySelector<HTMLAnchorElement>(
        '.sidebar .nav-item[title="Workspace"]',
      );
      expect(workspaceShortcut?.getAttribute('aria-current')).toBe('page');
      expect(target.querySelector('.sidebar .nav-item[title="Notes"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Todo"]')).toBeNull();
      const todoButton = [
        ...target.querySelectorAll<HTMLButtonElement>('.compact-topbar button'),
      ].find((button) => button.textContent?.trim() === 'Todo')!;
      todoButton.click();
      await tick();

      expect(target.querySelector('.app-shell')?.classList.contains('todo-open')).toBe(true);
      expect(todoButton.getAttribute('aria-expanded')).toBe('true');
      const notice = target.querySelector<HTMLElement>('.global-notice');
      expect(inertState(notice)).toBe(true);
      expect(notice?.hidden).toBe(true);
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      await tick();
      expect(target.querySelector('.app-shell')?.classList.contains('todo-open')).toBe(true);

      target.querySelector('dialog')?.removeAttribute('open');
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      await tick();
      expect(target.querySelector('.app-shell')?.classList.contains('todo-open')).toBe(false);
      expect(inertState(notice)).toBe(false);
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
      expect(target.querySelector('.sidebar .nav-item[title="Notes"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Tasks"]')).not.toBeNull();
      expect(target.querySelector('.sidebar .nav-item[title="Archive"]')).not.toBeNull();
      expect(target.querySelector('[aria-label="Workspace view"]')).toBeNull();
      const notes = target.querySelector<HTMLElement>('.workspace-column')!;
      const todo = target.querySelector<HTMLElement>('.todo-rail')!;
      const switcher = target.querySelector<HTMLElement>('[aria-label="Workspace layout"]')!;
      const notesOnly = switcher.querySelector<HTMLButtonElement>(
        '[aria-label="Show Notes only"]',
      )!;
      const split = switcher.querySelector<HTMLButtonElement>(
        '[aria-label="Show Notes and Todo"]',
      )!;
      const todoOnly = switcher.querySelector<HTMLButtonElement>('[aria-label="Show Todo only"]')!;

      expect(split.getAttribute('aria-pressed')).toBe('true');
      expect(inertState(notes)).toBe(false);
      expect(inertState(todo)).toBe(false);

      notesOnly.click();
      await tick();
      expect(target.querySelector('.workspace-stage')?.classList.contains('layout-notes')).toBe(
        true,
      );
      expect(notesOnly.getAttribute('aria-pressed')).toBe('true');
      expect(inertState(notes)).toBe(false);
      expect(inertState(todo)).toBe(true);
      expect(target.querySelector('.skip-link')?.getAttribute('href')).toBe('#main-content');

      todoOnly.click();
      await tick();
      expect(target.querySelector('.workspace-stage')?.classList.contains('layout-todo')).toBe(
        true,
      );
      expect(todoOnly.getAttribute('aria-pressed')).toBe('true');
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

  it('lets an open delete dialog own Tab handling inside the Todo drawer', async () => {
    installDialogPolyfill();
    vi.mocked(listTasks).mockResolvedValue({ items: [todayTask] });
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
        expect(target.querySelector('.todo-rail')?.getAttribute('aria-modal')).toBe('true'),
      );
      [...target.querySelectorAll<HTMLButtonElement>('.compact-topbar button')]
        .find((button) => button.textContent?.trim() === 'Todo')
        ?.click();
      await vi.waitFor(() =>
        expect(target.querySelector('.app-shell')?.classList.contains('todo-open')).toBe(true),
      );
      const deleteButton = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Delete Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      deleteButton.click();
      await vi.waitFor(() => expect(target.querySelector('dialog')?.open).toBe(true));

      const confirm = target.querySelector<HTMLButtonElement>('.confirm-dialog .button.danger')!;
      confirm.focus();
      const tab = new KeyboardEvent('keydown', {
        bubbles: true,
        cancelable: true,
        key: 'Tab',
      });
      confirm.dispatchEvent(tab);

      expect(tab.defaultPrevented).toBe(false);
      expect(document.activeElement).toBe(confirm);
      expect(target.querySelector('.app-shell')?.classList.contains('todo-open')).toBe(true);
    } finally {
      await unmount(component);
      uninstallDialogPolyfill();
    }
  });
});

function inertState(element: Element | null): boolean | undefined {
  return (element as (HTMLElement & { inert?: boolean }) | null)?.inert;
}

function installDialogPolyfill(): void {
  Object.defineProperties(HTMLDialogElement.prototype, {
    close: {
      configurable: true,
      value(this: HTMLDialogElement) {
        this.open = false;
      },
    },
    showModal: {
      configurable: true,
      value(this: HTMLDialogElement) {
        this.open = true;
      },
    },
  });
}

function uninstallDialogPolyfill(): void {
  delete (HTMLDialogElement.prototype as Partial<HTMLDialogElement>).close;
  delete (HTMLDialogElement.prototype as Partial<HTMLDialogElement>).showModal;
}
