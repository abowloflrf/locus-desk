import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { LibraryItem } from '../api/types';
import LibraryCaptureForm from './LibraryCaptureForm.svelte';

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('LibraryCaptureForm', () => {
  it('submits one URL once, then closes the modal and restores trigger focus', async () => {
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
      const dialog = document.body.querySelector<HTMLElement>('[role="dialog"]')!;
      expect(dialog.getAttribute('aria-modal')).toBe('true');
      expect(document.body.querySelector('[data-slot="dialog-overlay"]')).not.toBeNull();
      await vi.waitFor(() => expect(document.activeElement).toBe(url));
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
      dialog.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
      );
      await tick();
      expect(document.body.querySelector('[role="dialog"]')).toBe(dialog);

      resolveCreate?.(libraryItem());
      await vi.waitFor(() => expect(document.body.querySelector('#library-url')).toBeNull());
      expect(document.activeElement).toBe(trigger);
    } finally {
      await unmount(component);
    }
  });

  it('dismisses with Escape, restores focus, and retains the unsaved URL', async () => {
    const onCreate = vi.fn();
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
      await vi.waitFor(() => expect(document.activeElement).toBe(url));
      url.value = 'https://example.com/draft';
      url.dispatchEvent(new Event('input', { bubbles: true }));
      url.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
      );
      await vi.waitFor(() => expect(document.body.querySelector('[role="dialog"]')).toBeNull());
      await vi.waitFor(() => expect(document.activeElement).toBe(trigger));
      expect(onCreate).not.toHaveBeenCalled();
      trigger.click();
      await vi.waitFor(() =>
        expect(document.body.querySelector<HTMLInputElement>('#library-url')?.value).toBe(
          'https://example.com/draft',
        ),
      );
      document.body.querySelector<HTMLButtonElement>('[data-slot="dialog-close"]')!.click();
      await vi.waitFor(() => expect(document.body.querySelector('[role="dialog"]')).toBeNull());
      await vi.waitFor(() => expect(document.activeElement).toBe(trigger));
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
      expect(url.getAttribute('aria-invalid')).toBe('true');
      const message = document.getElementById(url.getAttribute('aria-describedby')!);
      expect(message?.getAttribute('role')).toBe('alert');
      expect(message?.textContent).toContain('Enter a complete http or https URL.');
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
