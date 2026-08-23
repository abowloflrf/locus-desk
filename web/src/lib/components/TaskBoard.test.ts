import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { createTask, deleteTask, listTasks, updateTask } from '../api/tasks';
import type { Task, TaskStatus } from '../api/types';
import TaskBoard from './TaskBoard.svelte';

vi.mock('../api/tasks', () => ({
  createTask: vi.fn(),
  deleteTask: vi.fn(),
  listTasks: vi.fn(),
  updateTask: vi.fn(),
}));

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('TaskBoard focus and filtering', () => {
  it('sends only one create request for synchronous duplicate submissions', async () => {
    const pending = deferred<Task>();
    vi.mocked(listTasks).mockResolvedValue({ items: [] });
    vi.mocked(createTask).mockReturnValue(pending.promise);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskBoard, {
      props: { mode: 'all', today: '2026-08-23' },
      target,
    });

    try {
      const title = target.querySelector<HTMLInputElement>('#new-task-all')!;
      title.value = 'Only once';
      title.dispatchEvent(new Event('input', { bubbles: true }));
      const form = target.querySelector<HTMLFormElement>('.task-create')!;
      const submit = target.querySelector<HTMLButtonElement>('.task-add-button')!;
      await vi.waitFor(() => expect(submit.disabled).toBe(false));

      form.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }));
      form.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }));

      expect(createTask).toHaveBeenCalledOnce();
      await tick();
      expect(submit.disabled).toBe(true);
      pending.resolve(task('created', 'Only once'));
      await vi.waitFor(() => expect(title.value).toBe(''));
    } finally {
      await unmount(component);
    }
  });

  it('keeps focus on a task moved between groups and announces the change', async () => {
    const alpha = task('alpha', 'Alpha');
    const beta = task('beta', 'Beta');
    vi.mocked(listTasks).mockResolvedValue({ items: [alpha, beta] });
    vi.mocked(updateTask).mockResolvedValue({ ...alpha, status: 'DONE' });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskBoard, {
      props: { mode: 'all', today: '2026-08-23' },
      target,
    });

    try {
      const toggle = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Complete Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      toggle.focus();
      toggle.click();

      await vi.waitFor(() =>
        expect(document.activeElement?.outerHTML).toContain('aria-label="Restore Alpha"'),
      );
      expect(
        document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
      ).toBe('alpha');
      await vi.waitFor(() =>
        expect(target.querySelector('[data-action-status]')?.textContent).toContain(
          'Task completed: Alpha.',
        ),
      );
    } finally {
      await unmount(component);
    }
  });

  it('removes an edited task that moves beyond Today and focuses its neighbor', async () => {
    const alpha = task('alpha', 'Alpha');
    const beta = task('beta', 'Beta');
    vi.mocked(listTasks).mockResolvedValue({ items: [alpha, beta] });
    vi.mocked(updateTask).mockResolvedValue({ ...alpha, dueDate: '2026-08-24' });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskBoard, {
      props: { mode: 'today', today: '2026-08-23' },
      target,
    });

    try {
      const edit = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Edit Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      edit.click();
      const dueDate = await vi.waitFor(() => {
        const input = target.querySelector<HTMLInputElement>(
          '[data-focus-uid="alpha"] input[type="date"]',
        );
        expect(input).not.toBeNull();
        return input!;
      });
      dueDate.value = '2026-08-24';
      dueDate.dispatchEvent(new Event('input', { bubbles: true }));
      target
        .querySelector<HTMLFormElement>('[data-focus-uid="alpha"] .task-edit-form')
        ?.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }));

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('beta'),
      );
      expect(target.querySelector('[data-focus-uid="alpha"]')).toBeNull();
      expect(target.querySelector('[data-action-status]')?.textContent).toContain(
        'Task moved out of Today: Alpha.',
      );
    } finally {
      await unmount(component);
    }
  });

  it('removes a future completed task when it is restored in Today', async () => {
    const alpha = {
      ...task('alpha', 'Alpha', 'DONE'),
      dueDate: '2026-08-24',
    };
    const beta = task('beta', 'Beta');
    vi.mocked(listTasks).mockResolvedValue({ items: [alpha, beta] });
    vi.mocked(updateTask).mockResolvedValue({
      ...alpha,
      completedAt: null,
      status: 'TODO',
    });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskBoard, {
      props: { mode: 'today', today: '2026-08-23' },
      target,
    });

    try {
      const restore = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Restore Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      restore.focus();
      restore.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('beta'),
      );
      expect(target.querySelector('[data-focus-uid="alpha"]')).toBeNull();
      expect(target.querySelector('[data-action-status]')?.textContent).toContain(
        'Task restored and moved out of Today: Alpha.',
      );
    } finally {
      await unmount(component);
    }
  });

  it('focuses the adjacent task and announces a successful delete', async () => {
    installDialogPolyfill();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    const alpha = task('alpha', 'Alpha');
    const beta = task('beta', 'Beta');
    vi.mocked(listTasks).mockResolvedValue({ items: [alpha, beta] });
    vi.mocked(deleteTask).mockResolvedValue();
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskBoard, {
      props: { mode: 'all', today: '2026-08-23' },
      target,
    });

    try {
      const deleteButton = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Delete Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      deleteButton.focus();
      deleteButton.click();
      await vi.waitFor(() => expect(target.querySelector('dialog')?.open).toBe(true));
      target.querySelector<HTMLButtonElement>('.confirm-dialog .button.danger')?.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('beta'),
      );
      expect(target.querySelector('[data-action-status]')?.textContent).toContain(
        'Task deleted: Alpha.',
      );
    } finally {
      await unmount(component);
      uninstallDialogPolyfill();
    }
  });

  it('clears stale rows and marks the board busy when the status filter changes', async () => {
    const openTask = task('open', 'Open task');
    const completedTask = task('done', 'Completed task', 'DONE');
    const pending = deferred<{ items: Task[] }>();
    vi.mocked(listTasks)
      .mockResolvedValueOnce({ items: [openTask] })
      .mockReturnValueOnce(pending.promise);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskBoard, {
      props: { mode: 'all', today: '2026-08-23' },
      target,
    });

    try {
      await vi.waitFor(() => expect(target.textContent).toContain('Open task'));
      const completedFilter = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Completed',
      )!;
      completedFilter.click();
      await tick();

      expect(completedFilter.getAttribute('aria-pressed')).toBe('true');
      expect(target.textContent).not.toContain('Open task');
      expect(target.querySelector('.task-board')?.getAttribute('aria-busy')).toBe('true');
      expect(target.textContent).toContain('Loading tasks…');

      pending.resolve({ items: [completedTask] });
      await vi.waitFor(() => expect(target.textContent).toContain('Completed task'));
      expect(target.querySelector('.task-board')?.getAttribute('aria-busy')).toBe('false');
    } finally {
      await unmount(component);
    }
  });
});

function task(uid: string, title: string, status: TaskStatus = 'TODO'): Task {
  return {
    completedAt: status === 'DONE' ? '2026-08-23T12:00:00.000Z' : null,
    createdAt: '2026-08-23T10:00:00.000Z',
    description: '',
    dueDate: '2026-08-23',
    dueTime: null,
    priority: 0,
    sortKey: 1,
    status,
    title,
    uid,
    updatedAt: '2026-08-23T10:00:00.000Z',
  };
}

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
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
