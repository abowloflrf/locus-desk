import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Note } from '../api/types';
import NoteComposer from './NoteComposer.svelte';

afterEach(() => {
  document.body.replaceChildren();
});

describe('note composer submission', () => {
  it('keeps its initial size on focus and preserves the draft after a failed save', async () => {
    const onCreate = vi.fn().mockRejectedValue(new Error('Connection lost'));
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NoteComposer, { props: { onCreate }, target });
    try {
      const textarea = target.querySelector<HTMLTextAreaElement>('textarea')!;
      expect(textarea.rows).toBe(1);
      textarea.focus();
      await tick();
      expect(textarea.rows).toBe(1);
      textarea.blur();
      await tick();
      expect(textarea.rows).toBe(1);
      textarea.value = 'Keep this draft';
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
      const save = target.querySelector<HTMLButtonElement>('.composer-submit')!;
      await vi.waitFor(() => expect(save.disabled).toBe(false));
      save.click();
      await vi.waitFor(() => expect(target.textContent).toContain('Connection lost'));
      expect(textarea.value).toBe('Keep this draft');
      expect(textarea.rows).toBe(1);
      await vi.waitFor(() => expect(document.activeElement).toBe(textarea));
    } finally {
      await unmount(component);
    }
  });

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
      const submitButton = target.querySelector<HTMLButtonElement>('.composer-submit')!;
      expect(target.querySelector('#composer-hint')).toBeNull();
      expect(submitButton.closest('[data-slot="input-group"]')).not.toBeNull();
      expect(submitButton.textContent?.trim()).toBe('Save');
      await vi.waitFor(() => expect(submitButton.disabled).toBe(false));
      submitButton.click();
      await vi.waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
      expect(onCreate).toHaveBeenCalledWith(submittedDraft);

      expect(textarea.disabled).toBe(true);
      expect(submitButton.disabled).toBe(true);
      expect(submitButton.textContent?.trim()).toBe('Saving…');
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
