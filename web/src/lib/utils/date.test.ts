import { describe, expect, it } from 'vitest';

import { formatNoteTimestamp } from './date';

describe('formatNoteTimestamp', () => {
  it('shows a compact numeric date and time in the workspace timezone', () => {
    expect(formatNoteTimestamp('2026-08-23T16:30:00.000Z', 'Asia/Singapore')).toBe('08/24 · 00:30');
  });
});
