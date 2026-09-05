import { mount, tick, unmount } from 'svelte';
import { fromStore, writable } from 'svelte/store';
import { highlightLibraryHtml } from '../library-content';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { getLibraryContent } from '../api/library';
import type { LibraryItem } from '../api/types';
import { READER_PREFERENCES_STORAGE_KEY } from '../reader-preferences';
import LibraryReader from './LibraryReader.svelte';
import readerSource from './LibraryReader.svelte?raw';

vi.mock('../api/library', () => ({
  getLibraryContent: vi.fn(),
}));

vi.mock('../library-content', async (original) => {
  const module = await original<typeof import('../library-content')>();
  return { ...module, highlightLibraryHtml: vi.fn(module.highlightLibraryHtml) };
});

beforeEach(() => {
  vi.mocked(highlightLibraryHtml).mockReset();
  vi.mocked(highlightLibraryHtml).mockImplementation(async (source) => source);
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
  vi.unstubAllGlobals();
  window.localStorage.clear();
  document.body.replaceChildren();
});

describe('LibraryReader', () => {
  it.each([
    ['2026-09-03T00:00:00.742Z', 'Sep 3, 2026'],
    [null, 'Aug 24, 2026'],
  ])('uses the publication date or capture creation date (%s)', async (publishedAt, label) => {
    const { component, target } = mountReader({ item: libraryItem({ publishedAt }) });
    try {
      await vi.waitFor(() => expect(target.textContent).toContain(label));
    } finally {
      await unmount(component);
    }
  });

  it.each(['preview', 'full'] as const)(
    'navigates and tracks headings within the %s scroll container',
    async (mode) => {
      vi.stubGlobal(
        'matchMedia',
        vi.fn((query: string) => ({
          matches: query === '(prefers-reduced-motion: reduce)',
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
        })),
      );
      vi.mocked(getLibraryContent).mockResolvedValue({
        contentVersion: 3,
        fetchedAt: '2026-08-24T03:00:00.000Z',
        plainText: 'Article sections',
        safeHtml:
          '<h2>First section</h2><p>Introduction</p><h3>Details</h3><p>Details text</p><h2>Last section</h2>',
      });
      const onBack = vi.fn();
      const { component, target } = mountReader({ mode, onBack });
      const scrollTo = vi.fn();
      target.scrollTo = scrollTo;
      Object.defineProperties(target, {
        clientHeight: { configurable: true, value: 800 },
        scrollHeight: { configurable: true, value: 2400 },
      });
      try {
        await vi.waitFor(() =>
          expect(target.querySelectorAll('.toc-markers > span')).toHaveLength(3),
        );
        expect(document.querySelector('.toc-list')).toBeNull();
        const headings = [
          ...target.querySelectorAll<HTMLElement>('.reader-content h2, .reader-content h3'),
        ];
        headings.forEach((heading, index) => {
          vi.spyOn(heading, 'getBoundingClientRect').mockImplementation(
            () =>
              ({
                top: 200 + index * 700 - target.scrollTop,
              }) as DOMRect,
          );
        });
        target.scrollTop = 800;
        target.dispatchEvent(new Event('scroll'));
        await vi.waitFor(() =>
          expect(
            target.querySelector('.toc-markers .current')?.getAttribute('data-heading-id'),
          ).toBe(headings[1]!.id),
        );

        const trigger = target.querySelector<HTMLButtonElement>(
          'button[aria-label="Table of contents"]',
        )!;
        const previousFocus = document.activeElement;
        const pointer = (type: string, pointerType = 'mouse') =>
          Object.assign(new Event(type, { bubbles: true }), {
            pointerType,
            clientX: 0,
            clientY: 0,
          });
        trigger.dispatchEvent(pointer('pointerenter', 'touch'));
        await tick();
        expect(trigger.getAttribute('aria-expanded')).toBe('false');
        trigger.dispatchEvent(pointer('pointerenter'));
        await vi.waitFor(() => expect(document.querySelector('.toc-list a')).not.toBeNull());
        expect(document.activeElement).toBe(previousFocus);
        const ruler = [...target.querySelectorAll<HTMLElement>('.toc-markers > span')];
        ruler.forEach((marker, index) => {
          vi.spyOn(marker, 'getBoundingClientRect').mockReturnValue({
            top: 100 + index * 8,
            height: 2,
          } as DOMRect);
        });
        const hoverPanel = document.querySelector<HTMLElement>('[data-slot="popover-content"]')!;
        vi.spyOn(hoverPanel, 'getBoundingClientRect').mockReturnValue({
          top: 100,
          bottom: 300,
          height: 200,
        } as DOMRect);
        const lastLink = hoverPanel.querySelectorAll<HTMLAnchorElement>('a')[2]!;
        vi.spyOn(lastLink, 'getBoundingClientRect').mockReturnValue({
          top: 400,
          bottom: 440,
          height: 40,
        } as DOMRect);
        trigger.dispatchEvent(Object.assign(pointer('pointermove'), { clientY: 117 }));
        await vi.waitFor(() => expect(lastLink.hasAttribute('data-preview-current')).toBe(true));
        expect(ruler[2]!.classList.contains('previewed')).toBe(true);
        expect(hoverPanel.scrollTop).toBeGreaterThan(0);
        expect(hoverPanel.querySelector('a[aria-current="location"]')?.textContent).toBe('Details');
        expect(scrollTo).not.toHaveBeenCalled();
        expect(document.activeElement).toBe(previousFocus);
        trigger.dispatchEvent(Object.assign(pointer('pointerdown'), { clientY: 117 }));
        trigger.dispatchEvent(
          new MouseEvent('click', { detail: 1, clientY: 117, bubbles: true, cancelable: true }),
        );
        await vi.waitFor(() => expect(document.activeElement).toBe(headings[2]));
        expect(scrollTo).toHaveBeenLastCalledWith({ top: 1520, behavior: 'instant' });
        await vi.waitFor(() => expect(document.querySelector('.toc-list')).toBeNull());
        expect(target.querySelector('.previewed')).toBeNull();
        trigger.click();
        await vi.waitFor(() => expect(document.querySelector('.toc-list a')).not.toBeNull());
        document
          .querySelector('[data-slot="popover-content"]')!
          .dispatchEvent(
            new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
          );
        await vi.waitFor(() => expect(document.querySelector('.toc-list')).toBeNull());
        await vi.waitFor(() => expect(document.activeElement).toBe(trigger));
        trigger.click();
        await vi.waitFor(() =>
          expect(document.querySelector('[data-slot="popover-content"] a')).not.toBeNull(),
        );
        const panel = document.querySelector<HTMLElement>('[data-slot="popover-content"]')!;
        panel.querySelectorAll<HTMLAnchorElement>('a')[2]!.click();
        await vi.waitFor(() => expect(document.activeElement).toBe(headings[2]));
        expect(scrollTo).toHaveBeenLastCalledWith({ top: 1520, behavior: 'instant' });
        expect(window.location.hash).toBe('');

        await vi.waitFor(() =>
          expect(document.querySelector('[data-slot="popover-content"]')).toBeNull(),
        );
        trigger.focus();
        trigger.click();
        await vi.waitFor(() => expect(trigger.getAttribute('aria-expanded')).toBe('true'));
        await vi.waitFor(() =>
          expect(document.activeElement?.closest('[data-slot="popover-content"]')).not.toBeNull(),
        );
        document.activeElement?.dispatchEvent(
          new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
        );
        await vi.waitFor(() => expect(trigger.getAttribute('aria-expanded')).toBe('false'));
        expect(onBack).not.toHaveBeenCalled();
        await vi.waitFor(() => expect(document.activeElement).toBe(trigger));

        trigger.click();
        await vi.waitFor(() =>
          expect(
            document.querySelector(
              '[data-slot="popover-content"] button[aria-label="Back to top"]',
            ),
          ).not.toBeNull(),
        );
        document
          .querySelector<HTMLButtonElement>(
            '[data-slot="popover-content"] button[aria-label="Back to top"]',
          )!
          .click();
        expect(scrollTo).toHaveBeenLastCalledWith({ top: 0, behavior: 'instant' });
        await vi.waitFor(() => expect(document.activeElement?.id).toBe('library-reader-title'));
      } finally {
        await unmount(component);
      }
    },
  );

  it('keeps the current article when an older highlight finishes later', async () => {
    const oldHighlight = deferred<string>();
    vi.mocked(highlightLibraryHtml).mockImplementation((source) =>
      source.includes('Old code') ? oldHighlight.promise : Promise.resolve(source),
    );
    vi.mocked(getLibraryContent)
      .mockResolvedValueOnce({
        contentVersion: 3,
        fetchedAt: '2026-08-24T03:00:00.000Z',
        plainText: 'Old code',
        safeHtml: '<pre data-language="js">Old code</pre>',
      })
      .mockResolvedValueOnce({
        contentVersion: 4,
        fetchedAt: '2026-08-24T03:00:00.000Z',
        plainText: 'New code',
        safeHtml: '<pre data-language="js">New code</pre>',
      });
    const item = writable(libraryItem());
    const reactive = fromStore(item);
    const component = mount(LibraryReader, {
      target: document.body,
      props: {
        get item() {
          return reactive.current;
        },
        onBack: vi.fn(),
        onToggleRead: vi.fn(),
        timeZone: 'UTC',
      },
    });
    try {
      await vi.waitFor(() =>
        expect(document.querySelector('.reader-content')?.textContent).toBe('Old code'),
      );
      item.set(libraryItem({ uid: 'library-2', contentVersion: 4 }));
      await vi.waitFor(() =>
        expect(document.querySelector('.reader-content')?.textContent).toBe('New code'),
      );
      oldHighlight.resolve('<pre>Stale highlight</pre>');
      await tick();
      expect(document.querySelector('.reader-content')?.textContent).toBe('New code');
    } finally {
      await unmount(component);
    }
  });

  it('keeps sanitized content readable when highlighting fails', async () => {
    vi.mocked(highlightLibraryHtml).mockRejectedValue(new Error('Highlight failed'));
    const { component, target } = mountReader();
    try {
      await vi.waitFor(() =>
        expect(target.querySelector('.reader-content')?.textContent).toContain('A safe article.'),
      );
      expect(target.querySelector('.reader-content script')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('focuses the article, sanitizes backend HTML again, and isolates links', async () => {
    const { component, target } = mountReader();

    try {
      const heading = target.querySelector<HTMLHeadingElement>('#library-reader-title')!;
      await vi.waitFor(() => expect(document.activeElement).toBe(heading));
      await vi.waitFor(() => expect(target.querySelector('.reader-content')).not.toBeNull());

      const content = target.querySelector<HTMLElement>('.reader-content')!;
      expect(content.textContent).toContain('A safe article.');
      expect(content.querySelector('script')).toBeNull();
      expect(target.querySelector('[aria-label="Article navigation"]')).toBeNull();
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
        fontPreset: 'sans',
        fontSize: 'large',
        lineHeight: 'spacious',
        width: 'wide',
      }),
    );
    const { component, target } = mountReader();

    try {
      const reader = target.querySelector<HTMLElement>('.library-reader')!;
      await vi.waitFor(() => expect(reader.dataset.readerFont).toBe('sans'));
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
      document.querySelector<HTMLButtonElement>('[aria-label="System typeface"]')!.click();
      await vi.waitFor(() => expect(reader.dataset.readerFont).toBe('system'));
      document.querySelector<HTMLButtonElement>('[aria-label="Montserrat typeface"]')!.click();
      await vi.waitFor(() => expect(reader.dataset.readerFont).toBe('sans'));

      await vi.waitFor(() => expect(reader.dataset.readerSize).toBe('small'));
      expect(reader.dataset.readerWidth).toBe('balanced');
      expect(JSON.parse(window.localStorage.getItem(READER_PREFERENCES_STORAGE_KEY)!)).toEqual({
        fontPreset: 'sans',
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
    mode?: 'preview' | 'full';
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
  target.className = overrides.mode === 'preview' ? 'library-detail' : 'workspace-column';
  document.body.append(target);
  const component = mount(LibraryReader, {
    props: {
      mode: overrides.mode,
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
