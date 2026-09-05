import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createTask, deleteTask, listTasks, updateTask } from '../api/tasks';
import type { Task } from '../api/types';
import TaskBoard from './TaskBoard.svelte';

vi.mock('../api/tasks', () => ({
  createTask: vi.fn(),
  deleteTask: vi.fn(),
  listTasks: vi.fn(),
  updateTask: vi.fn(),
}));
afterEach(() => {
  vi.clearAllMocks();
  document.body.replaceChildren();
});

function task(uid: string, overrides: Partial<Task> = {}): Task {
  return {
    uid,
    title: uid,
    description: '',
    dueDate: '2026-09-05',
    dueTime: null,
    priority: 0,
    status: 'TODO',
    sortKey: 1,
    completedAt: null,
    createdAt: '2026-09-05T10:00:00Z',
    updatedAt: '2026-09-05T10:00:00Z',
    ...overrides,
  };
}
async function setup(items: Task[], mode: 'todo' | 'all' = 'all') {
  vi.mocked(listTasks).mockResolvedValue({ items });
  const target = document.createElement('div');
  document.body.append(target);
  const component = mount(TaskBoard, { target, props: { mode, today: '2026-09-05' } });
  await vi.waitFor(() => expect(target.querySelector('[data-focus-uid]')).not.toBeNull());
  return { target, component };
}
function button(label: string, root: ParentNode = document.body) {
  const result = root.querySelector<HTMLButtonElement>(`[aria-label="${label}"]`);
  expect(result, label).not.toBeNull();
  return result!;
}
async function textButton(text: string) {
  return vi.waitFor(() => {
    const result = [...document.body.querySelectorAll<HTMLButtonElement>('button')].find(
      (b) => b.textContent?.trim() === text || b.getAttribute('aria-label') === text,
    );
    expect(result, text).toBeDefined();
    return result!;
  });
}
async function editTitle(target: HTMLElement, uid: string, value: string) {
  button(`Edit ${uid}`, target).click();
  await tick();
  const input = target.querySelector<HTMLInputElement>(`#task-title-${uid}`)!;
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
  return input;
}

