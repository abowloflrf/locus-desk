import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Note } from '../api/types';
import NoteTimeline from './NoteTimeline.svelte';

afterEach(() => {
  document.body.replaceChildren();
});

describe('NoteTimeline ordering', () => {
  it('keeps pinned notes first and orders every section newest first', async () => {
    const target = document.createElement('div');
    document.body.append(target);
    const component = mount(NoteTimeline, {
      props: {
        busyUids: new Set<string>(),
        notes: [
          note('older', '2026-07-22T08:00:00.000Z'),
          note('pinned-old', '2026-08-20T08:00:00.000Z', true),
          note('newest', '2026-08-24T08:00:00.000Z'),
          note('pinned-new', '2026-08-23T08:00:00.000Z', true),
        ],
        onDelete: vi.fn(),
        onSave: vi.fn().mockResolvedValue(undefined),
        timeZone: 'Asia/Singapore',
      },
      target,
    });

    try {
      expect(
        [...target.querySelectorAll<HTMLElement>('[data-focus-uid]')].map(
          (element) => element.dataset.focusUid,
        ),
      ).toEqual(['pinned-new', 'pinned-old', 'newest', 'older']);
      expect(target.querySelector('.pinned-notes > h2')?.textContent).toBe('Pinned');
      expect(
        [...target.querySelectorAll('.note-day:not(.pinned-notes) > h2')].map(
          (heading) => heading.textContent,
        ),
      ).toEqual(['August 2026', 'July 2026']);
    } finally {
      await unmount(component);
    }
  });
});

function note(uid: string, createdAt: string, pinned = false): Note {
  return {
    content: uid,
    createdAt,
    pinned,
    status: 'ACTIVE',
    tags: [],
    uid,
    updatedAt: createdAt,
  };
}
