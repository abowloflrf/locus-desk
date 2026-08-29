import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { LibraryItem } from '../api/types';
import LibraryCaptureForm from './LibraryCaptureForm.svelte';

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('LibraryCaptureForm', () => {
  it('submits one URL once, then closes the popover and restores trigger focus', async () => {
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    let resolveCreate: ((item: LibraryItem) => void) | undefined;
    const onCreate = vi.fn(
      () =>
        new Promise<LibraryItem>((resolve) => {
          resolveCreate = resolve;
        }),
    );
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(LibraryCaptureForm, { props: { onCreate }, target });

    try {
      await tick();
      const trigger = target.querySelector<HTMLButtonElement>('[aria-label="Add a link"]')!;
      trigger.click();
      const url = await vi.waitFor(() => {
        const input = document.body.querySelector<HTMLInputElement>('#library-url');
        expect(input).not.toBeNull();
        return input!;
      });
      url.value = 'https://example.com/design';
      url.dispatchEvent(new Event('input', { bubbles: true }));

      const submit = [...document.body.querySelectorAll<HTMLButtonElement>('button')].find(
        (button) => button.textContent?.trim() === 'Save',
      )!;
      await vi.waitFor(() => expect(submit.disabled).toBe(false));
      submit.click();
      submit.click();
      await vi.waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
      expect(onCreate).toHaveBeenCalledWith({ url: 'https://example.com/design' });
      expect(submit.disabled).toBe(true);
      expect(submit.textContent?.trim()).toBe('Saving…');

      resolveCreate?.(libraryItem());
      await vi.waitFor(() => expect(document.body.querySelector('#library-url')).toBeNull());
      expect(document.activeElement).toBe(trigger);
    } finally {
      await unmount(component);
    }
  });

  it('rejects an invalid URL submitted with Command + Enter', async () => {
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    const onCreate = vi.fn();
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(LibraryCaptureForm, {
      props: { onCreate },
      target,
    });

    try {
      await tick();
      const trigger = target.querySelector<HTMLButtonElement>('[aria-label="Add a link"]')!;
      trigger.click();
      const url = await vi.waitFor(() => {
        const input = document.body.querySelector<HTMLInputElement>('#library-url');
        expect(input).not.toBeNull();
        return input!;
      });
      url.value = 'ftp://example.com/file';
      url.dispatchEvent(new Event('input', { bubbles: true }));
      url.dispatchEvent(
        new KeyboardEvent('keydown', { bubbles: true, key: 'Enter', metaKey: true }),
      );

      await vi.waitFor(() =>
        expect(document.body.textContent).toContain('Enter a complete http or https URL.'),
      );
      expect(onCreate).not.toHaveBeenCalled();
      expect(document.activeElement).toBe(url);
    } finally {
      await unmount(component);
    }
  });
});

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
    tags: ['Architecture', 'rust'],
    title: 'A calm system design',
    uid: 'library-1',
    updatedAt: '2026-08-24T01:00:00.000Z',
  };
}
