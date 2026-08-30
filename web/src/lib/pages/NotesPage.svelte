<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import { onMount } from 'svelte';

  import { errorMessage } from '../api/client';
  import { createNote, deleteNote, listNotes, listTags, updateNote } from '../api/notes';
  import type { Note, SessionInfo, UpdateNoteRequest } from '../api/types';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import NoteComposer from '../components/NoteComposer.svelte';
  import NoteTimeline from '../components/NoteTimeline.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { Button } from '../components/ui/button';
  import * as Empty from '../components/ui/empty';
  import { Input } from '../components/ui/input';
  import * as Kbd from '../components/ui/kbd';
  import { Spinner } from '../components/ui/spinner';
  import * as ToggleGroup from '../components/ui/toggle-group';
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
  let searchInput = $state<HTMLInputElement | null>(null);
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
      notes = sortNotes(reset ? response.items : [...notes, ...response.items]);
      page = response.page;
      total = response.total;
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === requestId) loadError = errorMessage(cause, 'Unable to load memos.');
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

  function handleTagFilterChange(): void {
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
          actionStatus = 'Memo updated and removed from the current view.';
          await restoreListFocus(pageElement, focusSnapshot);
        }
      }
      void refreshTags();
      void loadNotesPage(true);
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to save the memo.');
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
      actionStatus = 'Memo archived.';
      void refreshTags();
      void loadNotesPage(true);
    } catch (cause) {
      if (matchesCurrentFilter(note) && !notes.some((item) => item.uid === note.uid)) {
        notes = sortNotes([note, ...notes]);
        total += 1;
      }
      operationError = errorMessage(cause, 'Unable to archive the memo.');
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
      operationError = errorMessage(cause, 'Unable to update the memo.');
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
      actionStatus = 'Memo deleted.';
      void refreshTags();
      void loadNotesPage(true);
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete the memo.');
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
    <h1 class="sr-only">Memos</h1>
    <label class="search-field">
      <Search class="pointer-events-none absolute left-3 size-4 text-muted-foreground" />
      <span class="sr-only">Search memos</span>
      <Input
        class="pl-9 pr-12"
        autocomplete="off"
        bind:ref={searchInput}
        oninput={handleSearch}
        placeholder="Search memos"
        type="search"
        value={query}
        variant="flat"
      />
      <Kbd.Root class="pointer-events-none absolute right-3">⌘K</Kbd.Root>
    </label>
  </header>

  <NoteComposer onCreate={handleCreate} />

  {#if tags.length > 0}
    <ToggleGroup.Root
      aria-label="Filter by tag"
      bind:value={selectedTag}
      class="tag-filter mb-[17px] max-w-full flex-wrap pb-1"
      onValueChange={handleTagFilterChange}
      size="xs"
      spacing={1}
      type="single"
      variant="outline"
    >
      <ToggleGroup.Item
        class="data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:font-semibold data-[state=on]:text-primary-foreground data-[state=on]:shadow-none"
        value=""
      >
        <span class="font-mono text-[11px] leading-none">All</span>
      </ToggleGroup.Item>
      {#each tags as tag}
        <ToggleGroup.Item
          class="data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:font-semibold data-[state=on]:text-primary-foreground data-[state=on]:shadow-none"
          value={tag}
        >
          <span class="font-mono text-[11px] leading-none">#{tag}</span>
        </ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
  {/if}

  {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
  <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
    {actionStatus ?? ''}
  </div>
  {#if loading && notes.length > 0}<span aria-live="polite" class="sr-only">Updating memos…</span
    >{/if}

  {#if loading && notes.length === 0}
    <div aria-live="polite" class="loading-state large flex items-center justify-center gap-2">
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
        <Button onclick={() => void loadNotesPage(true)} variant="secondary">Try again</Button>
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
    {#if notes.length < total}
      <Button
        class="load-more"
        disabled={loadingMore}
        onclick={() => void loadNotesPage(false)}
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
  message="This memo will be permanently deleted. This action cannot be undone."
  onCancel={cancelDelete}
  onConfirm={confirmDelete}
  onFallbackFocus={focusAfterDelete}
  open={Boolean(pendingDelete)}
  title="Delete this memo?"
/>

<style>
  .notes-page {
    width: min(100%, 920px);
  }

  .notes-header {
    align-items: center;
    justify-content: flex-end;
    margin-bottom: 28px;
  }

  .notes-header :global(.search-field) {
    min-height: 42px;
    background: color-mix(in oklch, var(--card), transparent 10%);
    border-color: transparent;
    border-radius: var(--radius-md);
  }

  .notes-header :global(.search-field:focus-within) {
    border-color: transparent;
  }

  @media (max-width: 767px) {
    .notes-header {
      grid-template-columns: minmax(0, 1fr);
      align-items: start;
      margin-bottom: 20px;
    }

    .notes-header :global(.search-field) {
      width: 100%;
      max-width: none;
    }

    :global(.tag-filter [data-slot='toggle-group-item']) {
      position: relative;
      isolation: isolate;
      height: 44px;
      min-width: 44px;
      padding-inline: 10px;
      background: transparent;
      border-color: transparent;
      box-shadow: none;
    }

    :global(.tag-filter [data-slot='toggle-group-item']::before) {
      position: absolute;
      inset: 10px 0;
      z-index: 0;
      pointer-events: none;
      background: var(--background);
      border: 1px solid var(--input);
      border-radius: var(--radius-md);
      box-shadow: 0 1px 2px color-mix(in oklch, var(--foreground), transparent 94%);
      content: '';
    }

    :global(.tag-filter [data-slot='toggle-group-item'] > span) {
      position: relative;
      z-index: 1;
    }

    :global(.tag-filter [data-slot='toggle-group-item'][data-state='on']::before) {
      background: var(--primary);
      border-color: var(--primary);
    }
  }
</style>
