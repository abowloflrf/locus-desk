import { mount, tick, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import NoteItem from './NoteItem.svelte';

let component: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

it('opens overflowing memos in a dialog and keeps the timeline collapsed', async () => {
  let measure = () => {};
  let height = 500;
  const disconnect = vi.fn();
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(callback: () => void) {
        measure = callback;
      }
      observe(node: Element) {
        Object.defineProperty(node, 'scrollHeight', { get: () => height });
      }
      disconnect = disconnect;
    },
  );
  const target = document.createElement('div');
  document.body.append(target);
  component = mount(NoteItem, {
    target,
    props: {
      note: {
        uid: 'long-note',
        content: 'Memo with [a link](https://example.com)',
        pinned: true,
        status: 'ACTIVE',
        tags: [],
        createdAt: '2026-09-05T09:00:00Z',
        updatedAt: '2026-09-05T09:00:00Z',
      },
      timeZone: 'UTC',
      busy: false,
      onSave: vi.fn(),
      onDelete: vi.fn(),
    },
  });
  await tick();
  measure();
  await tick();
  expect(target.querySelector('.pin-label')).toBeNull();
  const button = target.querySelector<HTMLButtonElement>('.memo-expand')!;
  expect(button.textContent).toContain('More');
  expect(button.getAttribute('aria-haspopup')).toBe('dialog');
  expect(target.querySelector('.note-preview.collapsed')).not.toBeNull();
  button.click();
  await tick();
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).not.toBeNull());
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('Memo with');
  expect(target.querySelector('.note-preview.collapsed')).not.toBeNull();
  const close = document.querySelector<HTMLButtonElement>('[data-slot="dialog-close"]')!;
  close.click();
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  await vi.waitFor(() => expect(document.activeElement).toBe(button));
  height = 100;
  measure();
  await tick();
  expect(target.querySelector('.memo-expand')).toBeNull();
  await unmount(component);
  component = undefined;
  expect(disconnect).toHaveBeenCalledOnce();
});
