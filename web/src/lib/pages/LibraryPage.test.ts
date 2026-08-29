import { mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  createLibraryItem,
  deleteLibraryItem,
  getLibraryContent,
  getLibraryItem,
  listLibraryItems,
  retryLibraryItem,
  updateLibraryItem,
} from '../api/library';
import type { LibraryItem, ListLibraryItemsResponse, SessionInfo } from '../api/types';
import LibraryPage from './LibraryPage.svelte';

vi.mock('../api/library', () => ({
  createLibraryItem: vi.fn(),
  deleteLibraryItem: vi.fn(),
  getLibraryContent: vi.fn(),
  getLibraryItem: vi.fn(),
  listLibraryItems: vi.fn(),
  retryLibraryItem: vi.fn(),
  updateLibraryItem: vi.fn(),
}));

const session: SessionInfo = {
  user: { uid: 'user-1', username: 'owner' },
  workspace: {
    name: 'Personal',
    role: 'OWNER',
    timezone: 'Asia/Singapore',
    today: '2026-08-24',
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
          matches: false,
          media: query,
          removeEventListener: vi.fn(),
        }) as unknown as MediaQueryList,
    ),
  );
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  vi.mocked(createLibraryItem).mockResolvedValue(item('created', 'Created link'));
  vi.mocked(deleteLibraryItem).mockResolvedValue();
  vi.mocked(getLibraryContent).mockResolvedValue({
    contentVersion: 1,
    fetchedAt: '2026-08-24T02:00:00.000Z',
    plainText: 'Readable content.',
    safeHtml: '<p>Readable content.</p>',
  });
  vi.mocked(getLibraryItem).mockImplementation(async (uid) => item(uid, uid));
  vi.mocked(listLibraryItems).mockResolvedValue(page([]));
  vi.mocked(retryLibraryItem).mockImplementation(async (uid) =>
    item(uid, uid, { processingStatus: 'PENDING' }),
  );
  vi.mocked(updateLibraryItem).mockImplementation(async (uid, payload) => ({
    ...item(uid, uid),
    ...payload,
    readAt: payload.read === undefined ? null : payload.read ? '2026-08-24T02:00:00.000Z' : null,
  }));
});

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('LibraryPage', () => {
  it('keeps a deduplicated create at one row and one total item', async () => {
    const existing = item('alpha', 'Alpha');
    vi.mocked(listLibraryItems).mockResolvedValue(page([existing]));
    vi.mocked(createLibraryItem).mockResolvedValue({
      ...existing,
      captures: [
        {
          capturedTitle: 'Alpha',
          createdAt: '2026-08-24T02:00:00.000Z',
          note: 'Second capture',
          selectedText: '',
          uid: 'capture-2',
        },
      ],
    });
    const { component, target } = mountPage();

    try {
      await vi.waitFor(() => expect(target.textContent).toContain('Alpha'));
      const url = target.querySelector<HTMLInputElement>('#library-url')!;
      url.value = 'https://example.com/alpha';
      url.dispatchEvent(new Event('input', { bubbles: true }));
      const save = target.querySelector<HTMLButtonElement>('.save-link')!;
      await vi.waitFor(() => expect(save.disabled).toBe(false));
      save.click();

      await vi.waitFor(() => expect(createLibraryItem).toHaveBeenCalledOnce());
      await vi.waitFor(() =>
        expect(target.querySelector('.library-toolbar p')?.textContent).toBe('1 item in this view'),
      );
      expect(target.querySelectorAll('[data-focus-uid="alpha"]')).toHaveLength(1);
    } finally {
      await unmount(component);
    }
  });

  it('invalidates stale searches and clears the old view while changing status', async () => {
    const stale = deferred<ListLibraryItemsResponse>();
    const fresh = item('fresh', 'Fresh result');
    const archived = item('archived', 'Archived result', { status: 'ARCHIVED' });
    vi.mocked(listLibraryItems)
      .mockReturnValueOnce(stale.promise)
      .mockResolvedValueOnce(page([fresh]))
      .mockResolvedValueOnce(page([archived]));
    const { component, target } = mountPage();

    try {
      await vi.waitFor(() => expect(listLibraryItems).toHaveBeenCalledOnce());
      const initialSignal = vi.mocked(listLibraryItems).mock.calls[0]?.[1];
      const search = target.querySelector<HTMLInputElement>('[placeholder="Search Library"]')!;
      search.value = 'fresh';
      search.dispatchEvent(new Event('input', { bubbles: true }));
      expect(initialSignal?.aborted).toBe(true);

      await vi.waitFor(() => expect(listLibraryItems).toHaveBeenCalledTimes(2), { timeout: 1_000 });
      await vi.waitFor(() => expect(target.textContent).toContain('Fresh result'));
      stale.resolve(page([item('stale', 'Stale result')]));
      await Promise.resolve();
      expect(target.textContent).not.toContain('Stale result');

      const archivedFilter = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Archived',
      )!;
      archivedFilter.click();
      await tick();
      expect(target.textContent).not.toContain('Fresh result');
      expect(target.querySelector('.library-results')?.getAttribute('aria-busy')).toBe('true');
      await vi.waitFor(() => expect(target.textContent).toContain('Archived result'));
      expect(vi.mocked(listLibraryItems).mock.calls[2]?.[0]).toEqual(
        expect.objectContaining({ q: 'fresh', status: 'ARCHIVED' }),
      );
    } finally {
      await unmount(component);
    }
  });

  it('rolls back a failed optimistic star and announces the error', async () => {
    const alpha = item('alpha', 'Alpha');
    const update = deferred<LibraryItem>();
    vi.mocked(listLibraryItems).mockResolvedValue(page([alpha]));
    vi.mocked(updateLibraryItem).mockReturnValue(update.promise);
    const { component, target } = mountPage();

    try {
      const star = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Star Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      star.click();
      await vi.waitFor(() => expect(star.getAttribute('aria-pressed')).toBe('true'));

      update.reject(new Error('Star update failed.'));
      await vi.waitFor(() => expect(star.getAttribute('aria-pressed')).toBe('false'));
      expect(target.querySelector('.status-error')?.textContent).toContain('Star update failed.');
      expect(target.querySelector('.status-error')?.getAttribute('aria-live')).toBe('assertive');
    } finally {
      await unmount(component);
    }
  });

  it('restores an archived row and its focus when the optimistic request fails', async () => {
    const alpha = item('alpha', 'Alpha');
    const beta = item('beta', 'Beta');
    const update = deferred<LibraryItem>();
    vi.mocked(listLibraryItems).mockResolvedValue(page([alpha, beta]));
    vi.mocked(updateLibraryItem).mockReturnValue(update.promise);
    const { component, target } = mountPage();

    try {
      const archive = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Archive Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      archive.focus();
      archive.click();
      await vi.waitFor(() => expect(target.querySelector('[data-focus-uid="alpha"]')).toBeNull());
      expect(
        document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
      ).toBe('beta');

      update.reject(new Error('Archive failed.'));
      await vi.waitFor(() =>
        expect(target.querySelector('[data-focus-uid="alpha"]')).not.toBeNull(),
      );
      expect(
        document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
      ).toBe('alpha');
      expect(target.querySelector('.status-error')?.textContent).toContain('Archive failed.');
    } finally {
      await unmount(component);
    }
  });

  it('shows capture history in the compact detail drawer and restores focus on Escape', async () => {
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
    const summary = item('alpha', 'Alpha');
    const detail = item('alpha', 'Alpha', {
      captures: [
        {
          capturedTitle: 'Alpha page',
          createdAt: '2026-08-24T01:00:00.000Z',
          note: 'Use this in the migration plan.',
          selectedText: 'Keep the content pipeline observable.',
          uid: 'capture-1',
        },
      ],
    });
    vi.mocked(listLibraryItems).mockResolvedValue(page([summary]));
    vi.mocked(getLibraryItem).mockResolvedValue(detail);
    const { component, target } = mountPage();

    try {
      const select = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      select.focus();
      select.click();

      await vi.waitFor(() =>
        expect(target.textContent).toContain('Keep the content pipeline observable.'),
      );
      const detailPanel = target.querySelector<HTMLElement>('.library-detail')!;
      expect(detailPanel.getAttribute('aria-modal')).toBe('true');
      expect(detailPanel.getAttribute('role')).toBe('dialog');
      expect(document.activeElement?.textContent).toContain('Alpha');

      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      await vi.waitFor(() => expect(detailPanel.getAttribute('aria-hidden')).toBe('true'));
      expect(document.activeElement).toBe(select);
    } finally {
      await unmount(component);
    }
  });

  it('keeps delete failures in the confirmation dialog, then focuses the neighbor on success', async () => {
    installDialogPolyfill();
    const alpha = item('alpha', 'Alpha');
    const beta = item('beta', 'Beta');
    vi.mocked(listLibraryItems)
      .mockResolvedValueOnce(page([alpha, beta]))
      .mockResolvedValue(page([beta]));
    vi.mocked(deleteLibraryItem)
      .mockRejectedValueOnce(new Error('Delete failed.'))
      .mockResolvedValue();
    const { component, target } = mountPage();

    try {
      const deleteButton = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[aria-label="Delete Alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      deleteButton.focus();
      deleteButton.click();
      await vi.waitFor(() => expect(target.querySelector('dialog')?.open).toBe(true));
      const confirm = target.querySelector<HTMLButtonElement>('.confirm-dialog .button.danger')!;
      confirm.click();

      await vi.waitFor(() =>
        expect(target.querySelector('.confirm-dialog [role="alert"]')?.textContent).toContain(
          'Delete failed.',
        ),
      );
      expect(target.querySelector('dialog')?.open).toBe(true);
      expect(target.querySelector('[data-focus-uid="alpha"]')).not.toBeNull();

      confirm.click();
      await vi.waitFor(() => expect(target.querySelector('dialog')?.open).toBe(false));
      await vi.waitFor(() => expect(target.querySelector('[data-focus-uid="alpha"]')).toBeNull());
      await vi.waitFor(() =>
        expect(
          document.activeElement?.closest('[data-focus-uid]')?.getAttribute('data-focus-uid'),
        ).toBe('beta'),
      );
      expect(target.querySelector('[data-action-status]')?.textContent).toContain(
        'Library item deleted.',
      );
    } finally {
      await unmount(component);
      uninstallDialogPolyfill();
    }
  });

  it('retries failed capture and moves the detail into a stable pending state', async () => {
    const failed = item('alpha', 'Alpha', {
      lastError: 'The source timed out.',
      processingStatus: 'FAILED',
    });
    const pending = item('alpha', 'Alpha', {
      processingStatus: 'PENDING',
    });
    vi.mocked(listLibraryItems).mockResolvedValue(page([failed]));
    vi.mocked(getLibraryItem).mockResolvedValue(failed);
    vi.mocked(retryLibraryItem).mockResolvedValue(pending);
    const { component, target } = mountPage();

    try {
      const select = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      select.click();
      const retry = await vi.waitFor(() => {
        expect(target.textContent).toContain('The source timed out.');
        const button = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
          (entry) => entry.textContent?.trim() === 'Retry',
        );
        expect(button).not.toBeUndefined();
        return button!;
      });
      retry.click();

      await vi.waitFor(() =>
        expect(retryLibraryItem).toHaveBeenCalledWith('alpha', expect.any(AbortSignal)),
      );
      await vi.waitFor(() => expect(target.textContent).toContain('Preparing article'));
      expect(target.querySelector('[data-action-status]')?.textContent).toContain(
        'Content capture queued.',
      );
      expect(target.textContent).not.toContain('The source timed out.');
    } finally {
      await unmount(component);
    }
  });

  it('aborts an in-flight pending poll and clears its timer when details close', async () => {
    vi.useFakeTimers();
    const pendingItem = item('alpha', 'Alpha', { processingStatus: 'PENDING' });
    const pendingPoll = deferred<LibraryItem>();
    vi.mocked(listLibraryItems).mockResolvedValue(page([pendingItem]));
    vi.mocked(getLibraryItem)
      .mockResolvedValueOnce(pendingItem)
      .mockReturnValueOnce(pendingPoll.promise);
    const { component, target } = mountPage();

    try {
      await flushUpdates();
      const select = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]')!;
      expect(select).not.toBeNull();
      select.click();
      await flushUpdates();
      expect(getLibraryItem).toHaveBeenCalledTimes(1);

      await vi.advanceTimersByTimeAsync(2_000);
      await flushUpdates();
      expect(getLibraryItem).toHaveBeenCalledTimes(2);
      const pollSignal = vi.mocked(getLibraryItem).mock.calls[1]?.[1];
      expect(pollSignal?.aborted).toBe(false);

      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      await flushUpdates();
      expect(pollSignal?.aborted).toBe(true);
      await vi.advanceTimersByTimeAsync(6_000);
      expect(getLibraryItem).toHaveBeenCalledTimes(2);
    } finally {
      await unmount(component);
      vi.useRealTimers();
    }
  });

  it('opens ready content and restores the Read button focus when the reader closes with Escape', async () => {
    const ready = item('alpha', 'Alpha', {
      contentAvailable: true,
      contentVersion: 1,
      processingStatus: 'READY',
    });
    vi.mocked(listLibraryItems).mockResolvedValue(page([ready]));
    vi.mocked(getLibraryItem).mockResolvedValue(ready);
    const { component, target } = mountPage();

    try {
      const select = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      select.click();
      const read = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-read="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      read.focus();
      read.click();

      await vi.waitFor(() => expect(document.activeElement?.id).toBe('library-reader-title'));
      expect(target.querySelector('.library-reader')).not.toBeNull();
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));

      const restored = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-read="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      await vi.waitFor(() => expect(document.activeElement).toBe(restored));
      expect(target.querySelector('.library-reader')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('preserves full capture history when a list page refreshes the selected summary', async () => {
    const summary = item('alpha', 'Alpha');
    const detail = item('alpha', 'Alpha', {
      captures: [
        {
          capturedTitle: 'Alpha',
          createdAt: '2026-08-24T01:00:00.000Z',
          note: 'Keep this capture visible.',
          selectedText: 'Selected context',
          uid: 'capture-1',
        },
      ],
    });
    vi.mocked(listLibraryItems)
      .mockResolvedValueOnce({ ...page([summary]), total: 2 })
      .mockResolvedValueOnce({ ...page([item('beta', 'Beta')]), page: 2, total: 2 });
    vi.mocked(getLibraryItem).mockResolvedValue(detail);
    const { component, target } = mountPage();

    try {
      const select = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      select.click();
      await vi.waitFor(() => expect(target.textContent).toContain('Keep this capture visible.'));

      const loadMore = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Load more',
      )!;
      loadMore.click();

      await vi.waitFor(() => expect(target.textContent).toContain('Beta'));
      expect(target.textContent).toContain('Keep this capture visible.');
      expect(target.querySelector('.captures-section span')?.textContent).toBe('1');
    } finally {
      await unmount(component);
    }
  });

  it('cancels stale retry state when switching details', async () => {
    const alpha = item('alpha', 'Alpha', { processingStatus: 'FAILED' });
    const beta = item('beta', 'Beta', { processingStatus: 'FAILED' });
    const alphaRetry = deferred<LibraryItem>();
    vi.mocked(listLibraryItems).mockResolvedValue(page([alpha, beta]));
    vi.mocked(getLibraryItem).mockImplementation(async (uid) => (uid === 'alpha' ? alpha : beta));
    vi.mocked(retryLibraryItem)
      .mockReturnValueOnce(alphaRetry.promise)
      .mockResolvedValueOnce(item('beta', 'Beta', { processingStatus: 'PENDING' }));
    const { component, target } = mountPage();

    try {
      const alphaSelect = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      alphaSelect.click();
      const alphaRetryButton = await vi.waitFor(() => {
        const button = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
          (entry) => entry.textContent?.trim() === 'Retry',
        );
        expect(button).not.toBeUndefined();
        return button!;
      });
      alphaRetryButton.click();
      await vi.waitFor(() => expect(retryLibraryItem).toHaveBeenCalledTimes(1));
      const alphaSignal = vi.mocked(retryLibraryItem).mock.calls[0]?.[1];

      target.querySelector<HTMLButtonElement>('[data-library-select="beta"]')!.click();
      await vi.waitFor(() => expect(alphaSignal?.aborted).toBe(true));
      const betaRetryButton = await vi.waitFor(() => {
        const button = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
          (entry) => entry.textContent?.trim() === 'Retry',
        );
        expect(button?.disabled).toBe(false);
        return button!;
      });
      betaRetryButton.click();
      await vi.waitFor(() => expect(retryLibraryItem).toHaveBeenCalledTimes(2));
      alphaRetry.resolve(item('alpha', 'Alpha', { processingStatus: 'PENDING' }));

      await vi.waitFor(() => expect(target.textContent).toContain('Preparing article'));
      expect(target.querySelector('.library-detail h2')?.textContent).toBe('Beta');
      expect(target.querySelector('[data-action-status]')?.textContent).toContain(
        'Content capture queued.',
      );
    } finally {
      await unmount(component);
    }
  });

  it('aborts the detail request before entering the reader', async () => {
    vi.useFakeTimers();
    const ready = item('alpha', 'Alpha', {
      contentAvailable: true,
      contentVersion: 1,
      processingStatus: 'READY',
    });
    const lateDetail = deferred<LibraryItem>();
    vi.mocked(listLibraryItems).mockResolvedValue(page([ready]));
    vi.mocked(getLibraryItem).mockReturnValue(lateDetail.promise);
    const { component, target } = mountPage();

    try {
      await flushUpdates();
      target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]')!.click();
      await flushUpdates();
      const detailSignal = vi.mocked(getLibraryItem).mock.calls[0]?.[1];
      target.querySelector<HTMLButtonElement>('[data-library-read="alpha"]')!.click();
      await flushUpdates();
      expect(detailSignal?.aborted).toBe(true);

      lateDetail.resolve(item('alpha', 'Alpha', { processingStatus: 'PENDING' }));
      await flushUpdates();
      await vi.advanceTimersByTimeAsync(6_000);
      expect(getLibraryItem).toHaveBeenCalledTimes(1);
      expect(target.querySelector('.library-reader')).not.toBeNull();
    } finally {
      await unmount(component);
      vi.useRealTimers();
    }
  });
});