describe('Task list redesign', () => {
  it('keeps all open Todo tasks in one list and marks only high priority', async () => {
    const { target, component } = await setup(
      [
        task('Alpha'),
        task('Later', { dueDate: '2026-09-07', priority: 1 }),
        task('Undated', { dueDate: null, description: 'A private note' }),
      ],
      'todo',
    );
    try {
      expect(listTasks).toHaveBeenCalledWith({ status: 'TODO' }, expect.any(AbortSignal));
      expect(target.querySelectorAll('[data-focus-uid]')).toHaveLength(3);
      expect(target.querySelector('[aria-label="High priority"]')).not.toBeNull();
      expect(target.textContent).not.toContain('Regular');
      expect(target.textContent).not.toContain('A private note');
      button('Details for Undated', target).click();
      await vi.waitFor(() =>
        expect(document.body.querySelector<HTMLTextAreaElement>('textarea')?.value).toBe(
          'A private note',
        ),
      );
    } finally {
      await unmount(component);
    }
  });

  it('saves just the title and preserves legacy details, priority and time', async () => {
    const original = task('Alpha', { dueTime: '16:30', description: 'Keep details', priority: 1 });
    vi.mocked(updateTask).mockResolvedValue({ ...original, title: 'Renamed' });
    const { target, component } = await setup([original]);
    try {
      expect(target.querySelector('[aria-label="Sort tasks"]')).toBeNull();
      const input = await editTitle(target, 'Alpha', 'Renamed');
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
      );
      await vi.waitFor(() =>
        expect(updateTask).toHaveBeenCalledExactlyOnceWith('Alpha', { title: 'Renamed' }),
      );
      await vi.waitFor(() => expect(document.activeElement).toBe(button('Edit Renamed', target)));
      expect(target.textContent).toContain('Renamed');
    } finally {
      await unmount(component);
    }
  });

  it('cancels with Escape, ignores composition Enter, and keeps failed edits for retry', async () => {
    vi.mocked(updateTask)
      .mockRejectedValueOnce(new Error('Offline'))
      .mockResolvedValueOnce(task('Alpha', { title: 'Keep draft' }));
    const { target, component } = await setup([task('Alpha')]);
    try {
      let input = await editTitle(target, 'Alpha', 'Discard draft');
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
      );
      await tick();
      expect(updateTask).not.toHaveBeenCalled();
      input = await editTitle(target, 'Alpha', 'Keep draft');
      input.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Enter',
          isComposing: true,
          bubbles: true,
          cancelable: true,
        }),
      );
      expect(updateTask).not.toHaveBeenCalled();
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }),
      );
      await vi.waitFor(() => expect(target.textContent).toContain('Offline'));
      expect(input.value).toBe('Keep draft');
      input.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Enter',
          ctrlKey: true,
          bubbles: true,
          cancelable: true,
        }),
      );
      await vi.waitFor(() => expect(target.querySelector('#task-title-Alpha')).toBeNull());
      expect(updateTask).toHaveBeenCalledTimes(2);
    } finally {
      await unmount(component);
    }
  });

  it('changes dates in place and clears legacy time only when removing the date', async () => {
    const original = task('Alpha', { dueTime: '16:30' });
    vi.mocked(updateTask)
      .mockResolvedValueOnce({ ...original, dueDate: '2026-09-06' })
      .mockResolvedValueOnce({ ...original, dueDate: null, dueTime: null });
    const { target, component } = await setup([original], 'todo');
    try {
      button('Target date and priority for Alpha', target).click();
      (await textButton('Tomorrow')).click();
      await vi.waitFor(() =>
        expect(updateTask).toHaveBeenCalledWith('Alpha', { dueDate: '2026-09-06' }),
      );
      await vi.waitFor(() => expect(target.textContent).toContain('Tomorrow'));
      const noDate = await textButton('No date');
      await vi.waitFor(() => expect(noDate.disabled).toBe(false));
      noDate.click();
      await vi.waitFor(() =>
        expect(updateTask).toHaveBeenLastCalledWith('Alpha', { dueDate: null, dueTime: null }),
      );
      await vi.waitFor(() => expect(target.querySelector('.task-date')).toBeNull());
      expect(target.querySelector('[data-focus-uid="Alpha"]')).not.toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('opens a real calendar and leaves a failed priority change visibly unchecked', async () => {
    vi.mocked(updateTask)
      .mockRejectedValueOnce(new Error('Cannot save priority'))
      .mockResolvedValueOnce(task('Alpha', { dueDate: '2026-09-15' }));
    const { target, component } = await setup([task('Alpha')]);
    try {
      button('Target date and priority for Alpha', target).click();
      const priority = await vi.waitFor(() => {
        const node = document.body.querySelector<HTMLButtonElement>(
          '[data-slot="toggle-group-item"][aria-label="High priority"]',
        );
        expect(node).not.toBeNull();
        return node!;
      });
      priority.click();
      await vi.waitFor(() => expect(document.body.textContent).toContain('Cannot save priority'));
      expect(priority.getAttribute('data-state')).toBe('off');
      expect(priority.textContent?.trim()).toBe('');
      expect(updateTask).toHaveBeenCalledWith('Alpha', { priority: 1 });
      (await textButton('Choose date')).click();
      const day = await vi.waitFor(() => {
        const node = document.body.querySelector<HTMLButtonElement>(
          '[data-bits-day][data-value="2026-09-15"]',
        );
        expect(node).not.toBeNull();
        return node!;
      });
      day.click();
      await vi.waitFor(() =>
        expect(updateTask).toHaveBeenLastCalledWith('Alpha', { dueDate: '2026-09-15' }),
      );
      await vi.waitFor(() => expect(target.textContent).toContain('Sep 15'));
    } finally {
      await unmount(component);
    }
  });

  it('completes and restores tasks with focus across a collapsible completed group', async () => {
    const alpha = task('Alpha');
    vi.mocked(updateTask)
      .mockResolvedValueOnce({ ...alpha, status: 'DONE' })
      .mockResolvedValueOnce(alpha);
    const { target, component } = await setup([alpha, task('Beta')]);
    try {
      button('Complete Alpha', target).focus();
      button('Complete Alpha', target).click();
      await vi.waitFor(() => expect(document.activeElement).toBe(button('Restore Alpha', target)));
      const collapse = target.querySelector<HTMLButtonElement>('.completed-toggle')!;
      collapse.click();
      await tick();
      expect(target.querySelector('[aria-label="Restore Alpha"]')).toBeNull();
      collapse.click();
      await tick();
      button('Restore Alpha', target).click();
      await vi.waitFor(() => expect(document.activeElement).toBe(button('Complete Alpha', target)));
      expect(target.querySelector('.completed-toggle')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('rolls back a failed completion in Todo and returns focus to the restored task', async () => {
    vi.mocked(updateTask).mockRejectedValue(new Error('Offline'));
    const { target, component } = await setup([task('Alpha'), task('Beta')], 'todo');
    try {
      button('Complete Alpha', target).focus();
      button('Complete Alpha', target).click();
      await vi.waitFor(() => expect(target.textContent).toContain('Offline'));
      await vi.waitFor(() => expect(document.activeElement).toBe(button('Complete Alpha', target)));
      expect(target.querySelectorAll('[data-focus-uid]')).toHaveLength(2);
    } finally {
      await unmount(component);
    }
  });

  it('keeps delete outside the date menu, confirms deletion and focuses the neighbor', async () => {
    vi.mocked(deleteTask).mockResolvedValue(undefined);
    const { target, component } = await setup([task('Alpha'), task('Beta')]);
    try {
      button('Details for Alpha', target).click();
      const remove = await vi.waitFor(() => button('Delete Alpha'));
      remove.click();
      const confirm = await vi.waitFor(() => {
        const node = document.body.querySelector<HTMLButtonElement>(
          '[data-slot="alert-dialog-action"]',
        );
        expect(node).not.toBeNull();
        return node!;
      });
      expect(deleteTask).not.toHaveBeenCalled();
      confirm.click();
      await vi.waitFor(() => expect(deleteTask).toHaveBeenCalledWith('Alpha'));
      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('Beta'),
      );
    } finally {
      await unmount(component);
    }
  });
});
