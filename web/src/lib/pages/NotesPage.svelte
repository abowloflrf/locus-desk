<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import SearchField from '../components/SearchField.svelte';
  import { onMount, tick, untrack } from 'svelte';

  import { errorMessage } from '../api/client';
  import { createNote, deleteNote, listNotes, listTags, updateNote } from '../api/notes';
  import type { Note, SessionInfo, UpdateNoteRequest } from '../api/types';
  import type { MemoListView } from '../memo-view';
  import { provideMemoDrafts } from '../memo-drafts';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import NoteComposer from '../components/NoteComposer.svelte';
  import NoteTimeline from '../components/NoteTimeline.svelte';
  import ListContinuation from '../components/ListContinuation.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { Button } from '../components/ui/button';
  import * as Empty from '../components/ui/empty';
  import { Spinner } from '../components/ui/spinner';
  import * as ToggleGroup from '../components/ui/toggle-group';
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

  let {
    session,
    listView,
    onOpenArchive,
    focusSearchRequested = false,
    onSearchFocused,
  }: {
    session: SessionInfo;
    listView?: MemoListView;
    onOpenArchive?: () => void;
    focusSearchRequested?: boolean;
    onSearchFocused?: () => void;
  } = $props();

  let notes = $state<Note[]>([]);
  let tags = $state<string[]>([]);
  const drafts = provideMemoDrafts(() => session);
  let query = $state(untrack(() => listView?.query ?? ''));
  let selectedTag = $state(untrack(() => listView?.tag ?? ''));
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
  let searchInput = $state<HTMLInputElement | null>(null);
  let pageElement = $state<HTMLElement>();
  let activeController: AbortController | null = null;
  let tagController: AbortController | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let deleteFocusSnapshot: ListFocusSnapshot | null = null;
  let requestId = 0;
  let disposed = false;
  let tagRequestId = 0;

  $effect(() => {
    if (focusSearchRequested && searchInput) {
      searchInput.focus();
      onSearchFocused?.();
    }
  });

  onMount(() => {
    void restoreMemoList(listView?.pages ?? 1, listView?.scrollTop ?? 0);
    void refreshTags();

    return () => {
      disposed = true;
      invalidateListRequest();
      tagController?.abort();
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  async function restoreMemoList(pages: number, scrollTop: number): Promise<void> {
    if (!(await loadNotesPage('reset'))) return;
    if (pages > page && !(await loadNotesPage('refresh', pages))) return;
    const id = requestId;
    await tick();
    if (id !== requestId) return;
    const scroller = pageElement?.closest<HTMLElement>('.workspace-column');
    if (scroller) scroller.scrollTop = scrollTop;
  }

  async function loadNotesPage(load: ListLoad, lastPage = Math.max(1, page)): Promise<boolean> {
    if (disposed) return false;
    if (load === 'append' && (loading || loadingMore || page === 0)) return false;
    const continuation =
      load === 'append' || load === 'refresh'
        ? pageElement?.querySelector('[data-list-continuation]')
        : null;
    const id = ++requestId;
    activeController?.abort();
    const { signal } = (activeController = new AbortController());
    loading = load !== 'append';
    loadingMore = load === 'append';
    loadError = null;
    if (load === 'reset') continuationError = null;

    const fetchPage = (nextPage: number) =>
      listNotes(
        { page: nextPage, pageSize: 30, q: query, status: 'ACTIVE', tag: selectedTag },
        signal,
      );
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
      notes = sortNotes(result);
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
        const message = errorMessage(cause, 'Unable to load memos.');
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

  async function refreshTags(): Promise<void> {
    const id = ++tagRequestId;
    tagController?.abort();
    tagController = new AbortController();
    try {
      const response = await listTags(tagController.signal);
      if (id === tagRequestId) tags = response.items;
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      // Notes remain usable if tag metadata is temporarily unavailable.
    }
  }

  function handleSearch(event: Event): void {
    query = (event.currentTarget as HTMLInputElement).value;
    if (listView) listView.query = query;
    resetPagination();
    if (searchTimer) clearTimeout(searchTimer);
    invalidateListRequest();
    loading = true;
    searchTimer = setTimeout(() => {
      searchTimer = null;
      void loadNotesPage('reset');
    }, 250);
  }

  function selectTag(tag: string): void {
    selectedTag = selectedTag === tag ? '' : tag;
    handleTagFilterChange();
  }

  function handleTagFilterChange(): void {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = null;
    if (listView) listView.tag = selectedTag;
    resetPagination();
    void loadNotesPage('reset');
  }

  async function handleCreate(content: string): Promise<Note> {
    invalidateListRequest();
    operationError = null;
    try {
      const note = await createNote({ content });
      if (matchesCurrentFilter(note)) {
        notes = sortNotes([note, ...notes]);
        total += 1;
      }
      void refreshTags();
      void loadNotesPage('refresh');
      return note;
    } catch (cause) {
      void loadNotesPage('refresh');
      throw cause;
    }
  }

  async function handleSave(note: Note, content: string): Promise<void> {
    const focusSnapshot = captureListFocus(pageElement, note.uid);
    invalidateListRequest();
    markBusy(note.uid, true);
    operationError = null;
    actionStatus = null;
    try {
      const updated = await updateNote(note.uid, { content });
      if (matchesCurrentFilter(updated)) {
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
      void refreshTags();
      void loadNotesPage('refresh');
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to save the memo.');
      void loadNotesPage('refresh');
      throw cause;
    } finally {
      markBusy(note.uid, false);
    }
  }

  async function handlePin(note: Note): Promise<void> {
    await optimisticUpdate(note, { pinned: !note.pinned });
  }

  async function handleArchive(note: Note): Promise<void> {
    const focusSnapshot = captureListFocus(pageElement, note.uid);
    invalidateListRequest();
    actionStatus = null;
    const wasVisible = notes.some((item) => item.uid === note.uid);
    notes = notes.filter((item) => item.uid !== note.uid);
    if (wasVisible) total = Math.max(0, total - 1);
    markBusy(note.uid, true);
    operationError = null;
    await restoreListFocus(pageElement, focusSnapshot);

    try {
      await updateNote(note.uid, { status: 'ARCHIVED' });
      const returnedDuringRequest = notes.some((item) => item.uid === note.uid);
      notes = notes.filter((item) => item.uid !== note.uid);
      if (returnedDuringRequest) total = Math.max(0, total - 1);
      actionStatus = 'Memo archived.';
      void refreshTags();
      void loadNotesPage('refresh');
    } catch (cause) {
      if (matchesCurrentFilter(note) && !notes.some((item) => item.uid === note.uid)) {
        notes = sortNotes([note, ...notes]);
        total += 1;
      }
      operationError = errorMessage(cause, 'Unable to archive the memo.');
      void loadNotesPage('refresh');
    } finally {
      markBusy(note.uid, false);
    }
  }

  async function optimisticUpdate(note: Note, payload: UpdateNoteRequest): Promise<void> {
    invalidateListRequest();
    const optimistic = { ...note, ...payload } as Note;
    notes = sortNotes(notes.map((item) => (item.uid === note.uid ? optimistic : item)));
    markBusy(note.uid, true);
    operationError = null;

    try {
      const updated = await updateNote(note.uid, payload);
      notes = sortNotes(notes.map((item) => (item.uid === note.uid ? updated : item)));
      void loadNotesPage('refresh');
    } catch (cause) {
      notes = sortNotes(notes.map((item) => (item.uid === note.uid ? note : item)));
      operationError = errorMessage(cause, 'Unable to update the memo.');
      void loadNotesPage('refresh');
    } finally {
      markBusy(note.uid, false);
    }
  }

  async function confirmDelete(): Promise<void> {
    if (!pendingDelete) return;
    const note = pendingDelete;
    invalidateListRequest();
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
      void refreshTags();
      void loadNotesPage('refresh');
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete the memo.');
      void loadNotesPage('refresh');
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

  function matchesCurrentFilter(note: Note): boolean {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    const queryMatches =
      !normalizedQuery || note.content.toLocaleLowerCase().includes(normalizedQuery);
    const tagMatches = !selectedTag || note.tags.includes(selectedTag);
    return queryMatches && tagMatches;
  }

  function sortNotes(items: Note[]): Note[] {
    return [...items].sort((left, right) => {
      if (left.pinned !== right.pinned) return left.pinned ? -1 : 1;
      return right.createdAt.localeCompare(left.createdAt);
    });
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

  function invalidateListRequest(): void {
    requestId += 1;
    activeController?.abort();
    activeController = null;
    loading = false;
    loadingMore = false;
  }
</script>

<div bind:this={pageElement} class="page notes-page">
  <header class="page-header notes-header">
    <h1 class="max-md:sr-only">Memos</h1>
    <SearchField
      label="Search memos"
      placeholder="Search memos"
      value={query}
      oninput={handleSearch}
      bind:ref={searchInput}
      shortcut="⌘K"
    />
    <Button
      class="md:hidden"
      href="/archive"
      onclick={(event) => {
        if (
          !onOpenArchive ||
          event.button !== 0 ||
          event.metaKey ||
          event.ctrlKey ||
          event.shiftKey ||
          event.altKey
        )
          return;
        event.preventDefault();
        onOpenArchive();
      }}
      variant="ghost"
    >
      <Archive data-icon="inline-start" />
      Archive
    </Button>
  </header>

  <NoteComposer onCreate={handleCreate} />

  {#if tags.length > 0}
    <div class="tag-toolbar">
      <ToggleGroup.Root
        aria-label="Filter by tag"
        bind:value={selectedTag}
        class="tag-filter min-w-0"
        onValueChange={handleTagFilterChange}
        size="xs"
        spacing={1}
        type="single"
      >
        <ToggleGroup.Item value="">All</ToggleGroup.Item>
        {#each tags as tag (tag)}
          <ToggleGroup.Item class="min-w-0" value={tag} title={`#${tag}`}>
            <span class="truncate font-mono"><span class="mr-0.5">#</span>{tag}</span>
          </ToggleGroup.Item>
        {/each}
      </ToggleGroup.Root>
    </div>
  {/if}

  {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
  <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
    {actionStatus ?? ''}
  </div>
  {#if loading && notes.length > 0}<span aria-live="polite" class="sr-only">Updating memos…</span
    >{/if}

  {#if loading && notes.length === 0}
    <div aria-live="polite" class="loading-state flex items-center justify-center gap-2">
      <Spinner />
      Loading memos…
    </div>
  {:else if loadError}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>Memos are unavailable</Empty.Title>
        <Empty.Description>{loadError}</Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <Button onclick={() => void loadNotesPage('reset')} variant="secondary">Try again</Button>
      </Empty.Content>
    </Empty.Root>
  {:else if notes.length === 0}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title
          >{query || selectedTag ? 'No memos found' : 'Your timeline is clear'}</Empty.Title
        >
        <Empty.Description
          >{query || selectedTag
            ? 'Try another keyword or tag.'
            : 'Write the first thought you want to keep.'}</Empty.Description
        >
      </Empty.Header>
    </Empty.Root>
  {:else}
    <NoteTimeline
      {busyUids}
      {notes}
      onArchive={handleArchive}
      onDelete={requestDelete}
      onPin={handlePin}
      onSave={handleSave}
      onTag={selectTag}
      timeZone={session.workspace.timezone}
    />
  {/if}
  <ListContinuation
    error={continuationError}
    hasMore={page > 0 && !loadError && notes.length < total}
    {loading}
    {loadingMore}
    label="Load older memos"
    onLoad={loadNotesPage}
  />
</div>

<ConfirmDialog
  busy={deleteBusy}
  confirmLabel="Delete memo"
  error={deleteError}
  message="This memo will be permanently deleted. This action cannot be undone."
  onCancel={cancelDelete}
  onConfirm={confirmDelete}
  onFallbackFocus={focusAfterDelete}
  open={Boolean(pendingDelete)}
  title="Delete this memo?"
/>

<style>
  .tag-toolbar {
    margin-bottom: 4px;
    min-width: 0;
  }

  .tag-toolbar :global(.tag-filter) {
    width: 100%;
    flex-wrap: wrap;
  }

  .tag-toolbar :global([data-slot='toggle-group-item']) {
    font-family: var(--font-mono);
    flex-shrink: 0;
    max-width: min(160px, 100%);
  }

  @media (max-width: 767px) {
    .notes-page {
      padding-top: 12px;
    }

    .notes-header {
      margin-bottom: 8px;
    }

    .notes-page :global(input[type='search']),
    .notes-page :global(#note-composer-input) {
      font-size: 14px;
      line-height: 22px;
    }

    .tag-toolbar {
      margin-bottom: 8px;
    }

    .tag-toolbar :global(.tag-filter) {
      row-gap: 16px;
    }

    .tag-toolbar :global([data-slot='toggle-group-item']) {
      position: relative;
      height: 28px;
      min-height: 28px;
      font-size: 12px;
    }

    .tag-toolbar :global([data-slot='toggle-group-item'])::before {
      content: '';
      position: absolute;
      inset: -8px 0;
    }
  }
</style>
