import { describe, expect, it } from 'vitest';
import { targetDateLabel, taskDateShortcuts } from './task-date';

describe('workspace task dates', () => {
  it('ends this week on Sunday, including Sunday itself and year boundaries', () => {
    expect(taskDateShortcuts('2026-09-05')).toEqual({
      today: '2026-09-05',
      tomorrow: '2026-09-06',
      thisWeek: '2026-09-06',
    });
    expect(taskDateShortcuts('2026-09-06').thisWeek).toBe('2026-09-06');
    expect(taskDateShortcuts('2026-12-31')).toEqual({
      today: '2026-12-31',
      tomorrow: '2027-01-01',
      thisWeek: '2027-01-03',
    });
  });
  it('labels optional target dates without treating past plans as deadlines', () => {
    expect(targetDateLabel(null, '2026-09-05')).toBeNull();
    expect(targetDateLabel('2026-09-05', '2026-09-05')).toBe('Today');
    expect(targetDateLabel('2026-09-06', '2026-09-05')).toBe('Tomorrow');
    expect(targetDateLabel('2026-09-04', '2026-09-05')).toBe('Sep 4');
    expect(targetDateLabel('2027-01-01', '2026-09-05')).toBe('Jan 1, 2027');
  });
});
