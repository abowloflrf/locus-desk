import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import TaskCreateForm from './TaskCreateForm.svelte';

afterEach(() => {
  document.body.replaceChildren();
});

describe('Todo task creation', () => {
  it('keeps the quick form to one line and leaves the due date optional', async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      props: { busy: false, mode: 'todo', onCreate },
      target,
    });

    try {
      expect(target.querySelector('button[type="submit"]')).toBeNull();
      expect(target.querySelector('[aria-label="Due time"]')).toBeNull();

      const input = target.querySelector<HTMLInputElement>('input:not([type="date"])')!;
      input.value = 'Plan tomorrow';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      target.querySelector<HTMLFormElement>('form')!.requestSubmit();

      await vi.waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
      const payload = onCreate.mock.calls[0][0];
      expect(payload).toEqual({
        description: undefined,
        priority: 0,
        title: 'Plan tomorrow',
      });
      expect(payload).not.toHaveProperty('dueDate');
      expect(payload).not.toHaveProperty('dueTime');
    } finally {
      await unmount(component);
    }
  });

  it('reveals deadline and priority controls that update the created task', async () => {
    const onCreate = vi.fn().mockResolvedValue(undefined);
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(TaskCreateForm, {
      props: { busy: false, mode: 'todo', onCreate },
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

      await vi.waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
      const payload = onCreate.mock.calls[0][0];
      expect(payload).toEqual({
        description: undefined,
        dueDate: '2026-08-25',
        priority: 1,
        title: 'Send the report',
      });
      expect(payload).not.toHaveProperty('dueTime');
    } finally {
      await unmount(component);
    }
  });
});
