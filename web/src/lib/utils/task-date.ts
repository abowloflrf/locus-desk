import { getDayOfWeek, parseDate } from '@internationalized/date';

export function taskDateShortcuts(today: string) {
  const date = parseDate(today);
  return {
    today,
    tomorrow: date.add({ days: 1 }).toString(),
    // Target dates are calendar dates in the workspace timezone; weeks end on Sunday.
    thisWeek: date.add({ days: 6 - getDayOfWeek(date, 'en-GB') }).toString(),
  };
}

export function targetDateLabel(value: string | null, today: string): string | null {
  if (!value) return null;
  if (value === today) return 'Today';
  if (value === taskDateShortcuts(today).tomorrow) return 'Tomorrow';
  return new Intl.DateTimeFormat('en', {
    month: 'short',
    day: 'numeric',
    ...(value.slice(0, 4) !== today.slice(0, 4) ? { year: 'numeric' as const } : {}),
    timeZone: 'UTC',
  }).format(new Date(`${value}T00:00:00Z`));
}
