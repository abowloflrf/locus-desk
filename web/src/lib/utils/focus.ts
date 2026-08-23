import { tick } from 'svelte';

export interface ListFocusSnapshot {
  neighborUid: string | null;
  restore: boolean;
}

const focusableSelector = [
  'button:not([disabled])',
  'a[href]',
  'input:not([disabled])',
  'textarea:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

export function captureListFocus(root: HTMLElement | undefined, uid: string): ListFocusSnapshot {
  const rows = focusRows(root);
  const index = rows.findIndex((row) => row.dataset.focusUid === uid);
  const activeRow = index >= 0 ? rows[index] : undefined;

  return {
    neighborUid:
      index >= 0
        ? (rows[index + 1]?.dataset.focusUid ?? rows[index - 1]?.dataset.focusUid ?? null)
        : null,
    restore: Boolean(activeRow),
  };
}

export async function restoreListFocus(
  root: HTMLElement | undefined,
  snapshot: ListFocusSnapshot,
  preferredUid: string | null = null,
): Promise<void> {
  if (!snapshot.restore) return;

  await tick();
  const rows = focusRows(root);
  const target = [preferredUid, snapshot.neighborUid]
    .filter((uid): uid is string => Boolean(uid))
    .map((uid) => rows.find((row) => row.dataset.focusUid === uid))
    .find((row): row is HTMLElement => Boolean(row));
  const focusable = target?.querySelector<HTMLElement>(focusableSelector);

  if (focusable) focusable.focus();
  else if (target) target.focus();
  else document.getElementById('main-content')?.focus();
}

function focusRows(root: HTMLElement | undefined): HTMLElement[] {
  return root ? [...root.querySelectorAll<HTMLElement>('[data-focus-uid]')] : [];
}
