import { tick } from 'svelte';

export interface ListFocusSnapshot {
  neighborUid: string | null;
  restore: boolean;
}

export function captureListScroll(root: HTMLElement | undefined): (() => void) | undefined {
  const scroller = root?.closest<HTMLElement>('.workspace-column');
  if (!scroller?.scrollTop) return;
  const viewport = scroller.getBoundingClientRect();
  const row = focusRows(root).find((item) => item.getBoundingClientRect().bottom > viewport.top);
  const uid = row?.dataset.focusUid;
  const offset = row ? row.getBoundingClientRect().top - viewport.top : null;
  if (!uid || offset === null) return;

  return () => {
    const current = focusRows(root).find((item) => item.dataset.focusUid === uid);
    if (current) {
      scroller.scrollTop +=
        current.getBoundingClientRect().top - scroller.getBoundingClientRect().top - offset;
    }
  };
}

export async function focusAppendedRow(
  root: HTMLElement | undefined,
  control: Element,
  uid: string,
): Promise<void> {
  await tick();
  if (document.activeElement !== control && control.isConnected) return;
  if (!control.isConnected && document.activeElement !== document.body) return;
  const row = focusRows(root).find((item) => item.dataset.focusUid === uid);
  if (row?.hasAttribute('tabindex')) row.focus();
  else row?.querySelector<HTMLElement>(focusableSelector)?.focus();
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
