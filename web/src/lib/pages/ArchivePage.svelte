<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import { onMount } from 'svelte';

  import { errorMessage } from '../api/client';
  import { deleteNote, listNotes, updateNote } from '../api/notes';
  import type { Note, SessionInfo } from '../api/types';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import NoteTimeline from '../components/NoteTimeline.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { Button } from '../components/ui/button';
  import * as Empty from '../components/ui/empty';
  import { Input } from '../components/ui/input';
  import { Spinner } from '../components/ui/spinner';
  import { captureListFocus, restoreListFocus, type ListFocusSnapshot } from '../utils/focus';

  let { session }: { session: SessionInfo } = $props();

  let notes = $state<Note[]>([]);
  let query = $state('');
  let page = $state(1);
  let total = $state(0);
  let loading = $state(true);
  let loadingMore = $state(false);
  let loadError = $state<string | null>(null);
  let operationError = $state<string | null>(null);
  let actionStatus = $state<string | null>(null);
  let busyUids = $state<Set<string>>(new Set());
  let pendingDelete = $state<Note | null>(null);
  let deleteBusy = $state(false);
  let deleteError = $state<string | null>(null);
  let pageElement = $state<HTMLElement>();
  let controller: AbortController | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let deleteFocusSnapshot: ListFocusSnapshot | null = null;
  let requestId = 0;

  onMount(() => {
    void loadArchive(true);
    return () => {
      controller?.abort();
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  async function loadArchive(reset: boolean): Promise<void> {
    const nextPage = reset ? 1 : page + 1;
    const id = ++requestId;
    controller?.abort();
    controller = new AbortController();
    if (reset) loading = true;
    else loadingMore = true;
    loadError = null;
    try {
      const response = await listNotes(
        { page: nextPage, pageSize: 30, q: query, status: 'ARCHIVED' },
        controller.signal,
      );
      if (id !== requestId) return;
      notes = reset ? response.items : [...notes, ...response.items];
      page = response.page;
      total = response.total;
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === requestId) loadError = errorMessage(cause, 'Unable to load the archive.');
    } finally {
      if (id === requestId) {
        loading = false;
        loadingMore = false;
      }
    }
  }

  function handleSearch(event: Event): void {
    query = (event.currentTarget as HTMLInputElement).value;
    if (searchTimer) clearTimeout(searchTimer);
    invalidateArchiveRequest();
    loading = true;
    searchTimer = setTimeout(() => void loadArchive(true), 250);
  }

  async function handleRestore(note: Note): Promise<void> {
    const focusSnapshot = captureListFocus(pageElement, note.uid);
    invalidateArchiveRequest();
    actionStatus = null;
    const wasVisible = notes.some((item) => item.uid === note.uid);
    notes = notes.filter((item) => item.uid !== note.uid);
    if (wasVisible) total = Math.max(0, total - 1);
    markBusy(note.uid, true);
    operationError = null;
    await restoreListFocus(pageElement, focusSnapshot);
    try {
      await updateNote(note.uid, { status: 'ACTIVE' });
      const returnedDuringRequest = notes.some((item) => item.uid === note.uid);
      notes = notes.filter((item) => item.uid !== note.uid);
      if (returnedDuringRequest) total = Math.max(0, total - 1);
      actionStatus = 'Memo restored.';
      void loadArchive(true);
    } catch (cause) {
      const matchesQuery =
        !query.trim() ||
        note.content.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
      if (matchesQuery && !notes.some((item) => item.uid === note.uid)) {
        notes = [...notes, note].sort((left, right) =>
          right.createdAt.localeCompare(left.createdAt),
        );
        total += 1;
      }
      operationError = errorMessage(cause, 'Unable to restore the memo.');
      void loadArchive(true);
    } finally {
      markBusy(note.uid, false);
    }
  }

  async function handleSave(note: Note, content: string): Promise<void> {
    const focusSnapshot = captureListFocus(pageElement, note.uid);
    invalidateArchiveRequest();
    markBusy(note.uid, true);
    operationError = null;
    actionStatus = null;
    try {
      const updated = await updateNote(note.uid, { content });
      if (matchesCurrentQuery(updated)) {
        notes = notes.map((item) => (item.uid === updated.uid ? updated : item));
      } else {
        const wasVisible = notes.some((item) => item.uid === updated.uid);
        notes = notes.filter((item) => item.uid !== updated.uid);
        if (wasVisible) {
          total = Math.max(0, total - 1);
          actionStatus = 'Memo updated and removed from the current view.';
          await restoreListFocus(pageElement, focusSnapshot);
        }
      }
      void loadArchive(true);
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to save the memo.');
      void loadArchive(true);
      throw cause;
    } finally {
      markBusy(note.uid, false);
    }
  }

  async function confirmDelete(): Promise<void> {
    if (!pendingDelete) return;
    const note = pendingDelete;
    invalidateArchiveRequest();
    deleteBusy = true;
    deleteError = null;
    try {
      await deleteNote(note.uid);
      notes = notes.filter((item) => item.uid !== note.uid);
      total = Math.max(0, total - 1);
      pendingDelete = null;
      deleteError = null;
      actionStatus = 'Memo deleted.';
      void loadArchive(true);
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete the memo.');
      void loadArchive(true);
    } finally {
      deleteBusy = false;
    }
  }

  function requestDelete(note: Note): void {
    deleteFocusSnapshot = captureListFocus(pageElement, note.uid);
    deleteError = null;
    actionStatus = null;
    pendingDelete = note;
  }

  function cancelDelete(): void {
    deleteFocusSnapshot = null;
    deleteError = null;
    pendingDelete = null;
  }

  async function focusAfterDelete(): Promise<void> {
    const snapshot = deleteFocusSnapshot;
    deleteFocusSnapshot = null;
    if (snapshot) await restoreListFocus(pageElement, snapshot);
    else document.getElementById('main-content')?.focus();
  }

  function matchesCurrentQuery(note: Note): boolean {
    return (
      !query.trim() || note.content.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())
    );
  }

  function markBusy(uid: string, busy: boolean): void {
    const next = new Set(busyUids);
    if (busy) next.add(uid);
    else next.delete(uid);
    busyUids = next;
  }

  function invalidateArchiveRequest(): void {
    requestId += 1;
    controller?.abort();
    controller = null;
    loading = false;
    loadingMore = false;
  }
</script>

<div bind:this={pageElement} class="page archive-page">
  <header class="page-header">
    <div>
      <p class="eyebrow">Memos</p>
      <h1>Archive</h1>
      <p class="page-description">Memos kept out of the active timeline.</p>
    </div>
    <label class="search-field compact-search">
      <Search class="pointer-events-none absolute left-3 size-4 text-muted-foreground" />
      <span class="sr-only">Search archived memos</span>
      <Input
        class="pl-9"
        oninput={handleSearch}
        placeholder="Search archive"
        type="search"
        value={query}
      />
    </label>
  </header>

  {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
  <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
    {actionStatus ?? ''}
  </div>
  <div class="list-toolbar">
    <span>{total} archived {total === 1 ? 'memo' : 'memos'}</span>
    {#if loading && notes.length > 0}<span aria-live="polite">Updating…</span>{/if}
  </div>

  {#if loading && notes.length === 0}
    <div aria-live="polite" class="loading-state large flex items-center justify-center gap-2">
      <Spinner />
      Loading archive…
    </div>
  {:else if loadError}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>Archive unavailable</Empty.Title>
        <Empty.Description>{loadError}</Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <Button onclick={() => void loadArchive(true)} variant="secondary">Try again</Button>
      </Empty.Content>
    </Empty.Root>
  {:else if notes.length === 0}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>{query ? 'No archived memos found' : 'Nothing archived yet'}</Empty.Title>
        <Empty.Description
          >{query
            ? 'Try another keyword.'
            : 'Archived memos will stay available here.'}</Empty.Description
        >
      </Empty.Header>
    </Empty.Root>
  {:else}
    <NoteTimeline
      {busyUids}
      mode="archive"
      {notes}
      onDelete={requestDelete}
      onRestore={handleRestore}
      onSave={handleSave}
      timeZone={session.workspace.timezone}
    />
    {#if notes.length < total}
      <Button
        class="load-more"
        disabled={loadingMore}
        onclick={() => void loadArchive(false)}
        variant="secondary"
      >
        {#if loadingMore}<Spinner data-icon="inline-start" />{/if}
        {loadingMore ? 'Loading…' : 'Load older memos'}
      </Button>
    {/if}
  {/if}
</div>

<ConfirmDialog
  busy={deleteBusy}
  confirmLabel="Delete memo"
  error={deleteError}
  message="This archived memo will be permanently deleted."
  onCancel={cancelDelete}
  onConfirm={confirmDelete}
  onFallbackFocus={focusAfterDelete}
  open={Boolean(pendingDelete)}
  title="Delete this memo?"
/>
