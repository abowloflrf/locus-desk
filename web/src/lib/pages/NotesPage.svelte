<script lang="ts">
  import { onMount } from 'svelte';

  import { errorMessage } from '../api/client';
  import { createNote, deleteNote, listNotes, listTags, updateNote } from '../api/notes';
  import type { Note, SessionInfo, UpdateNoteRequest } from '../api/types';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import Icon from '../components/Icon.svelte';
  import NoteComposer from '../components/NoteComposer.svelte';
  import NoteTimeline from '../components/NoteTimeline.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { captureListFocus, restoreListFocus, type ListFocusSnapshot } from '../utils/focus';

  let { session }: { session: SessionInfo } = $props();

  let notes = $state<Note[]>([]);
  let tags = $state<string[]>([]);
  let query = $state('');
  let selectedTag = $state('');
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
  let searchInput = $state<HTMLInputElement>();
  let pageElement = $state<HTMLElement>();
  let activeController: AbortController | null = null;
  let tagController: AbortController | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let deleteFocusSnapshot: ListFocusSnapshot | null = null;
  let requestId = 0;
  let tagRequestId = 0;

  onMount(() => {
    void loadNotesPage(true);
    void refreshTags();

    const focusSearch = () => searchInput?.focus();
    window.addEventListener('locus:focus-search', focusSearch);
    return () => {
      activeController?.abort();
      tagController?.abort();
      if (searchTimer) clearTimeout(searchTimer);
      window.removeEventListener('locus:focus-search', focusSearch);
    };
  });

  async function loadNotesPage(reset: boolean): Promise<void> {
    const nextPage = reset ? 1 : page + 1;
    const id = ++requestId;
    activeController?.abort();
    activeController = new AbortController();
    if (reset) loading = true;
    else loadingMore = true;
    loadError = null;

    try {
      const response = await listNotes(
        {
          page: nextPage,
          pageSize: 30,
          q: query,
          status: 'ACTIVE',
          tag: selectedTag,
        },
        activeController.signal,
      );
      if (id !== requestId) return;
      notes = reset ? response.items : [...notes, ...response.items];
      page = response.page;
      total = response.total;
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === requestId) loadError = errorMessage(cause, 'Unable to load notes.');
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
    if (searchTimer) clearTimeout(searchTimer);
    invalidateListRequest();
    loading = true;
    searchTimer = setTimeout(() => {
      searchTimer = null;
      void loadNotesPage(true);
    }, 250);
  }

  function selectTag(tag: string): void {
    selectedTag = selectedTag === tag ? '' : tag;
    void loadNotesPage(true);
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
      void loadNotesPage(true);
      return note;
    } catch (cause) {
      void loadNotesPage(true);
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
          actionStatus = 'Note updated and removed from the current view.';
          await restoreListFocus(pageElement, focusSnapshot);
        }
      }
      void refreshTags();
      void loadNotesPage(true);
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to save the note.');
      void loadNotesPage(true);
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
      actionStatus = 'Note archived.';
      void refreshTags();
      void loadNotesPage(true);
    } catch (cause) {
      if (matchesCurrentFilter(note) && !notes.some((item) => item.uid === note.uid)) {
        notes = sortNotes([note, ...notes]);
        total += 1;
      }
      operationError = errorMessage(cause, 'Unable to archive the note.');
      void loadNotesPage(true);
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
      void loadNotesPage(true);
    } catch (cause) {
      notes = sortNotes(notes.map((item) => (item.uid === note.uid ? note : item)));
      operationError = errorMessage(cause, 'Unable to update the note.');
      void loadNotesPage(true);
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
      notes = notes.filter((item) => item.uid !== note.uid);
      total = Math.max(0, total - 1);
      pendingDelete = null;
      deleteError = null;
      actionStatus = 'Note deleted.';
      void refreshTags();
      void loadNotesPage(true);
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete the note.');
      void loadNotesPage(true);
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
    <div>
      <p class="eyebrow">{session.workspace.name}</p>
      <h1>Notes</h1>
    </div>
    <label class="search-field">
      <Icon name="search" size={17} />
      <span class="sr-only">Search notes</span>
      <input
        autocomplete="off"
        bind:this={searchInput}
        oninput={handleSearch}
        placeholder="Search notes"
        type="search"
        value={query}
      />
      <kbd>⌘K</kbd>
    </label>
  </header>

  <NoteComposer onCreate={handleCreate} />

  {#if tags.length > 0}
    <div aria-label="Filter by tag" class="tag-filter" role="group">
      <button
        aria-pressed={!selectedTag}
        class:active={!selectedTag}
        class="tag-chip"
        onclick={() => selectTag('')}
        type="button">All</button
      >
      {#each tags as tag}
        <button
          aria-pressed={selectedTag === tag}
          class:active={selectedTag === tag}
          class="tag-chip"
          onclick={() => selectTag(tag)}
          type="button">#{tag}</button
        >
      {/each}
    </div>
  {/if}

  {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
  <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
    {actionStatus ?? ''}
  </div>

  <div class="list-toolbar">
    <span
      >{query || selectedTag
        ? `${total} matching ${total === 1 ? 'note' : 'notes'}`
        : `${total} notes`}</span
    >
    {#if loading && notes.length > 0}<span aria-live="polite">Updating…</span>{/if}
  </div>

  {#if loading && notes.length === 0}
    <div aria-live="polite" class="loading-state large">Loading notes…</div>
  {:else if loadError}
    <div class="empty-state">
      <h2>Notes are unavailable</h2>
      <p>{loadError}</p>
      <button class="button secondary" onclick={() => void loadNotesPage(true)} type="button"
        >Try again</button
      >
    </div>
  {:else if notes.length === 0}
    <div class="empty-state">
      <h2>{query || selectedTag ? 'No notes found' : 'Your timeline is clear'}</h2>
      <p>
        {query || selectedTag
          ? 'Try another keyword or tag.'
          : 'Write the first thought you want to keep.'}
      </p>
    </div>
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
    {#if notes.length < total}
      <button
        class="button secondary load-more"
        disabled={loadingMore}
        onclick={() => void loadNotesPage(false)}
        type="button">{loadingMore ? 'Loading…' : 'Load older notes'}</button
      >
    {/if}
  {/if}
</div>

<ConfirmDialog
  busy={deleteBusy}
  confirmLabel="Delete note"
  error={deleteError}
  message="This note will be permanently deleted. This action cannot be undone."
  onCancel={cancelDelete}
  onConfirm={confirmDelete}
  onFallbackFocus={focusAfterDelete}
  open={Boolean(pendingDelete)}
  title="Delete this note?"
/>

<style>
  .notes-page {
    width: min(100%, 860px);
  }

  .tag-filter {
    display: flex;
    gap: 6px;
    padding-bottom: 4px;
    margin-bottom: 17px;
    overflow-x: auto;
  }

  .tag-filter .tag-chip.active {
    border-color: color-mix(in oklch, var(--color-accent), transparent 65%);
  }
</style>
