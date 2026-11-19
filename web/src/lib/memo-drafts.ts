import { getContext, setContext } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';
import type { SessionInfo } from './api/types';

const contextKey = Symbol('memo drafts');
const draftValues = new SvelteMap<string, string | null>();

function draftStorageKey(session: SessionInfo, uid: string): string {
  return `locus:memo-draft:${session.user.uid}:${session.workspace.uid}:${uid}`;
}

export function provideMemoDrafts(getSession: () => SessionInfo) {
  const drafts = {
    read(uid: string): string | null {
      const key = draftStorageKey(getSession(), uid);
      if (draftValues.has(key)) return draftValues.get(key) ?? null;
      try {
        return sessionStorage.getItem(key);
      } catch {
        return null;
      }
    },
    write(uid: string, content: string | null): void {
      const key = draftStorageKey(getSession(), uid);
      draftValues.set(key, content);
      try {
        if (content === null) sessionStorage.removeItem(key);
        else sessionStorage.setItem(key, content);
      } catch {
        // The tab still retains the latest draft when storage is unavailable.
      }
    },
  };
  setContext(contextKey, drafts);
  return drafts;
}

export function getMemoDrafts() {
  return getContext<ReturnType<typeof provideMemoDrafts> | undefined>(contextKey);
}
