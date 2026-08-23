import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Note } from '../api/types';
import NoteComposer from './NoteComposer.svelte';

afterEach(() => {
  document.body.replaceChildren();
});

describe('note composer submission', () => {
  it('prevents duplicate submissions and preserves a changed draft while posting', async () => {
    let resolveCreate: ((note: Note) => void) | undefined;
    const onCreate = vi.fn(
      () =>
        new Promise<Note>((resolve) => {
          resolveCreate = resolve;
        }),
    );
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NoteComposer, { props: { onCreate }, target });

    try {
      const textarea = target.querySelector<HTMLTextAreaElement>('textarea')!;
      const submittedDraft = '    Submitted draft  \n';
      textarea.value = submittedDraft;
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
      const submitButton = target.querySelector<HTMLButtonElement>('.composer-submit button')!;
      await vi.waitFor(() => expect(submitButton.disabled).toBe(false));
      submitButton.click();
      await vi.waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
      expect(onCreate).toHaveBeenCalledWith(submittedDraft);

      expect(textarea.disabled).toBe(true);
      expect(submitButton.disabled).toBe(true);
      submitButton.click();
      expect(onCreate).toHaveBeenCalledOnce();

      textarea.value = 'New draft';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
      resolveCreate?.({
        content: submittedDraft,
        createdAt: '2026-08-23T12:00:00.000Z',
        pinned: false,
        status: 'ACTIVE',
        tags: [],
        uid: 'note-1',
        updatedAt: '2026-08-23T12:00:00.000Z',
      });
      await vi.waitFor(() => expect(textarea.value).toBe('New draft'));
    } finally {
      await unmount(component);
    }
  });
});
