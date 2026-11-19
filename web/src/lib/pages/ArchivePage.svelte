<script lang="ts">
  import SearchField from '../components/SearchField.svelte';
  import { onMount, tick, untrack } from 'svelte';

  import { errorMessage } from '../api/client';
  import { deleteNote, listNotes, updateNote } from '../api/notes';
  import type { Note, SessionInfo } from '../api/types';
  import type { MemoListView } from '../memo-view';
  import { provideMemoDrafts } from '../memo-drafts';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import NoteTimeline from '../components/NoteTimeline.svelte';
  import ListContinuation from '../components/ListContinuation.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { Button } from '../components/ui/button';
  import * as Empty from '../components/ui/empty';
  import { Spinner } from '../components/ui/spinner';
  import {
    captureListFocus,
    captureListScroll,
    focusAppendedRow,
    restoreListFocus,
    type ListFocusSnapshot,
  } from '../utils/focus';

  import {
    fetchLoadedPages,
    mergePageItems,
    type ListContinuationError,
    type ListLoad,
  } from '../utils/pagination';

  let { session, listView }: { session: SessionInfo; listView?: MemoListView } = $props();

  let notes = $state<Note[]>([]);
  const drafts = provideMemoDrafts(() => session);
  let query = $state(untrack(() => listView?.query ?? ''));
  let page = $state(0);
  let total = $state(0);
  let loading = $state(true);
  let loadingMore = $state(false);
  let loadError = $state<string | null>(null);
  let continuationError = $state<ListContinuationError | null>(null);
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
  let disposed = false;

  onMount(() => {
    void restoreMemoList(listView?.pages ?? 1, listView?.scrollTop ?? 0);
    return () => {
      disposed = true;
      invalidateArchiveRequest();
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  async function restoreMemoList(pages: number, scrollTop: number): Promise<void> {
    if (!(await loadArchive('reset'))) return;
    if (pages > page && !(await loadArchive('refresh', pages))) return;
    const id = requestId;
    await tick();
    if (id !== requestId) return;
    const scroller = pageElement?.closest<HTMLElement>('.workspace-column');
    if (scroller) scroller.scrollTop = scrollTop;
  }

  async function loadArchive(load: ListLoad, lastPage = Math.max(1, page)): Promise<boolean> {
    if (disposed) return false;
    if (load === 'append' && (loading || loadingMore || page === 0)) return false;
    const continuation =
      load === 'append' || load === 'refresh'
        ? pageElement?.querySelector('[data-list-continuation]')
        : null;
    const id = ++requestId;
    controller?.abort();
    const { signal } = (controller = new AbortController());
    loading = load !== 'append';
    loadingMore = load === 'append';
    loadError = null;
    if (load === 'reset') continuationError = null;

    const fetchPage = (nextPage: number) =>
      listNotes({ page: nextPage, pageSize: 30, q: query, status: 'ARCHIVED' }, signal);
    try {
      const response =
        load === 'refresh'
          ? await fetchLoadedPages(lastPage, fetchPage, signal)
          : await fetchPage(load === 'reset' ? 1 : page + 1);
      if (id !== requestId || signal.aborted) return false;

      continuationError = null;
      const restoreScroll = load === 'refresh' ? captureListScroll(pageElement) : undefined;
      const nextFocusUid =
        continuation === document.activeElement
          ? response.items.find((item) => !notes.some((visible) => visible.uid === item.uid))?.uid
          : undefined;
      const result = load === 'append' ? mergePageItems(notes, response.items) : response.items;
      notes = result;
      page = response.page;
      total = response.total;
      if (listView) listView.pages = page;
      if (restoreScroll) {
        await tick();
        if (id === requestId && !signal.aborted) restoreScroll();
      }
      if (continuation && nextFocusUid && id === requestId && !signal.aborted) {
        await focusAppendedRow(pageElement, continuation, nextFocusUid);
      }
      return id === requestId && !signal.aborted;
    } catch (cause) {
      if (id === requestId && !signal.aborted) {
        const message = errorMessage(cause, 'Unable to load the archive.');
        if (load === 'reset' || page === 0) loadError = message;
        else continuationError = { load, message, lastPage };
      }
      return false;
    } finally {
      if (id === requestId) {
        loading = false;
        loadingMore = false;
      }
    }
  }

  function handleSearch(event: Event): void {
    query = (event.currentTarget as HTMLInputElement).value;
    if (listView) listView.query = query;
    resetPagination();
    if (searchTimer) clearTimeout(searchTimer);
    invalidateArchiveRequest();
    loading = true;
    searchTimer = setTimeout(() => void loadArchive('reset'), 250);
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
      void loadArchive('refresh');
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
      void loadArchive('refresh');
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
      void loadArchive('refresh');
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to save the memo.');
      void loadArchive('refresh');
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
      drafts.write(note.uid, null);
      notes = notes.filter((item) => item.uid !== note.uid);
      total = Math.max(0, total - 1);
      pendingDelete = null;
      deleteError = null;
      actionStatus = 'Memo deleted.';
      void loadArchive('refresh');
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete the memo.');
      void loadArchive('refresh');
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

  function resetPagination(): void {
    page = 0;
    continuationError = null;
    if (listView) listView.pages = 1;
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
      <h1>Archive</h1>
    </div>
    <SearchField
      label="Search archived memos"
      placeholder="Search archive"
      value={query}
      oninput={handleSearch}
    />
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
    <div aria-live="polite" class="loading-state flex items-center justify-center gap-2">
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
        <Button onclick={() => void loadArchive('reset')} variant="secondary">Try again</Button>
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
  {/if}
  <ListContinuation
    error={continuationError}
    hasMore={page > 0 && !loadError && notes.length < total}
    {loading}
    {loadingMore}
    label="Load older memos"
    onLoad={loadArchive}
  />
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
