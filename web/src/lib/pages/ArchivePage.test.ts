import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { listNotes, updateNote } from '../api/notes';
import type { Note, SessionInfo } from '../api/types';
import ArchivePage from './ArchivePage.svelte';

vi.mock('../api/notes', () => ({
  deleteNote: vi.fn(),
  listNotes: vi.fn(),
  updateNote: vi.fn(),
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

function archivedNote(index: number): Note {
  return {
    content: `Archived note ${index}`,
    createdAt: new Date(Date.UTC(2026, 7, 23, 12, index)).toISOString(),
    pinned: false,
    status: 'ARCHIVED',
    tags: [],
    uid: `note-${index}`,
    updatedAt: new Date(Date.UTC(2026, 7, 23, 12, index)).toISOString(),
  };
}

afterEach(() => {
  vi.clearAllMocks();
  document.body.replaceChildren();
});

describe('ArchivePage', () => {
  it('loads archived notes after the first page', async () => {
    const listNotesMock = vi.mocked(listNotes);
    listNotesMock
      .mockResolvedValueOnce({
        items: Array.from({ length: 30 }, (_, index) => archivedNote(index + 1)),
        page: 1,
        pageSize: 30,
        total: 31,
      })
      .mockResolvedValueOnce({
        items: [archivedNote(31)],
        page: 2,
        pageSize: 30,
        total: 31,
      });

    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(ArchivePage, { props: { session }, target });

    try {
      await vi.waitFor(() => expect(listNotesMock).toHaveBeenCalledTimes(1));
      expect(listNotesMock.mock.calls[0]?.[0]).toMatchObject({
        page: 1,
        pageSize: 30,
        status: 'ARCHIVED',
      });

      const loadMore = [...target.querySelectorAll('button')].find((button) =>
        button.textContent?.includes('Load older notes'),
      );
      expect(loadMore).toBeDefined();
      loadMore?.click();

      await vi.waitFor(() => expect(listNotesMock).toHaveBeenCalledTimes(2));
      expect(listNotesMock.mock.calls[1]?.[0]).toMatchObject({
        page: 2,
        pageSize: 30,
        status: 'ARCHIVED',
      });
      await vi.waitFor(() => expect(target.textContent).toContain('Archived note 31'));
    } finally {
      await unmount(component);
    }
  });

  it('moves focus to the adjacent note and announces a successful restore', async () => {
    const first = archivedNote(1);
    const second = archivedNote(2);
    vi.mocked(listNotes)
      .mockResolvedValueOnce({ items: [first, second], page: 1, pageSize: 30, total: 2 })
      .mockResolvedValue({ items: [second], page: 1, pageSize: 30, total: 1 });
    vi.mocked(updateNote).mockResolvedValue({ ...first, status: 'ACTIVE' });

    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(ArchivePage, { props: { session }, target });

    try {
      const restore = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>(
          '[data-focus-uid="note-1"] [aria-label="Restore note"]',
        );
        expect(button).not.toBeNull();
        return button!;
      });
      restore.focus();
      restore.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('note-2'),
      );
      await vi.waitFor(() =>
        expect(target.querySelector('[data-action-status]')?.textContent).toContain(
          'Note restored.',
        ),
      );
    } finally {
      await unmount(component);
    }
  });

  it('focuses the adjacent note when an edit no longer matches the archive search', async () => {
    const first = archivedNote(1);
    const second = archivedNote(2);
    vi.mocked(listNotes)
      .mockResolvedValueOnce({ items: [first, second], page: 1, pageSize: 30, total: 2 })
      .mockResolvedValueOnce({ items: [first, second], page: 1, pageSize: 30, total: 2 })
      .mockResolvedValue({ items: [second], page: 1, pageSize: 30, total: 1 });
    vi.mocked(updateNote).mockResolvedValue({ ...first, content: 'Different content' });

    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(ArchivePage, { props: { session }, target });

    try {
      await vi.waitFor(() => expect(target.textContent).toContain('Archived note 1'));
      const search = target.querySelector<HTMLInputElement>('[placeholder="Search archive"]')!;
      search.value = 'Archived';
      search.dispatchEvent(new Event('input', { bubbles: true }));
      await vi.waitFor(() => expect(listNotes).toHaveBeenCalledTimes(2), { timeout: 1_000 });
      target
        .querySelector<HTMLButtonElement>('[data-focus-uid="note-1"] [aria-label="Edit note"]')
        ?.click();
      const editor = await vi.waitFor(() => {
        const textarea = target.querySelector<HTMLTextAreaElement>('#edit-note-note-1');
        expect(textarea).not.toBeNull();
        return textarea!;
      });
      editor.value = 'Different content';
      editor.dispatchEvent(new Event('input', { bubbles: true }));
      target.querySelector<HTMLButtonElement>('[data-focus-uid="note-1"] .button.primary')?.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('note-2'),
      );
      expect(target.querySelector('[data-focus-uid="note-1"]')).toBeNull();
    } finally {
      await unmount(component);
    }
  });
});