function mountPage(): { component: ReturnType<typeof mount>; target: HTMLDivElement } {
  const target = document.createElement('div');
  document.body.append(target);
  return { component: mount(LibraryPage, { props: { session }, target }), target };
}

function page(items: LibraryItem[]): ListLibraryItemsResponse {
  return { items, page: 1, pageSize: 30, total: items.length };
}

function item(uid: string, title: string, overrides: Partial<LibraryItem> = {}): LibraryItem {
  return {
    author: null,
    canonicalUrl: `https://example.com/${uid}`,
    captures: [],
    contentAvailable: false,
    contentVersion: 0,
    createdAt: '2026-08-24T01:00:00.000Z',
    excerpt: '',
    fetchedAt: null,
    itemKind: 'BOOKMARK',
    lastError: null,
    normalizedUrl: `https://example.com/${uid}`,
    originalUrl: `https://example.com/${uid}`,
    processingStatus: 'NOT_FETCHED',
    publishedAt: null,
    readAt: null,
    siteName: 'Example',
    starred: false,
    status: 'ACTIVE',
    tags: [],
    title,
    uid,
    updatedAt: '2026-08-24T01:00:00.000Z',
    ...overrides,
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

async function flushUpdates(): Promise<void> {
  await Promise.resolve();
  await tick();
  await Promise.resolve();
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
