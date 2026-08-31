import { mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { getLibraryContent } from '../api/library';
import type { LibraryItem } from '../api/types';
import { READER_PREFERENCES_STORAGE_KEY } from '../reader-preferences';
import LibraryReader from './LibraryReader.svelte';
import readerSource from './LibraryReader.svelte?raw';

vi.mock('../api/library', () => ({
  getLibraryContent: vi.fn(),
}));

beforeEach(() => {
  window.localStorage.clear();
  vi.mocked(getLibraryContent).mockResolvedValue({
    contentVersion: 3,
    fetchedAt: '2026-08-24T03:00:00.000Z',
    plainText: 'A safe article.',
    safeHtml:
      '<p>A <strong>safe</strong> article. <a href="/next">Next</a></p><img src="/media/hero.jpg" alt="Article illustration"><script>alert(1)</script>',
  });
});

afterEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
  document.body.replaceChildren();
});

describe('LibraryReader', () => {
  it('focuses the article, sanitizes backend HTML again, and isolates links', async () => {
    const { component, target } = mountReader();

    try {
      const heading = target.querySelector<HTMLHeadingElement>('#library-reader-title')!;
      await vi.waitFor(() => expect(document.activeElement).toBe(heading));
      await vi.waitFor(() => expect(target.querySelector('.reader-content')).not.toBeNull());

      const content = target.querySelector<HTMLElement>('.reader-content')!;
      expect(content.textContent).toContain('A safe article.');
      expect(content.querySelector('script')).toBeNull();
      const link = content.querySelector<HTMLAnchorElement>('a')!;
      expect(link.href).toBe('https://example.com/next');
      expect(link.rel).toBe('noopener noreferrer');
      expect(link.target).toBe('_blank');
      const image = content.querySelector<HTMLImageElement>('img')!;
      expect(image.src).toBe('https://example.com/media/hero.jpg');
      expect(image.alt).toBe('Article illustration');
      expect(image.getAttribute('loading')).toBe('lazy');
      expect(image.getAttribute('referrerpolicy')).toBe('no-referrer');
    } finally {
      await unmount(component);
    }
  });

  it('presents captured metadata as an article excerpt separate from the reader body', async () => {
    const { component, target } = mountReader();

    try {
      const excerpt = target.querySelector<HTMLElement>('aside[aria-label="Article excerpt"]')!;
      expect(excerpt.querySelector('.reader-excerpt-label')?.textContent).toBe('Article excerpt');
      expect(excerpt.querySelector('.reader-excerpt-copy')?.textContent).toBe(
        'A short, calm summary.',
      );
      await vi.waitFor(() => expect(target.querySelector('.reader-content')).not.toBeNull());
      expect(target.querySelector('.reader-content')?.contains(excerpt)).toBe(false);
    } finally {
      await unmount(component);
    }
  });

  it('lets ready articles be refreshed so previously sanitized captures can be replaced', async () => {
    const onRetry = vi.fn();
    const { component, target } = mountReader({ onRetry });

    try {
      await vi.waitFor(() => expect(target.querySelector('.reader-content')).not.toBeNull());
      const refresh = target.querySelector<HTMLButtonElement>(
        'button[aria-label="Refresh article"]',
      )!;
      refresh.click();
      expect(onRetry).toHaveBeenCalledOnce();
    } finally {
      await unmount(component);
    }
  });

  it('keeps saved content visible while a shorter refresh waits for a decision', async () => {
    const onAcceptRefresh = vi.fn();
    const onDiscardRefresh = vi.fn();
    const { component, target } = mountReader({
      item: libraryItem({ refreshStatus: 'REVIEW' }),
      onAcceptRefresh,
      onDiscardRefresh,
    });

    try {
      await vi.waitFor(() => expect(target.querySelector('.reader-content')).not.toBeNull());
      expect(target.textContent).toContain('Shorter refresh needs review');
      expect(target.textContent).toContain('A safe article.');
      const keep = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Keep saved version',
      )!;
      keep.click();
      expect(onDiscardRefresh).toHaveBeenCalledOnce();
      const use = [...target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Use refreshed version',
      )!;
      use.click();
      expect(onAcceptRefresh).toHaveBeenCalledOnce();
    } finally {
      await unmount(component);
    }
  });

  it('returns on Escape and keeps reader controls at touch-target size', async () => {
    const onBack = vi.fn();
    const { component, target } = mountReader({ onBack });

    try {
      await vi.waitFor(() => expect(document.activeElement?.id).toBe('library-reader-title'));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
      expect(onBack).toHaveBeenCalledOnce();

      expect(readerSource).toContain('min-height: 44px');
      expect(readerSource).toContain('@media (max-width: 767px)');
      expect(readerSource).toContain('env(safe-area-inset-top)');
      expect(readerSource).toContain('calc(48px + env(safe-area-inset-bottom))');
      expect(readerSource).toContain('.reader-heading h1:focus-visible');
    } finally {
      await unmount(component);
    }
  });

  it('loads and persists browser reading preferences', async () => {
    window.localStorage.setItem(
      READER_PREFERENCES_STORAGE_KEY,
      JSON.stringify({
        fontPreset: 'atkinson',
        fontSize: 'large',
        lineHeight: 'spacious',
        width: 'wide',
      }),
    );
    const { component, target } = mountReader();

    try {
      const reader = target.querySelector<HTMLElement>('.library-reader')!;
      await vi.waitFor(() => expect(reader.dataset.readerFont).toBe('atkinson'));
      expect(reader.dataset.readerSize).toBe('large');
      expect(reader.dataset.readerLineHeight).toBe('spacious');
      expect(reader.dataset.readerWidth).toBe('wide');

      target.querySelector<HTMLButtonElement>('[aria-label="Reading preferences"]')!.click();
      await vi.waitFor(() =>
        expect(
          document.querySelector<HTMLButtonElement>('[aria-label="16 pixel text"]'),
        ).not.toBeNull(),
      );
      document.querySelector<HTMLButtonElement>('[aria-label="16 pixel text"]')!.click();
      document.querySelector<HTMLButtonElement>('[aria-label="Balanced article width"]')!.click();

      await vi.waitFor(() => expect(reader.dataset.readerSize).toBe('small'));
      expect(reader.dataset.readerWidth).toBe('balanced');
      expect(JSON.parse(window.localStorage.getItem(READER_PREFERENCES_STORAGE_KEY)!)).toEqual({
        fontPreset: 'atkinson',
        fontSize: 'small',
        lineHeight: 'spacious',
        width: 'balanced',
      });
    } finally {
      await unmount(component);
    }
  });

  it('hides the sticky toolbar while scrolling down and reveals it while scrolling up', async () => {
    const { component, target } = mountReader();

    try {
      const toolbar = target.querySelector<HTMLElement>('.reader-toolbar')!;
      const preferences = target.querySelector<HTMLButtonElement>(
        '[aria-label="Reading preferences"]',
      )!;
      await vi.waitFor(() => expect(document.activeElement?.id).toBe('library-reader-title'));

      window.dispatchEvent(new Event('pointerdown'));
      preferences.focus();
      for (const scrollTop of [8, 16, 24, 32, 40, 48, 56, 64, 72, 80]) {
        target.scrollTop = scrollTop;
        target.dispatchEvent(new Event('scroll'));
      }
      await vi.waitFor(() => expect(toolbar.classList).toContain('toolbar-hidden'));

      for (const scrollTop of [76, 72, 68]) {
        target.scrollTop = scrollTop;
        target.dispatchEvent(new Event('scroll'));
      }
      await vi.waitFor(() => expect(toolbar.classList).not.toContain('toolbar-hidden'));

      expect(readerSource).toContain('position: sticky');
      expect(readerSource).toContain('prefers-reduced-motion');
    } finally {
      await unmount(component);
    }
  });

  it('aborts an in-flight content request when the reader closes', async () => {
    const pending = deferred<never>();
    vi.mocked(getLibraryContent).mockReturnValue(pending.promise);
    const { component } = mountReader();

    await vi.waitFor(() => expect(getLibraryContent).toHaveBeenCalledOnce());
    const signal = vi.mocked(getLibraryContent).mock.calls[0]?.[1];
    expect(signal?.aborted).toBe(false);
    await unmount(component);
    expect(signal?.aborted).toBe(true);
  });

  it('shows pending and failed capture states without requesting unavailable content', async () => {
    const pending = mountReader({
      item: libraryItem({ contentAvailable: false, processingStatus: 'PENDING' }),
    });

    try {
      await vi.waitFor(() => expect(pending.target.textContent).toContain('Preparing article…'));
      expect(getLibraryContent).not.toHaveBeenCalled();
    } finally {
      await unmount(pending.component);
    }

    vi.clearAllMocks();
    const onRetry = vi.fn();
    const failed = mountReader({
      item: libraryItem({
        contentAvailable: false,
        lastError: 'The source timed out.',
        processingStatus: 'FAILED',
      }),
      onRetry,
    });

    try {
      await vi.waitFor(() => expect(failed.target.textContent).toContain('The source timed out.'));
      const retry = [...failed.target.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Retry',
      )!;
      retry.click();
      expect(onRetry).toHaveBeenCalledOnce();
      expect(getLibraryContent).not.toHaveBeenCalled();
    } finally {
      await unmount(failed.component);
    }
  });
});

