import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import TaskCreateForm from './TaskCreateForm.svelte';

afterEach(() => {
  document.body.replaceChildren();
});

function inputTitle(target: HTMLElement, text: string) {
  const input = target.querySelector<HTMLInputElement>('input')!;
  input.value = text;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  return input;
}

describe('Task quick creation', () => {
  it('creates an undated task once and restores focus for the next entry', async () => {
    let finish!: () => void;
    const onCreate = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      target,
      props: { busy: false, mode: 'todo', today: '2026-09-05', onCreate },
    });
    try {
      const input = inputTitle(target, 'Write a note');
      const form = target.querySelector('form')!;
      form.requestSubmit();
      form.requestSubmit();
      expect(onCreate).toHaveBeenCalledExactlyOnceWith({ title: 'Write a note', priority: 0 });
      await tick();
      expect(input.disabled).toBe(true);
      finish();
      await vi.waitFor(() => expect(input.value).toBe(''));
      expect(document.activeElement).toBe(input);
    } finally {
      await unmount(component);
    }
  });

  it('uses the shared date and priority menu and clears attributes after creating', async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      target,
      props: { busy: false, mode: 'all', today: '2026-09-05', onCreate },
    });
    try {
      const input = inputTitle(target, 'Weekly review');
      await tick();
      target.querySelector<HTMLButtonElement>('[aria-label="New task options"]')!.click();
      const tomorrow = await vi.waitFor(() => {
        const button = [...document.body.querySelectorAll<HTMLButtonElement>('button')].find(
          (b) => b.getAttribute('aria-label') === 'Tomorrow',
        );
        expect(button).toBeDefined();
        return button!;
      });
      expect(tomorrow.textContent?.trim()).toBe('');
      tomorrow.click();
      await vi.waitFor(() =>
        expect(target.querySelector('.creation-metadata')?.textContent).toContain('Tomorrow'),
      );
      document.body
        .querySelector<HTMLButtonElement>(
          '[data-slot="toggle-group-item"][aria-label="High priority"]',
        )!
        .click();
      await vi.waitFor(() =>
        expect(target.querySelector('[role="img"][aria-label="High priority"]')).not.toBeNull(),
      );
      target.querySelector('form')!.requestSubmit();
      await vi.waitFor(() =>
        expect(onCreate).toHaveBeenCalledExactlyOnceWith({
          title: 'Weekly review',
          dueDate: '2026-09-06',
          priority: 1,
        }),
      );
      await vi.waitFor(() => expect(input.value).toBe(''));
      expect(target.querySelector('.creation-metadata')).toBeNull();
      expect(target.querySelector('input[type="time"]')).toBeNull();
    } finally {
      await unmount(component);
    }
  });

  it('keeps a failed draft and supports retry with the keyboard', async () => {
    const onCreate = vi
      .fn()
      .mockRejectedValueOnce(new Error('Offline'))
      .mockResolvedValueOnce(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      target,
      props: { busy: false, mode: 'all', today: '2026-09-05', onCreate },
    });
    try {
      const input = inputTitle(target, 'Keep this draft');
      target.querySelector('form')!.requestSubmit();
      await vi.waitFor(() => expect(target.textContent).toContain('Offline'));
      expect(input.value).toBe('Keep this draft');
      input.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Enter',
          ctrlKey: true,
          bubbles: true,
          cancelable: true,
        }),
      );
      await vi.waitFor(() => expect(input.value).toBe(''));
      expect(onCreate).toHaveBeenCalledTimes(2);
    } finally {
      await unmount(component);
    }
  });
});
