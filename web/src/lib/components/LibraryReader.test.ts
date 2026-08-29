import { mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { getLibraryContent } from '../api/library';
import type { LibraryItem } from '../api/types';
import LibraryReader from './LibraryReader.svelte';
import readerSource from './LibraryReader.svelte?raw';

vi.mock('../api/library', () => ({
  getLibraryContent: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(getLibraryContent).mockResolvedValue({
    contentVersion: 3,
    fetchedAt: '2026-08-24T03:00:00.000Z',
    plainText: 'A safe article.',
    safeHtml:
      '<p>A <strong>safe</strong> article. <a href="/next">Next</a></p><script>alert(1)</script>',
  });
});

afterEach(() => {
  vi.clearAllMocks();
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
      expect(readerSource).toContain('padding: 12px 16px 72px');
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
});

function mountReader(overrides: { onBack?: () => void } = {}): {
  component: ReturnType<typeof mount>;
  target: HTMLDivElement;
} {
  const target = document.createElement('div');
  document.body.append(target);
  const component = mount(LibraryReader, {
    props: {
      item: libraryItem(),
      onBack: overrides.onBack ?? vi.fn(),
      onToggleRead: vi.fn(),
      timeZone: 'Asia/Singapore',
    },
    target,
  });
  return { component, target };
}

function libraryItem(): LibraryItem {
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
    publishedAt: '2026-08-20T01:00:00.000Z',
    readAt: null,
    siteName: 'Example',
    starred: false,
    status: 'ACTIVE',
    tags: [],
    title: 'A readable article',
    uid: 'library-1',
    updatedAt: '2026-08-24T03:00:00.000Z',
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
