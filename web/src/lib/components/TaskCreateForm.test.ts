import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import TaskCreateForm from './TaskCreateForm.svelte';

afterEach(() => {
  document.body.replaceChildren();
});

describe('today task creation', () => {
  it('keeps the quick form to one line and submits with today defaults', async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      props: { busy: false, mode: 'today', onCreate },
      target,
    });

    try {
      expect(target.querySelector('button[type="submit"]')).toBeNull();
      expect(target.querySelector('[aria-label="Due time"]')).toBeNull();

      const input = target.querySelector<HTMLInputElement>('input:not([type="date"])')!;
      input.value = 'Plan tomorrow';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      target.querySelector<HTMLFormElement>('form')!.requestSubmit();

      await vi.waitFor(() =>
        expect(onCreate).toHaveBeenCalledWith({
          description: undefined,
          dueDate: undefined,
          dueTime: undefined,
          priority: 0,
          title: 'Plan tomorrow',
        }),
      );
    } finally {
      await unmount(component);
    }
  });

  it('reveals deadline and priority controls that update the created task', async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      props: { busy: false, mode: 'today', onCreate },
      target,
    });

    try {
      const titleInput = target.querySelector<HTMLInputElement>('input:not([type="date"])')!;
      titleInput.focus();
      await vi.waitFor(() =>
        expect(target.querySelector('.quick-actions')?.classList.contains('visible')).toBe(true),
      );

      const dateInput = target.querySelector<HTMLInputElement>('[aria-label="Due date"]')!;
      dateInput.value = '2026-08-25';
      dateInput.dispatchEvent(new Event('input', { bubbles: true }));

      target.querySelector<HTMLButtonElement>('[aria-label="Set priority"]')!.click();
      await vi.waitFor(() => expect(target.querySelector('[role="menu"]')).not.toBeNull());
      target.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')[1].click();

      titleInput.value = 'Send the report';
      titleInput.dispatchEvent(new Event('input', { bubbles: true }));
      target.querySelector<HTMLFormElement>('form')!.requestSubmit();

      await vi.waitFor(() =>
        expect(onCreate).toHaveBeenCalledWith({
          description: undefined,
          dueDate: '2026-08-25',
          dueTime: undefined,
          priority: 1,
          title: 'Send the report',
        }),
      );
    } finally {
      await unmount(component);
    }
  });
});