function mountReader(
  overrides: {
    item?: LibraryItem;
    onBack?: () => void;
    onAcceptRefresh?: (item: LibraryItem) => void;
    onDiscardRefresh?: (item: LibraryItem) => void;
    onRetry?: (item: LibraryItem) => void;
  } = {},
): {
  component: ReturnType<typeof mount>;
  target: HTMLDivElement;
} {
  const target = document.createElement('div');
  target.className = 'workspace-column';
  document.body.append(target);
  const component = mount(LibraryReader, {
    props: {
      item: overrides.item ?? libraryItem(),
      onBack: overrides.onBack ?? vi.fn(),
      onAcceptRefresh: overrides.onAcceptRefresh,
      onDiscardRefresh: overrides.onDiscardRefresh,
      onRetry: overrides.onRetry,
      onToggleRead: vi.fn(),
      timeZone: 'Asia/Singapore',
    },
    target,
  });
  return { component, target };
}

function libraryItem(overrides: Partial<LibraryItem> = {}): LibraryItem {
  return {
    author: 'Ada Reader',
    canonicalUrl: 'https://example.com/article',
    captures: [],
    contentAvailable: true,
    contentVersion: 3,
    createdAt: '2026-08-24T01:00:00.000Z',
    excerpt: 'A short, calm summary.',
    fetchedAt: '2026-08-24T03:00:00.000Z',
    itemKind: 'ARTICLE',
    lastError: null,
    normalizedUrl: 'https://example.com/article',
    originalUrl: 'https://example.com/article',
    processingStatus: 'READY',
    refreshError: null,
    refreshStatus: 'IDLE',
    publishedAt: '2026-08-20T01:00:00.000Z',
    readAt: null,
    siteName: 'Example',
    starred: false,
    status: 'ACTIVE',
    tags: [],
    title: 'A readable article',
    uid: 'library-1',
    updatedAt: '2026-08-24T03:00:00.000Z',
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
