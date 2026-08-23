import type { Note, Task } from '../api/types';

const dateFormatterCache = new Map<string, Intl.DateTimeFormat>();

export interface NoteGroup {
  key: string;
  label: string;
  items: Note[];
}

export function groupNotesByMonth(notes: Note[], timeZone: string): NoteGroup[] {
  const groups = new Map<string, NoteGroup>();

  for (const note of sortNotesByCreatedAt(notes)) {
    const date = new Date(note.createdAt);
    const key = formatMonthKey(date, timeZone);
    let group = groups.get(key);
    if (!group) {
      group = {
        items: [],
        key,
        label: getFormatter(timeZone, 'month').format(date),
      };
      groups.set(key, group);
    }
    group.items.push(note);
  }

  return [...groups.values()];
}

export function sortNotesByCreatedAt(notes: Note[]): Note[] {
  return [...notes].sort(
    (left, right) =>
      right.createdAt.localeCompare(left.createdAt) || right.uid.localeCompare(left.uid),
  );
}

export function formatNoteTimestamp(value: string, timeZone: string): string {
  const parts = getFormatter(timeZone, 'timestamp').formatToParts(new Date(value));
  const dateTime = Object.fromEntries(parts.map((part) => [part.type, part.value]));
  return `${dateTime.month}/${dateTime.day} · ${dateTime.hour}:${dateTime.minute}`;
}

export function formatTodayLabel(today: string): string {
  return new Intl.DateTimeFormat('en', {
    day: 'numeric',
    month: 'short',
    timeZone: 'UTC',
    weekday: 'short',
  }).format(new Date(`${today}T00:00:00Z`));
}

export function taskDateLabel(task: Task, today: string): string | null {
  if (!task.dueDate) return null;
  if (task.dueDate === today) return task.dueTime ?? 'Today';

  const date = new Intl.DateTimeFormat('en', {
    day: 'numeric',
    month: 'short',
    timeZone: 'UTC',
  }).format(new Date(`${task.dueDate}T00:00:00Z`));

  if (task.status === 'TODO' && task.dueDate < today) {
    return task.dueTime ? `Overdue · ${date} · ${task.dueTime}` : `Overdue · ${date}`;
  }

  return task.dueTime ? `${date} · ${task.dueTime}` : date;
}

export function isTaskOverdue(task: Task, today: string): boolean {
  return task.status === 'TODO' && Boolean(task.dueDate && task.dueDate < today);
}

function getFormatter(timeZone: string, type: 'month' | 'timestamp'): Intl.DateTimeFormat {
  const key = `${timeZone}:${type}`;
  const cached = dateFormatterCache.get(key);
  if (cached) return cached;

  const formatter =
    type === 'month'
      ? new Intl.DateTimeFormat('en', {
          month: 'long',
          timeZone,
          year: 'numeric',
        })
      : new Intl.DateTimeFormat('en', {
          day: '2-digit',
          hour: '2-digit',
          hourCycle: 'h23',
          minute: '2-digit',
          month: '2-digit',
          timeZone,
        });

  dateFormatterCache.set(key, formatter);
  return formatter;
}

function formatMonthKey(date: Date, timeZone: string): string {
  const parts = new Intl.DateTimeFormat('en-CA', {
    month: '2-digit',
    timeZone,
    year: 'numeric',
  }).formatToParts(date);
  const value = Object.fromEntries(parts.map((part) => [part.type, part.value]));
  return `${value.year}-${value.month}`;
}
