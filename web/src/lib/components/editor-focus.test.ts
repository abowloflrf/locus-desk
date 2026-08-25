import { mount, unmount } from 'svelte';
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
      const editButton = target.querySelector<HTMLButtonElement>('[aria-label="Edit memo"]');
      editButton?.click();
      await vi.waitFor(() => expect(document.activeElement?.tagName).toBe('TEXTAREA'));
      expect((document.activeElement as HTMLTextAreaElement).style.height).toBe('48px');
      expect(target.querySelector('.note-item')?.classList.contains('editing')).toBe(true);
      expect(target.querySelector('.note-actions')).toBeNull();

      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }),
      );
      await vi.waitFor(() =>
        expect(document.activeElement?.getAttribute('aria-label')).toBe('Edit memo'),
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
      target.querySelector<HTMLButtonElement>('[aria-label="Edit memo"]')?.click();
      const editor = await vi.waitFor(() => {
        const textarea = target.querySelector<HTMLTextAreaElement>('textarea');
        expect(textarea).not.toBeNull();
        return textarea!;
      });
      const markdown = '    indented code\nline with spaces  \n';
      editor.value = markdown;
      editor.dispatchEvent(new Event('input', { bubbles: true }));
      target.querySelector<HTMLButtonElement>('.note-edit-form .button.primary')?.click();

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
      const editButton = target.querySelector<HTMLButtonElement>(
        '[aria-label="Edit A task to edit"]',
      );
      editButton?.click();
      await vi.waitFor(() => expect(document.activeElement?.tagName).toBe('INPUT'));

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
});
