import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { LibraryItem } from '../api/types';
import LibraryCaptureForm from './LibraryCaptureForm.svelte';

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

describe('LibraryCaptureForm', () => {
  it('submits the full manual capture once, then resets focus for another link', async () => {
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
      const contextToggle = target.querySelector<HTMLButtonElement>('.context-toggle')!;
      contextToggle.click();
      await tick();
      const values: Record<string, string> = {
        '#library-note': 'Useful for the shared object decision.',
        '#library-selection': 'Keep strong domain tables.',
        '#library-tags': 'Architecture, rust, architecture',
        '#library-title-input': 'A calm system design',
        '#library-url': 'https://example.com/design',
      };
      for (const [selector, value] of Object.entries(values)) {
        const field = target.querySelector<HTMLInputElement | HTMLTextAreaElement>(selector)!;
        field.value = value;
        field.dispatchEvent(new Event('input', { bubbles: true }));
      }

      const submit = target.querySelector<HTMLButtonElement>('.save-link')!;
      await vi.waitFor(() => expect(submit.disabled).toBe(false));
      submit.click();
      submit.click();
      await vi.waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
      expect(onCreate).toHaveBeenCalledWith({
        note: 'Useful for the shared object decision.',
        selection: 'Keep strong domain tables.',
        tags: ['Architecture', 'rust'],
        title: 'A calm system design',
        url: 'https://example.com/design',
      });
      expect(submit.disabled).toBe(true);
      expect(submit.textContent?.trim()).toBe('Saving…');

      resolveCreate?.(libraryItem());
      await vi.waitFor(() =>
        expect(target.querySelector('.status-success')?.textContent).toContain(
          'Saved “A calm system design” to Library.',
        ),
      );
      const urlInput = target.querySelector<HTMLInputElement>('#library-url')!;
      expect(urlInput.value).toBe('');
      expect(document.activeElement).toBe(urlInput);
      expect(target.querySelector('#library-capture-context')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('collapses optional context with Escape and restores the toggle focus', async () => {
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(LibraryCaptureForm, {
      props: { onCreate: vi.fn() },
      target,
    });

    try {
      const toggle = target.querySelector<HTMLButtonElement>('.context-toggle')!;
      toggle.click();
      await tick();
      const note = target.querySelector<HTMLTextAreaElement>('#library-note')!;
      note.focus();
      note.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));

      await vi.waitFor(() => expect(target.querySelector('#library-capture-context')).toBeNull());
      expect(document.activeElement).toBe(toggle);
      expect(toggle.getAttribute('aria-expanded')).toBe('false');
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
