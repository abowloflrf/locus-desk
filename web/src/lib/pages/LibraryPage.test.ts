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
      target.querySelector<HTMLButtonElement>('[aria-label="Add a link"]')!.click();
      await tick();
      const url = document.body.querySelector<HTMLInputElement>('#library-url')!;
      url.value = 'https://example.com/alpha';
      url.dispatchEvent(new Event('input', { bubbles: true }));
      const save = [...document.body.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Save',
      )!;
      await vi.waitFor(() => expect(save.disabled).toBe(false));
      save.click();

      await vi.waitFor(() => expect(createLibraryItem).toHaveBeenCalledOnce());
      await vi.waitFor(() =>
        expect(target.querySelector('[data-library-count]')?.textContent).toContain(
          '1 item in this view',
        ),
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
      const search = target.querySelector<HTMLInputElement>('[placeholder="Search links"]')!;
      search.value = 'fresh';
      search.dispatchEvent(new Event('input', { bubbles: true }));
      expect(initialSignal?.aborted).toBe(true);

      await vi.waitFor(() => expect(listLibraryItems).toHaveBeenCalledTimes(2), { timeout: 1_000 });
      await vi.waitFor(() => expect(target.textContent).toContain('Fresh result'));
      stale.resolve(page([item('stale', 'Stale result')]));
      await Promise.resolve();
      expect(target.textContent).not.toContain('Stale result');

      target.querySelector<HTMLButtonElement>('[aria-label="Status: Active"]')!.click();
      const archivedOption = await vi.waitFor(() => {
        const option = [
          ...document.body.querySelectorAll<HTMLElement>('[role="menuitemradio"]'),
        ].find((entry) => entry.textContent?.trim() === 'Archived');
        expect(option).not.toBeUndefined();
        return option!;
      });
      archivedOption.click();
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

  it('keeps list rows compact and only surfaces exceptional processing states', async () => {
    const ready = item('ready', 'Ready article', {
      excerpt: 'This summary belongs in the preview.',
      processingStatus: 'READY',
    });
    const pending = item('pending', 'Pending article', { processingStatus: 'PENDING' });
    vi.mocked(listLibraryItems).mockResolvedValue(page([ready, pending]));
    const { component, target } = mountPage();

    try {
      const readyRow = await vi.waitFor(() => {
        const row = target.querySelector<HTMLElement>('[data-library-select="ready"]');
        expect(row).not.toBeNull();
        return row!;
      });
      const pendingRow = target.querySelector<HTMLElement>('[data-library-select="pending"]')!;

      expect(readyRow.textContent).not.toContain('This summary belongs in the preview.');
      expect(readyRow.querySelector('.processing-state')).toBeNull();
      expect(readyRow.querySelector('.unread-mark')).not.toBeNull();
      expect(pendingRow.textContent).toContain('Processing');

      const favicon = readyRow.querySelector<HTMLImageElement>('.item-favicon img')!;
      expect(favicon.src).toBe(
        'https://www.google.com/s2/favicons?domain_url=https%3A%2F%2Fexample.com&sz=64',
      );
      expect(favicon.getAttribute('loading')).toBe('lazy');
      expect(favicon.getAttribute('referrerpolicy')).toBe('no-referrer');
      favicon.dispatchEvent(new Event('error'));
      expect(favicon.hidden).toBe(true);
      expect(readyRow.querySelector('.item-favicon')?.textContent?.trim()).toBe('E');
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
      const star = await openLibraryAction(target, 'Alpha', 'Star Alpha');
      star.click();
      await vi.waitFor(() => expect(updateLibraryItem).toHaveBeenCalledOnce());
      expect(target.querySelector('[data-focus-uid="alpha"]')?.classList).toContain('busy');

      update.reject(new Error('Star update failed.'));
      await vi.waitFor(() => expect(target.querySelector('.status-error')).not.toBeNull());
      expect(await openLibraryAction(target, 'Alpha', 'Star Alpha')).not.toBeNull();
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
      const archive = await openLibraryAction(target, 'Alpha', 'Archive Alpha');
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
            matches: query === '(max-width: 1199px)',
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
        expect(document.body.textContent).toContain('Keep the content pipeline observable.'),
      );
      const detailPanel = document.body.querySelector<HTMLElement>('[data-slot="sheet-content"]')!;
      expect(detailPanel.getAttribute('role')).toBe('dialog');
      expect(document.activeElement?.textContent).toContain('Alpha');

      document.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
      await vi.waitFor(() =>
        expect(document.body.querySelector('[data-slot="sheet-content"]')).toBeNull(),
      );
      expect(document.activeElement).toBe(select);
    } finally {
      await unmount(component);
    }
  });

  it('opens a mobile article directly in the full reader and restores the list on Back', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(
        (query: string) =>
          ({
            addEventListener: vi.fn(),
            matches: query === '(max-width: 1199px)' || query === '(max-width: 767px)',
            media: query,
            removeEventListener: vi.fn(),
          }) as unknown as MediaQueryList,
      ),
    );
    const ready = item('alpha', 'Alpha', {
      contentAvailable: true,
      contentVersion: 1,
      processingStatus: 'READY',
    });
    vi.mocked(listLibraryItems).mockResolvedValue(page([ready]));
    const onImmersiveChange = vi.fn();
    const { component, target } = mountPage(onImmersiveChange);

    try {
      const select = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      select.focus();
      select.click();

      await vi.waitFor(() => expect(target.querySelector('.library-reader')).not.toBeNull());
      expect(target.querySelector('.library-reader.preview')).toBeNull();
      expect(document.body.querySelector('[data-slot="sheet-content"]')).toBeNull();
      expect(getLibraryItem).not.toHaveBeenCalled();
      expect(onImmersiveChange).toHaveBeenLastCalledWith(true);

      const back = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Back to Library',
      )!;
      back.click();

      const restored = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-select="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      await vi.waitFor(() => expect(document.activeElement).toBe(restored));
      expect(onImmersiveChange).toHaveBeenLastCalledWith(false);
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
      const deleteButton = await openLibraryAction(target, 'Alpha', 'Delete Alpha');
      deleteButton.focus();
      deleteButton.click();
      await waitForDialog(true);
      const confirm = document.body.querySelector<HTMLButtonElement>(
        '[data-slot="alert-dialog-action"]',
      )!;
      confirm.click();

      await vi.waitFor(() =>
        expect(document.body.querySelector('[role="alert"]')?.textContent).toContain(
          'Delete failed.',
        ),
      );
      expect(document.body.querySelector('[data-slot="alert-dialog-content"]')).not.toBeNull();
      expect(target.querySelector('[data-focus-uid="alpha"]')).not.toBeNull();

      confirm.click();
      await waitForDialog(false);
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

  it('previews ready content and restores the expand button after full-screen reading', async () => {
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
      const expand = await vi.waitFor(() => {
        expect(target.querySelector('.library-reader.preview')).not.toBeNull();
        const button = target.querySelector<HTMLButtonElement>('[data-library-expand="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      expand.focus();
      expand.click();

      await vi.waitFor(() => expect(document.activeElement?.id).toBe('library-reader-title'));
      expect(target.querySelector('.library-reader.preview')).toBeNull();
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));

      const restored = await vi.waitFor(() => {
        const button = target.querySelector<HTMLButtonElement>('[data-library-expand="alpha"]');
        expect(button).not.toBeNull();
        return button!;
      });
      await vi.waitFor(() => expect(document.activeElement).toBe(restored));
      expect(target.querySelector('.library-reader.preview')).not.toBeNull();
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
      target.querySelector<HTMLButtonElement>('[data-library-expand="alpha"]')!.click();
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

function mountPage(onImmersiveChange?: (open: boolean) => void): {
  component: ReturnType<typeof mount>;
  target: HTMLDivElement;
} {
  const target = document.createElement('div');
  document.body.append(target);
  return {
    component: mount(LibraryPage, { props: { onImmersiveChange, session }, target }),
    target,
  };
}

async function openLibraryAction(
  target: HTMLElement,
  title: string,
  actionLabel: string,
): Promise<HTMLElement> {
  const trigger = await vi.waitFor(() => {
    const button = target.querySelector<HTMLButtonElement>(`[aria-label="Actions for ${title}"]`);
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
