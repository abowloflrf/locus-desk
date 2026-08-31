import { EditorView } from 'codemirror';
import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Note, Task } from '../api/types';
import NoteItem from './NoteItem.svelte';
import TaskRow from './TaskRow.svelte';

const note: Note = {
  content: 'A note to edit',
  createdAt: '2026-08-23T12:00:00.000Z',
  pinned: false,
  status: 'ACTIVE',
  tags: [],
  uid: 'note-1',
  updatedAt: '2026-08-23T12:00:00.000Z',
};

const task: Task = {
  completedAt: null,
  createdAt: '2026-08-23T12:00:00.000Z',
  description: '',
  dueDate: null,
  dueTime: null,
  priority: 0,
  sortKey: 0,
  status: 'TODO',
  title: 'A task to edit',
  uid: 'task-1',
  updatedAt: '2026-08-23T12:00:00.000Z',
};

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('inline editor focus', () => {
  it('moves focus into and back out of the note editor', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NoteItem, {
      props: {
        busy: false,
        note,
        onDelete: vi.fn(),
        onSave: vi.fn().mockResolvedValue(undefined),
        timeZone: 'Asia/Singapore',
      },
      target,
    });

    try {
      (await openAction(target, 'More memo actions', 'Edit memo')).click();
      await vi.waitFor(() =>
        expect(document.activeElement?.classList.contains('cm-content')).toBe(true),
      );
      expect(target.querySelector('.cm-editor')).not.toBeNull();
      expect(document.activeElement?.getAttribute('aria-label')).toBe('Edit memo');
      expect(target.querySelector('.note-item')?.classList.contains('editing')).toBe(true);
      expect(target.querySelector('.note-actions')).toBeNull();

      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }),
      );
      await vi.waitFor(() =>
        expect(document.activeElement?.getAttribute('aria-label')).toBe('More memo actions'),
      );
      expect(target.querySelector('.note-item')?.classList.contains('editing')).toBe(false);
    } finally {
      await unmount(component);
    }
  });

  it('passes the original Markdown string when saving a note', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NoteItem, {
      props: {
        busy: false,
        note,
        onDelete: vi.fn(),
        onSave,
        timeZone: 'Asia/Singapore',
      },
      target,
    });

    try {
      (await openAction(target, 'More memo actions', 'Edit memo')).click();
      const view = await vi.waitFor(() => {
        const editor = target.querySelector<HTMLElement>('.cm-editor');
        expect(editor).not.toBeNull();
        const nextView = EditorView.findFromDOM(editor!);
        expect(nextView).not.toBeNull();
        return nextView!;
      });
      const markdown = '    indented code\nline with spaces  \n';
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: markdown } });
      await tick();
      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', {
          bubbles: true,
          cancelable: true,
          ctrlKey: true,
          key: 'Enter',
        }),
      );

      await vi.waitFor(() => expect(onSave).toHaveBeenCalledOnce());
      expect(onSave).toHaveBeenCalledWith(note, markdown);
    } finally {
      await unmount(component);
    }
  });

  it('moves focus into and back out of the task editor', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskRow, {
      props: {
        busy: false,
        onDelete: vi.fn(),
        onSave: vi.fn().mockResolvedValue(undefined),
        onToggle: vi.fn().mockResolvedValue(undefined),
        task,
        today: '2026-08-23',
      },
      target,
    });

    try {
      target.querySelector<HTMLButtonElement>('[aria-label="Edit A task to edit"]')?.click();
      await vi.waitFor(() => expect(document.activeElement?.tagName).toBe('INPUT'));
      expect(target.querySelector('.task-edit-form')).not.toBeNull();
      expect(document.body.querySelector('.task-editor-sheet')).toBeNull();
      const inlineTitle = document.activeElement as HTMLInputElement;
      expect(inlineTitle.selectionStart).toBe(0);
      expect(inlineTitle.selectionEnd).toBe(inlineTitle.value.length);

      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }),
      );
      await vi.waitFor(() =>
        expect(document.activeElement?.getAttribute('aria-label')).toBe('Edit A task to edit'),
      );
    } finally {
      await unmount(component);
    }
  });

  it('opens and saves the task editor in a bottom sheet on mobile', async () => {
    const visualViewport = Object.assign(new EventTarget(), { height: 844, offsetTop: 0 });
    vi.stubGlobal('innerHeight', 844);
    vi.stubGlobal('visualViewport', visualViewport as unknown as VisualViewport);
    vi.stubGlobal(
      'matchMedia',
      vi.fn(
        (query: string) =>
          ({
            addEventListener: vi.fn(),
            matches: query === '(max-width: 767px)',
            media: query,
            removeEventListener: vi.fn(),
          }) as unknown as MediaQueryList,
      ),
    );
    const onSave = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskRow, {
      props: {
        busy: false,
        onDelete: vi.fn(),
        onSave,
        onToggle: vi.fn().mockResolvedValue(undefined),
        task,
        today: '2026-08-23',
      },
      target,
    });

    try {
      await tick();
      target.querySelector<HTMLButtonElement>('[aria-label="Edit A task to edit"]')?.click();
      const sheet = await vi.waitFor(() => {
        const content = document.body.querySelector<HTMLElement>('.task-editor-sheet');
        expect(content).not.toBeNull();
        return content!;
      });

      expect(sheet.dataset.side).toBe('bottom');
      expect(target.querySelector('.task-edit-form')).toBeNull();
      expect(sheet.textContent).toContain('Edit task');
      expect(sheet.querySelector('textarea[aria-label="Details"]')).not.toBeNull();
      const title = sheet.querySelector<HTMLInputElement>('#task-title-task-1')!;
      expect(document.activeElement).toBe(title);
      expect(title.selectionStart).toBe(title.value.length);
      expect(title.selectionEnd).toBe(title.value.length);

      visualViewport.height = 500;
      visualViewport.dispatchEvent(new Event('resize'));
      await vi.waitFor(() => expect(sheet.style.bottom).toBe('344px'));
      expect(sheet.style.maxHeight).toBe('min(42rem, 450px)');

      title.value = 'Discarded mobile edit';
      title.dispatchEvent(new Event('input', { bubbles: true }));
      [...sheet.querySelectorAll<HTMLButtonElement>('button')]
        .find((button) => button.textContent?.trim() === 'Cancel')
        ?.click();

      await vi.waitFor(() => expect(document.body.querySelector('.task-editor-sheet')).toBeNull());
      expect(onSave).not.toHaveBeenCalled();
      expect(document.activeElement?.getAttribute('aria-label')).toBe('Edit A task to edit');

      target.querySelector<HTMLButtonElement>('[aria-label="Edit A task to edit"]')?.click();
      const reopenedSheet = await vi.waitFor(() => {
        const content = document.body.querySelector<HTMLElement>('.task-editor-sheet');
        expect(content).not.toBeNull();
        return content!;
      });
      const reopenedTitle = reopenedSheet.querySelector<HTMLInputElement>('#task-title-task-1')!;
      expect(reopenedTitle.value).toBe('A task to edit');

      reopenedTitle.value = 'Updated on mobile';
      reopenedTitle.dispatchEvent(new Event('input', { bubbles: true }));
      [...reopenedSheet.querySelectorAll<HTMLButtonElement>('button')]
        .find((button) => button.textContent?.trim() === 'Save')
        ?.click();

      await vi.waitFor(() => expect(onSave).toHaveBeenCalledOnce());
      expect(onSave).toHaveBeenCalledWith(
        task,
        expect.objectContaining({ title: 'Updated on mobile' }),
      );
      await vi.waitFor(() => expect(document.body.querySelector('.task-editor-sheet')).toBeNull());
      expect(document.activeElement?.getAttribute('aria-label')).toBe('Edit A task to edit');
    } finally {
      await unmount(component);
    }
  });

  it('moves from the task title to the details without submitting on Enter', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskRow, {
      props: {
        busy: false,
        onDelete: vi.fn(),
        onSave,
        onToggle: vi.fn().mockResolvedValue(undefined),
        task,
        today: '2026-08-23',
      },
      target,
    });

    try {
      target.querySelector<HTMLButtonElement>('[aria-label="Edit A task to edit"]')?.click();
      await vi.waitFor(() => expect(document.activeElement?.tagName).toBe('INPUT'));

      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', {
          bubbles: true,
          cancelable: true,
          isComposing: true,
          key: 'Enter',
        }),
      );
      expect(document.activeElement?.tagName).toBe('INPUT');

      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Enter' }),
      );

      expect(document.activeElement?.tagName).toBe('TEXTAREA');
      expect(onSave).not.toHaveBeenCalled();
    } finally {
      await unmount(component);
    }
  });

  it('auto-saves a task when focus leaves the expanded editor', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    const outside = document.createElement('button');
    document.body.append(target, outside);
    const component = mount(TaskRow, {
      props: {
        busy: false,
        onDelete: vi.fn(),
        onSave,
        onToggle: vi.fn().mockResolvedValue(undefined),
        task: { ...task, description: 'Existing details' },
        today: '2026-08-23',
      },
      target,
    });

    try {
      target.querySelector<HTMLButtonElement>('[aria-label="Edit A task to edit"]')?.click();
      const title = await vi.waitFor(() => {
        const input = target.querySelector<HTMLInputElement>('#task-title-task-1');
        expect(input).not.toBeNull();
        return input!;
      });
      title.value = 'Updated task';
      title.dispatchEvent(new Event('input', { bubbles: true }));
      outside.focus();

      await vi.waitFor(() => expect(onSave).toHaveBeenCalledOnce());
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({ uid: 'task-1' }),
        expect.objectContaining({ description: 'Existing details', title: 'Updated task' }),
      );
      expect(document.activeElement).toBe(outside);
    } finally {
      await unmount(component);
    }
  });
});

async function openAction(
  target: HTMLElement,
  triggerLabel: string,
  actionLabel: string,
): Promise<HTMLElement> {
  const trigger = target.querySelector<HTMLButtonElement>(`[aria-label="${triggerLabel}"]`)!;
  trigger.click();
  return vi.waitFor(() => {
    const action = document.body.querySelector<HTMLElement>(`[aria-label="${actionLabel}"]`);
    expect(action).not.toBeNull();
    return action!;
  });
}
