import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  createLibraryItem,
  getLibraryContent,
  listLibraryItems,
  retryLibraryItem,
  updateLibraryItem,
} from './library';
import type { LibraryItem } from './types';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('Library API', () => {
  it('serializes list filters using the backend query contract', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(jsonResponse({ items: [], page: 2, pageSize: 15, total: 0 }));
    vi.stubGlobal('fetch', fetchMock);

    await listLibraryItems({
      page: 2,
      pageSize: 15,
      q: 'design notes',
      read: false,
      starred: true,
      status: 'ARCHIVED',
      tag: 'research',
    });

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/v1/library?status=ARCHIVED&q=design+notes&tag=research&read=false&starred=true&page=2&page_size=15',
      expect.objectContaining({ credentials: 'same-origin', method: 'GET' }),
    );
  });

  it('sends manual capture fields and safely encodes item identifiers', async () => {
    const item = libraryItem();
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(item))
      .mockResolvedValueOnce(jsonResponse({ ...item, starred: true }));
    vi.stubGlobal('fetch', fetchMock);

    await createLibraryItem({
      note: 'Keep the data model section.',
      selection: 'Shared objects, strong domain tables.',
      tags: ['architecture'],
      title: 'System design',
      url: 'https://example.com/design',
    });
    await updateLibraryItem('item/with space', { starred: true });

    expect(JSON.parse(fetchMock.mock.calls[0]?.[1]?.body as string)).toEqual({
      note: 'Keep the data model section.',
      selection: 'Shared objects, strong domain tables.',
      tags: ['architecture'],
      title: 'System design',
      url: 'https://example.com/design',
    });
    expect(fetchMock.mock.calls[1]?.[0]).toBe('/api/v1/library/item%2Fwith%20space');
    expect(fetchMock.mock.calls[1]?.[1]).toEqual(expect.objectContaining({ method: 'PATCH' }));
  });

  it('uses the frozen content and retry endpoints with encoded identifiers', async () => {
    const item = libraryItem();
    const content = {
      contentVersion: 2,
      fetchedAt: '2026-08-24T02:00:00.000Z',
      plainText: 'Readable text',
      safeHtml: '<p>Readable text</p>',
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(content))
      .mockResolvedValueOnce(jsonResponse({ ...item, processingStatus: 'PENDING' }));
    vi.stubGlobal('fetch', fetchMock);

    await expect(getLibraryContent('item/with space')).resolves.toEqual(content);
    await retryLibraryItem('item/with space');

    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/v1/library/item%2Fwith%20space/content');
    expect(fetchMock.mock.calls[0]?.[1]).toEqual(expect.objectContaining({ method: 'GET' }));
    expect(fetchMock.mock.calls[1]?.[0]).toBe('/api/v1/library/item%2Fwith%20space/retry');
    expect(fetchMock.mock.calls[1]?.[1]).toEqual(expect.objectContaining({ method: 'POST' }));
  });
});

function jsonResponse(value: unknown): Response {
  return new Response(JSON.stringify(value), {
    headers: { 'content-type': 'application/json' },
    status: 200,
  });
}

function libraryItem(): LibraryItem {
  return {
    author: null,
    canonicalUrl: 'https://example.com/design',
    captures: [],
    contentAvailable: false,
    contentVersion: 0,
    createdAt: '2026-08-24T01:00:00.000Z',
    excerpt: '',
    fetchedAt: null,
    itemKind: 'BOOKMARK',
    lastError: null,
    normalizedUrl: 'https://example.com/design',
    originalUrl: 'https://example.com/design',
    processingStatus: 'NOT_FETCHED',
    publishedAt: null,
    readAt: null,
    siteName: 'Example',
    starred: false,
    status: 'ACTIVE',
    tags: ['architecture'],
    title: 'System design',
    uid: 'library-1',
    updatedAt: '2026-08-24T01:00:00.000Z',
  };
}
