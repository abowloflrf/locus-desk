import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { createNote, deleteNote, listNotes, listTags, updateNote } from '../api/notes';
import type { ListNotesResponse, Note, SessionInfo } from '../api/types';
import NotesPage from './NotesPage.svelte';

vi.mock('../api/notes', () => ({
  createNote: vi.fn(),
  deleteNote: vi.fn(),
  listNotes: vi.fn(),
  listTags: vi.fn(),
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

function note(content: string, tags: string[] = []): Note {
  return {
    content,
    createdAt: '2026-08-23T12:00:00.000Z',
    pinned: false,
    status: 'ACTIVE',
    tags,
    uid: content.toLocaleLowerCase().replaceAll(' ', '-'),
    updatedAt: '2026-08-23T12:00:00.000Z',
  };
}

function page(items: Note[]): ListNotesResponse {
  return { items, page: 1, pageSize: 30, total: items.length };
}

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('NotesPage request ordering', () => {
  it('keeps overflow tags reachable and shows the selected filter in the toolbar', async () => {
    vi.mocked(listNotes).mockResolvedValue(page([]));
    vi.mocked(listTags).mockResolvedValue({ items: ['alpha', 'beta', 'gamma', 'delta'] });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      await vi.waitFor(() => expect(target.textContent).toContain('#alpha'));
      expect(target.querySelector('.tag-toolbar')?.textContent).not.toContain('#delta');
      target.querySelector<HTMLButtonElement>('[aria-label="All tags"]')!.click();
      const option = await vi.waitFor(() => {
        const entry = [...document.querySelectorAll<HTMLElement>('[role="menuitemradio"]')].find(
          (entry) => entry.textContent?.trim() === '#delta',
        );
        expect(entry).toBeDefined();
        return entry!;
      });
      option.click();
      await vi.waitFor(() =>
        expect(listNotes).toHaveBeenLastCalledWith(
          expect.objectContaining({ tag: 'delta', page: 1 }),
          expect.any(AbortSignal),
        ),
      );
      await vi.waitFor(() =>
        expect(target.querySelector('.tag-filter [data-state="on"]')?.textContent).toContain(
          '#delta',
        ),
      );
      target.querySelector<HTMLButtonElement>('.tag-filter [data-state="on"]')!.click();
      await vi.waitFor(() =>
        expect(listNotes).toHaveBeenLastCalledWith(
          expect.objectContaining({ tag: '' }),
          expect.any(AbortSignal),
        ),
      );
    } finally {
      await unmount(component);
    }
  });

  it('invalidates an in-flight list as soon as the search changes', async () => {
    let resolveStale: ((response: ListNotesResponse) => void) | undefined;
    const stale = new Promise<ListNotesResponse>((resolve) => {
      resolveStale = resolve;
    });
    vi.mocked(listNotes)
      .mockReturnValueOnce(stale)
      .mockResolvedValue(page([note('Fresh result')]));
    vi.mocked(listTags).mockResolvedValue({ items: [] });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      await vi.waitFor(() => expect(listNotes).toHaveBeenCalledOnce());
      const firstSignal = vi.mocked(listNotes).mock.calls[0]?.[1];
      const search = target.querySelector<HTMLInputElement>('[placeholder="Search memos"]')!;
      search.value = 'fresh';
      search.dispatchEvent(new Event('input', { bubbles: true }));
      expect(firstSignal?.aborted).toBe(true);

      await vi.waitFor(() => expect(listNotes).toHaveBeenCalledTimes(2), { timeout: 1_000 });
      await vi.waitFor(() => expect(target.textContent).toContain('Fresh result'));
      resolveStale?.(page([note('Stale result')]));
      await Promise.resolve();
      expect(target.textContent).not.toContain('Stale result');
    } finally {
      await unmount(component);
    }
  });

  it('does not let an older tag response replace post-mutation tags', async () => {
    let resolveStaleTags: ((response: { items: string[] }) => void) | undefined;
    const staleTags = new Promise<{ items: string[] }>((resolve) => {
      resolveStaleTags = resolve;
    });
    vi.mocked(listNotes).mockResolvedValue(page([]));
    vi.mocked(listTags)
      .mockReturnValueOnce(staleTags)
      .mockResolvedValueOnce({ items: ['new'] });
    vi.mocked(createNote).mockResolvedValue(note('Created #new', ['new']));
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      await vi.waitFor(() => expect(listTags).toHaveBeenCalledOnce());
      const textarea = target.querySelector<HTMLTextAreaElement>('#note-composer-input')!;
      textarea.value = 'Created #new';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
      const submit = target.querySelector<HTMLButtonElement>('.composer-submit')!;
      await vi.waitFor(() => expect(submit.disabled).toBe(false));
      submit.click();

      await vi.waitFor(() => expect(listTags).toHaveBeenCalledTimes(2));
      await vi.waitFor(() => expect(target.textContent).toContain('#new'));
      resolveStaleTags?.({ items: ['old'] });
      await Promise.resolve();
      expect(target.textContent).not.toContain('#old');
    } finally {
      await unmount(component);
    }
  });

  it('keeps delete failures inside the confirmation dialog and clears them when reopened', async () => {
    installDialogPolyfill();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.mocked(listNotes).mockResolvedValue(page([note('Keep me')]));
    vi.mocked(listTags).mockResolvedValue({ items: [] });
    vi.mocked(deleteNote).mockRejectedValue(new Error('Delete failed.'));
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      (await openNoteAction(target, 'keep-me', 'Delete memo')).click();
      await waitForDialog(true);
      document.body.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')?.click();

      await vi.waitFor(() =>
        expect(document.body.querySelector('[role="alert"]')?.textContent).toContain(
          'Delete failed.',
        ),
      );
      expect(document.body.querySelector('[role="alert"]')?.getAttribute('aria-live')).toBe(
        'assertive',
      );

      document.body.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')?.click();
      await waitForDialog(false);
      (await openNoteAction(target, 'keep-me', 'Delete memo')).click();
      await waitForDialog(true);
      expect(document.body.querySelector('[role="alert"]')).toBeNull();
    } finally {
      await unmount(component);
      uninstallDialogPolyfill();
    }
  });

  it('focuses the adjacent note and announces a successful delete', async () => {
    installDialogPolyfill();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    const first = note('Delete me');
    const second = note('Keep me');
    vi.mocked(listNotes)
      .mockResolvedValueOnce(page([first, second]))
      .mockResolvedValue(page([second]));
    vi.mocked(listTags).mockResolvedValue({ items: [] });
    vi.mocked(deleteNote).mockResolvedValue();
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      const deleteButton = await openNoteAction(target, 'delete-me', 'Delete memo');
      deleteButton.focus();
      deleteButton.click();
      await waitForDialog(true);
      document.body.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')?.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('keep-me'),
      );
      expect(target.querySelector('[data-action-status]')?.textContent).toContain('Memo deleted.');
    } finally {
      await unmount(component);
      uninstallDialogPolyfill();
    }
  });

  it('focuses the adjacent note when an edit no longer matches the active tag', async () => {
    const first = note('First work', ['work']);
    const second = note('Second work', ['work']);
    vi.mocked(listNotes)
      .mockResolvedValueOnce(page([first, second]))
      .mockResolvedValueOnce(page([first, second]))
      .mockResolvedValue(page([second]));
    vi.mocked(listTags).mockResolvedValue({ items: ['work'] });
    vi.mocked(updateNote).mockResolvedValue({
      ...first,
      content: 'No longer tagged',
      tags: [],
    });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      const tagFilter = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('.tag-filter button:last-child');
        expect(button?.textContent).toContain('#work');
        return button!;
      });
      tagFilter.click();
      await vi.waitFor(() => expect(tagFilter.dataset.state).toBe('on'));
      await vi.waitFor(() => expect(listNotes).toHaveBeenCalledTimes(2));
      (await openNoteAction(target, 'first-work', 'Edit memo')).click();
      const editor = await vi.waitFor(() => {
        const textarea = target.querySelector<HTMLTextAreaElement>('#edit-note-first-work');
        expect(textarea).not.toBeNull();
        return textarea!;
      });
      editor.value = 'No longer tagged';
      editor.dispatchEvent(new Event('input', { bubbles: true }));
      target
        .querySelector<HTMLButtonElement>(
          '[data-focus-uid="first-work"] .note-edit-form button:last-child',
        )
        ?.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('second-work'),
      );
      expect(target.querySelector('[data-focus-uid="first-work"]')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('moves focus to the adjacent note and announces a successful archive', async () => {
    const first = note('First note');
    const second = note('Second note');
    vi.mocked(listNotes)
      .mockResolvedValueOnce(page([first, second]))
      .mockResolvedValue(page([second]));
    vi.mocked(listTags).mockResolvedValue({ items: [] });
    vi.mocked(updateNote).mockResolvedValue({ ...first, status: 'ARCHIVED' });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NotesPage, { props: { session }, target });

    try {
      const archive = await openNoteAction(target, 'first-note', 'Archive memo');
      archive.focus();
      archive.click();

      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('second-note'),
      );
      await vi.waitFor(() =>
        expect(target.querySelector('[data-action-status]')?.textContent).toContain(
          'Memo archived.',
        ),
      );
    } finally {
      await unmount(component);
    }
  });
});

async function openNoteAction(
  target: HTMLElement,
  uid: string,
  actionLabel: string,
): Promise<HTMLElement> {
  const trigger = await vi.waitFor(() => {
    const button = target.querySelector<HTMLButtonElement>(
      `[data-focus-uid="${uid}"] [aria-label="More memo actions"]`,
    );
    expect(button).not.toBeNull();
    return button!;
  });
  trigger.click();
  return vi.waitFor(() => {
    const action = document.body.querySelector<HTMLElement>(`[aria-label="${actionLabel}"]`);
    expect(action).not.toBeNull();
    return action!;
  });
}

async function waitForDialog(open: boolean): Promise<void> {
  await vi.waitFor(() =>
    expect(Boolean(document.body.querySelector('[data-slot="alert-dialog-content"]'))).toBe(open),
  );
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
