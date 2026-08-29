<script lang="ts">
  import { onMount, tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import {
    createLibraryItem,
    deleteLibraryItem,
    getLibraryItem,
    listLibraryItems,
    retryLibraryItem,
    updateLibraryItem,
  } from '../api/library';
  import type {
    CreateLibraryItemRequest,
    LibraryItem,
    LibraryItemStatus,
    SessionInfo,
    UpdateLibraryItemRequest,
  } from '../api/types';
  import Icon from '../components/Icon.svelte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import LibraryCaptureForm from '../components/LibraryCaptureForm.svelte';
  import LibraryReader from '../components/LibraryReader.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { safeLibrarySourceUrl } from '../library-content';
  import { captureListFocus, restoreListFocus } from '../utils/focus';
  import { formatNoteTimestamp } from '../utils/date';

  let { session }: { session: SessionInfo } = $props();

  let items = $state<LibraryItem[]>([]);
  let query = $state('');
  let status = $state<LibraryItemStatus>('ACTIVE');
  let page = $state(1);
  let total = $state(0);
  let loading = $state(true);
  let loadingMore = $state(false);
  let loadError = $state<string | null>(null);
  let operationError = $state<string | null>(null);
  let actionStatus = $state<string | null>(null);
  let busyUids = $state<Set<string>>(new Set());
  let selectedUid = $state<string | null>(null);
  let selectedItem = $state<LibraryItem | null>(null);
  let readerItem = $state<LibraryItem | null>(null);
  let detailLoading = $state(false);
  let detailError = $state<string | null>(null);
  let detailPollingError = $state<string | null>(null);
  let detailProcessingError = $state<string | null>(null);
  let retryingUid = $state<string | null>(null);
  let pendingDelete = $state<LibraryItem | null>(null);
  let deleteBusy = $state(false);
  let deleteError = $state<string | null>(null);
  let deleteFocusSnapshot = $state<ReturnType<typeof captureListFocus> | null>(null);
  let compactDetail = $state(false);
  let pageElement = $state<HTMLElement>();
  let detailPanel = $state<HTMLElement>();
  let detailHeading = $state<HTMLElement>();
  let searchInput = $state<HTMLInputElement>();
  let listController: AbortController | null = null;
  let detailController: AbortController | null = null;
  let pollController: AbortController | null = null;
  let retryController: AbortController | null = null;
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let requestId = 0;
  let detailRequestId = 0;
  let retryRequestId = 0;
  let pollGeneration = 0;

  const LIBRARY_POLL_INTERVAL_MS = 2_000;

  onMount(() => {
    const media = window.matchMedia('(max-width: 1199px)');
    const updateMedia = () => {
      compactDetail = media.matches;
    };
    updateMedia();
    media.addEventListener('change', updateMedia);
    void loadItems(true);

    return () => {
      media.removeEventListener('change', updateMedia);
      listController?.abort();
      detailController?.abort();
      cancelRetry();
      stopPolling();
      if (searchTimer) clearTimeout(searchTimer);
    };
  });

  async function loadItems(reset: boolean): Promise<void> {
    const nextPage = reset ? 1 : page + 1;
    const id = ++requestId;
    listController?.abort();
    listController = new AbortController();
    if (reset) loading = true;
    else loadingMore = true;
    loadError = null;

    try {
      const response = await listLibraryItems(
        { page: nextPage, pageSize: 30, q: query, status },
        listController.signal,
      );
      if (id !== requestId) return;
      items = reset ? response.items : [...items, ...response.items];
      page = response.page;
      total = response.total;

      if (selectedUid) {
        const visible = items.find((item) => item.uid === selectedUid);
        if (!visible) closeDetail(false);
        else if (!detailLoading) {
          selectedItem = {
            ...visible,
            captures: selectedItem?.uid === visible.uid ? selectedItem.captures : visible.captures,
          };
          startPolling(visible);
        }
      }
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === requestId) loadError = errorMessage(cause, 'Unable to load the Library.');
    } finally {
      if (id === requestId) {
        loading = false;
        loadingMore = false;
      }
    }
  }

  async function handleCreate(payload: CreateLibraryItemRequest): Promise<LibraryItem> {
    invalidateListRequest();
    operationError = null;
    actionStatus = null;
    try {
      const item = await createLibraryItem(payload);
      const alreadyVisible = items.some((entry) => entry.uid === item.uid);
      if (status === 'ACTIVE' && matchesQuery(item)) {
        items = [item, ...items.filter((entry) => entry.uid !== item.uid)];
        if (!alreadyVisible) total += 1;
      }
      selectedUid = item.uid;
      selectedItem = item;
      detailError = null;
      detailPollingError = null;
      detailProcessingError = null;
      startPolling(item);
      actionStatus = 'Link saved to Library.';
      void loadItems(true);
      return item;
    } catch (cause) {
      void loadItems(true);
      throw cause;
    }
  }

  function handleSearch(event: Event): void {
    query = (event.currentTarget as HTMLInputElement).value;
    if (searchTimer) clearTimeout(searchTimer);
    clearVisibleResults();
    searchTimer = setTimeout(() => {
      searchTimer = null;
      void loadItems(true);
    }, 250);
  }

  function selectStatus(next: LibraryItemStatus): void {
    if (status === next) return;
    status = next;
    clearVisibleResults();
    void loadItems(true);
  }

  function clearVisibleResults(): void {
    invalidateListRequest();
    items = [];
    page = 1;
    total = 0;
    loading = true;
    loadError = null;
  }

  async function openDetail(item: LibraryItem): Promise<void> {
    if (retryingUid && retryingUid !== item.uid) cancelRetry();
    const id = ++detailRequestId;
    detailController?.abort();
    detailController = new AbortController();
    selectedUid = item.uid;
    selectedItem = item;
    detailLoading = true;
    detailError = null;
    detailPollingError = null;
    detailProcessingError = null;
    stopPolling();

    if (compactDetail) {
      await tick();
      requestAnimationFrame(() => detailHeading?.focus());
    }

    try {
      const detail = await getLibraryItem(item.uid, detailController.signal);
      if (id === detailRequestId && selectedUid === item.uid) {
        selectedItem = detail;
        startPolling(detail);
      }
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === detailRequestId && selectedUid === item.uid) {
        detailError = errorMessage(cause, 'Unable to load item details.');
        startPolling(selectedItem);
      }
    } finally {
      if (id === detailRequestId) detailLoading = false;
    }
  }

  async function closeDetail(restoreFocus = true): Promise<void> {
    const uid = selectedUid;
    detailRequestId += 1;
    detailController?.abort();
    detailController = null;
    cancelRetry();
    stopPolling();
    selectedUid = null;
    selectedItem = null;
    detailLoading = false;
    detailError = null;
    detailPollingError = null;
    detailProcessingError = null;

    if (!restoreFocus || !uid) return;
    await tick();
    [...(pageElement?.querySelectorAll<HTMLButtonElement>('[data-library-select]') ?? [])]
      .find((button) => button.dataset.librarySelect === uid)
      ?.focus();
  }

  async function toggleRead(item: LibraryItem): Promise<void> {
    const nextRead = !item.readAt;
    await optimisticUpdate(
      item,
      { read: nextRead },
      { ...item, readAt: nextRead ? new Date().toISOString() : null },
      nextRead ? 'Marked as read.' : 'Marked as unread.',
    );
  }

  async function retryCapture(item: LibraryItem): Promise<void> {
    if (retryingUid === item.uid) return;
    cancelRetry();
    const id = ++retryRequestId;
    const controller = new AbortController();
    retryController = controller;
    retryingUid = item.uid;
    detailProcessingError = null;
    operationError = null;
    actionStatus = null;

    try {
      const updated = await retryLibraryItem(item.uid, controller.signal);
      if (id !== retryRequestId || selectedUid !== item.uid) return;
      replaceItem(updated);
      startPolling(updated);
      actionStatus = 'Content capture queued.';
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === retryRequestId && selectedUid === item.uid) {
        detailProcessingError = errorMessage(cause, 'Unable to retry content capture.');
      }
    } finally {
      if (id === retryRequestId) {
        retryController = null;
        retryingUid = null;
      }
    }
  }

  function cancelRetry(): void {
    retryRequestId += 1;
    retryController?.abort();
    retryController = null;
    retryingUid = null;
  }

  function startPolling(item: LibraryItem | null): void {
    stopPolling();
    if (!item || item.processingStatus !== 'PENDING' || selectedUid !== item.uid) return;
    const generation = pollGeneration;
    schedulePoll(item.uid, generation);
  }

  function schedulePoll(uid: string, generation: number): void {
    if (generation !== pollGeneration || selectedUid !== uid) return;
    pollTimer = setTimeout(() => {
      pollTimer = null;
      void pollItem(uid, generation);
    }, LIBRARY_POLL_INTERVAL_MS);
  }

  async function pollItem(uid: string, generation: number): Promise<void> {
    if (generation !== pollGeneration || selectedUid !== uid) return;
    const controller = new AbortController();
    pollController = controller;

    try {
      const updated = await getLibraryItem(uid, controller.signal);
      if (generation !== pollGeneration || selectedUid !== uid) return;
      replaceItem(updated);
      detailPollingError = null;
      if (updated.processingStatus === 'PENDING') schedulePoll(uid, generation);
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (generation !== pollGeneration || selectedUid !== uid) return;
      detailPollingError = errorMessage(cause, 'Unable to refresh content capture status.');
      schedulePoll(uid, generation);
    } finally {
      if (pollController === controller) pollController = null;
    }
  }

  function stopPolling(): void {
    pollGeneration += 1;
    if (pollTimer) clearTimeout(pollTimer);
    pollTimer = null;
    pollController?.abort();
    pollController = null;
  }

  function openReader(item: LibraryItem): void {
    if (item.processingStatus !== 'READY' || !item.contentAvailable) return;
    detailRequestId += 1;
    detailController?.abort();
    detailController = null;
    detailLoading = false;
    stopPolling();
    operationError = null;
    actionStatus = null;
    readerItem = item;
  }

  async function closeReader(): Promise<void> {
    const uid = readerItem?.uid;
    readerItem = null;
    if (selectedItem) startPolling(selectedItem);
    await tick();
    if (!uid) return;
    const readButton = [
      ...(pageElement?.querySelectorAll<HTMLButtonElement>('[data-library-read]') ?? []),
    ].find((button) => button.dataset.libraryRead === uid);
    if (readButton) readButton.focus();
    else detailHeading?.focus();
  }

  async function toggleStar(item: LibraryItem): Promise<void> {
    const starred = !item.starred;
    await optimisticUpdate(
      item,
      { starred },
      { ...item, starred },
      starred ? 'Added to starred items.' : 'Removed from starred items.',
    );
  }

  async function toggleArchive(item: LibraryItem): Promise<void> {
    const nextStatus: LibraryItemStatus = item.status === 'ACTIVE' ? 'ARCHIVED' : 'ACTIVE';
    const index = items.findIndex((entry) => entry.uid === item.uid);
    const focusSnapshot = captureListFocus(pageElement, item.uid);
    invalidateListRequest();
    items = items.filter((entry) => entry.uid !== item.uid);
    total = Math.max(0, total - 1);
    if (selectedUid === item.uid) await closeDetail(false);
    markBusy(item.uid, true);
    operationError = null;
    actionStatus = null;
    await restoreListFocus(pageElement, focusSnapshot);

    try {
      await updateLibraryItem(item.uid, { status: nextStatus });
      actionStatus =
        nextStatus === 'ARCHIVED' ? 'Library item archived.' : 'Library item restored.';
      void loadItems(true);
    } catch (cause) {
      const restored = [...items];
      restored.splice(Math.max(0, index), 0, item);
      items = restored;
      total += 1;
      operationError = errorMessage(
        cause,
        nextStatus === 'ARCHIVED'
          ? 'Unable to archive this Library item.'
          : 'Unable to restore this Library item.',
      );
      await restoreListFocus(pageElement, focusSnapshot, item.uid);
    } finally {
      markBusy(item.uid, false);
    }
  }

  function requestDelete(item: LibraryItem): void {
    deleteFocusSnapshot = captureListFocus(pageElement, item.uid);
    deleteError = null;
    actionStatus = null;
    pendingDelete = item;
  }

  function cancelDelete(): void {
    deleteFocusSnapshot = null;
    deleteError = null;
    pendingDelete = null;
  }

  async function confirmDelete(): Promise<void> {
    if (!pendingDelete) return;
    const item = pendingDelete;
    invalidateListRequest();
    deleteBusy = true;
    deleteError = null;
    try {
      await deleteLibraryItem(item.uid);
      items = items.filter((entry) => entry.uid !== item.uid);
      total = Math.max(0, total - 1);
      if (selectedUid === item.uid) await closeDetail(false);
      pendingDelete = null;
      actionStatus = 'Library item deleted.';
      void loadItems(true);
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete this Library item.');
    } finally {
      deleteBusy = false;
    }
  }

  async function focusAfterDelete(): Promise<void> {
    const snapshot = deleteFocusSnapshot;
    deleteFocusSnapshot = null;
    if (snapshot) await restoreListFocus(pageElement, snapshot);
    else document.getElementById('main-content')?.focus();
  }

  async function optimisticUpdate(
    item: LibraryItem,
    payload: UpdateLibraryItemRequest,
    optimistic: LibraryItem,
    successMessage: string,
  ): Promise<void> {
    replaceItem(optimistic);
    markBusy(item.uid, true);
    operationError = null;
    actionStatus = null;
    try {
      const updated = await updateLibraryItem(item.uid, payload);
      replaceItem(updated);
      actionStatus = successMessage;
    } catch (cause) {
      replaceItem(item);
      operationError = errorMessage(cause, 'Unable to update this Library item.');
    } finally {
      markBusy(item.uid, false);
    }
  }

  function replaceItem(item: LibraryItem): void {
    items = items.map((entry) => (entry.uid === item.uid ? item : entry));
    if (selectedUid === item.uid) selectedItem = item;
    if (readerItem?.uid === item.uid) readerItem = item;
  }

  function matchesQuery(item: LibraryItem): boolean {
    const normalized = query.trim().toLocaleLowerCase();
    return (
      !normalized ||
      [item.title, item.siteName, item.originalUrl, ...item.tags]
        .filter((value): value is string => Boolean(value))
        .some((value) => value.toLocaleLowerCase().includes(normalized))
    );
  }

  function markBusy(uid: string, busy: boolean): void {
    const next = new Set(busyUids);
    if (busy) next.add(uid);
    else next.delete(uid);
    busyUids = next;
  }

  function invalidateListRequest(): void {
    requestId += 1;
    listController?.abort();
    listController = null;
    loading = false;
    loadingMore = false;
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (readerItem) return;
    if (!selectedUid || event.defaultPrevented) return;
    if (document.querySelector('dialog[open]')) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      void closeDetail();
      return;
    }
    if (!compactDetail || event.key !== 'Tab' || !detailPanel) return;

    const focusable = [
      ...detailPanel.querySelectorAll<HTMLElement>('button:not([disabled]), a[href], [tabindex]'),
    ].filter((element) => element.tabIndex !== -1);
    const first = focusable.at(0);
    const last = focusable.at(-1);
    if (!first || !last) return;

    if (!detailPanel.contains(document.activeElement)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function displayTitle(item: LibraryItem): string {
    return item.title?.trim() || hostname(item) || 'Untitled link';
  }

  function hostname(item: LibraryItem): string {
    try {
      return new URL(item.canonicalUrl ?? item.normalizedUrl ?? item.originalUrl).hostname.replace(
        /^www\./,
        '',
      );
    } catch {
      return '';
    }
  }

  function safeLink(item: LibraryItem): string | null {
    return safeLibrarySourceUrl(item.canonicalUrl ?? item.normalizedUrl ?? item.originalUrl);
  }

  function processingLabel(item: LibraryItem): string {
    if (item.processingStatus === 'PENDING') return 'Processing';
    if (item.processingStatus === 'READY') return 'Ready';
    if (item.processingStatus === 'FAILED') return 'Fetch failed';
    return 'Saved';
  }

  function processingClass(item: LibraryItem): string {
    if (item.processingStatus === 'FAILED') return 'failed';
    if (item.processingStatus === 'READY') return 'ready';
    if (item.processingStatus === 'PENDING') return 'pending';
    return 'saved';
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if readerItem}
  <LibraryReader
    {actionStatus}
    busy={busyUids.has(readerItem.uid)}
    item={readerItem}
    onBack={closeReader}
    onToggleRead={toggleRead}
    {operationError}
    timeZone={session.workspace.timezone}
  />
{:else}
  <div bind:this={pageElement} class:detail-open={Boolean(selectedUid)} class="library-page">
    <div class="library-primary" inert={compactDetail && Boolean(selectedUid)}>
      <LibraryCaptureForm onCreate={handleCreate} />

      <section aria-labelledby="library-items-title" class="library-index">
        <header class="library-toolbar">
          <div>
            <h2 id="library-items-title">Saved items</h2>
            <p>{total} {total === 1 ? 'item' : 'items'} in this view</p>
          </div>
          <label class="search-field library-search">
            <Icon name="search" size={17} />
            <span class="sr-only">Search Library</span>
            <input
              autocomplete="off"
              bind:this={searchInput}
              oninput={handleSearch}
              placeholder="Search Library"
              type="search"
              value={query}
            />
          </label>
        </header>

        <div aria-label="Filter Library by status" class="library-filters" role="group">
          <button
            aria-pressed={status === 'ACTIVE'}
            class:active={status === 'ACTIVE'}
            onclick={() => selectStatus('ACTIVE')}
            type="button">Active</button
          >
          <button
            aria-pressed={status === 'ARCHIVED'}
            class:active={status === 'ARCHIVED'}
            onclick={() => selectStatus('ARCHIVED')}
            type="button">Archived</button
          >
          {#if loading && items.length > 0}<span aria-live="polite">Updating…</span>{/if}
        </div>

        {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
        <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
          {actionStatus ?? ''}
        </div>

        <div aria-busy={loading || loadingMore} class="library-results">
          {#if loading && items.length === 0}
            <div aria-live="polite" class="loading-state large">Loading Library…</div>
          {:else if loadError}
            <div class="empty-state">
              <h3>Library unavailable</h3>
              <p>{loadError}</p>
              <button class="button secondary" onclick={() => void loadItems(true)} type="button"
                >Try again</button
              >
            </div>
          {:else if items.length === 0}
            <div class="empty-state">
              <h3>
                {query
                  ? 'No saved items found'
                  : status === 'ARCHIVED'
                    ? 'No archived items'
                    : 'Save your first link'}
              </h3>
              <p>
                {query
                  ? 'Try another keyword.'
                  : status === 'ARCHIVED'
                    ? 'Archived links will remain available here.'
                    : 'Add a URL above and keep the context that matters.'}
              </p>
              {#if query}
                <button
                  class="button secondary"
                  onclick={() => {
                    query = '';
                    if (searchInput) searchInput.value = '';
                    void loadItems(true);
                  }}
                  type="button">Clear search</button
                >
              {/if}
            </div>
          {:else}
            <ol aria-label="Library items" class="library-list">
              {#each items as item (item.uid)}
                <li
                  class:busy={busyUids.has(item.uid)}
                  class:selected={selectedUid === item.uid}
                  data-focus-uid={item.uid}
                >
                  <button
                    aria-current={selectedUid === item.uid ? 'true' : undefined}
                    class="item-select"
                    data-library-select={item.uid}
                    onclick={() => void openDetail(item)}
                    type="button"
                  >
                    <span class="item-title">{displayTitle(item)}</span>
                    <span class="item-source"
                      >{item.siteName || hostname(item) || item.originalUrl}</span
                    >
                    {#if item.excerpt}<span class="item-excerpt">{item.excerpt}</span>{/if}
                    <span class="item-meta">
                      <span class={`processing-state ${processingClass(item)}`}
                        >{processingLabel(item)}</span
                      >
                      <span>{item.readAt ? 'Read' : 'Unread'}</span>
                      <span>{formatNoteTimestamp(item.createdAt, session.workspace.timezone)}</span>
                    </span>
                    {#if item.tags.length > 0}
                      <span class="item-tags">
                        {#each item.tags.slice(0, 3) as tag}<span>#{tag}</span>{/each}
                      </span>
                    {/if}
                  </button>
                  <div class="item-actions">
                    <button
                      aria-label={`${item.starred ? 'Unstar' : 'Star'} ${displayTitle(item)}`}
                      aria-pressed={item.starred}
                      class:active={item.starred}
                      class="icon-button"
                      disabled={busyUids.has(item.uid)}
                      onclick={() => void toggleStar(item)}
                      title={item.starred ? 'Unstar' : 'Star'}
                      type="button"><Icon name="star" size={17} /></button
                    >
                    <button
                      aria-label={`${item.readAt ? 'Mark as unread' : 'Mark as read'}: ${displayTitle(item)}`}
                      class="text-action"
                      disabled={busyUids.has(item.uid)}
                      onclick={() => void toggleRead(item)}
                      type="button">{item.readAt ? 'Unread' : 'Read'}</button
                    >
                    <button
                      aria-label={`${item.status === 'ACTIVE' ? 'Archive' : 'Restore'} ${displayTitle(item)}`}
                      class="icon-button"
                      disabled={busyUids.has(item.uid)}
                      onclick={() => void toggleArchive(item)}
                      title={item.status === 'ACTIVE' ? 'Archive' : 'Restore'}
                      type="button"
                    >
                      <Icon name={item.status === 'ACTIVE' ? 'archive' : 'restore'} size={17} />
                    </button>
                    <button
                      aria-label={`Delete ${displayTitle(item)}`}
                      class="icon-button danger-quiet"
                      disabled={busyUids.has(item.uid)}
                      onclick={() => requestDelete(item)}
                      title="Delete"
                      type="button"><Icon name="delete" size={17} /></button
                    >
                  </div>
                </li>
              {/each}
            </ol>

            {#if items.length < total}
              <button
                class="button secondary load-more"
                disabled={loadingMore}
                onclick={() => void loadItems(false)}
                type="button">{loadingMore ? 'Loading…' : 'Load more'}</button
              >
            {/if}
          {/if}
        </div>
      </section>
    </div>

    {#if compactDetail && selectedUid}
      <button
        aria-label="Close Library item details"
        class="detail-backdrop"
        onclick={() => void closeDetail()}
        tabindex="-1"
        type="button"
      ></button>
    {/if}

    <aside
      aria-hidden={compactDetail ? !selectedUid : undefined}
      aria-label="Library item details"
      aria-modal={compactDetail && selectedUid ? 'true' : undefined}
      bind:this={detailPanel}
      class:open={Boolean(selectedUid)}
      class="library-detail"
      inert={compactDetail && !selectedUid}
      role={compactDetail ? 'dialog' : 'complementary'}
    >
      {#if selectedItem}
        <header class="detail-header">
          <div>
            <p class="eyebrow">
              {selectedItem.itemKind === 'ARTICLE' ? 'Saved article' : 'Saved link'}
            </p>
            <h2 bind:this={detailHeading} tabindex="-1">{displayTitle(selectedItem)}</h2>
          </div>
          <button
            aria-label="Close Library item details"
            class="icon-button"
            onclick={() => void closeDetail()}
            type="button"><Icon name="close" /></button
          >
        </header>

        {#if detailLoading}<p aria-live="polite" class="detail-loading">Updating details…</p>{/if}
        {#if detailError}<StatusMessage tone="error">{detailError}</StatusMessage>{/if}

        <div class="detail-actions" role="group" aria-label="Library item actions">
          {#if selectedItem.processingStatus === 'READY' && selectedItem.contentAvailable}
            <button
              class="button primary"
              data-library-read={selectedItem.uid}
              onclick={() => openReader(selectedItem!)}
              type="button"
            >
              <Icon name="reader" size={16} />
              Read
            </button>
          {/if}
          <button
            aria-pressed={selectedItem.starred}
            class:active={selectedItem.starred}
            class="button secondary"
            disabled={busyUids.has(selectedItem.uid)}
            onclick={() => void toggleStar(selectedItem!)}
            type="button"
          >
            <Icon name="star" size={16} />
            {selectedItem.starred ? 'Starred' : 'Star'}
          </button>
          <button
            class="button secondary"
            disabled={busyUids.has(selectedItem.uid)}
            onclick={() => void toggleRead(selectedItem!)}
            type="button"
          >
            <Icon name="reader" size={16} />
            {selectedItem.readAt ? 'Mark unread' : 'Mark read'}
          </button>
          <button
            class="button secondary"
            disabled={busyUids.has(selectedItem.uid)}
            onclick={() => void toggleArchive(selectedItem!)}
            type="button"
          >
            <Icon name={selectedItem.status === 'ACTIVE' ? 'archive' : 'restore'} size={16} />
            {selectedItem.status === 'ACTIVE' ? 'Archive' : 'Restore'}
          </button>
          <button
            class="button delete-action"
            disabled={busyUids.has(selectedItem.uid)}
            onclick={() => requestDelete(selectedItem!)}
            type="button"
          >
            <Icon name="delete" size={16} />
            Delete
          </button>
        </div>

        {#if selectedItem.processingStatus === 'PENDING'}
          <section aria-live="polite" class="content-state pending">
            <span aria-hidden="true" class="content-state-mark"></span>
            <div>
              <h3>Preparing article</h3>
              <p>The saved page is being made ready for local reading.</p>
            </div>
          </section>
        {:else if selectedItem.processingStatus === 'FAILED' || selectedItem.processingStatus === 'NOT_FETCHED'}
          <section class="content-state unavailable">
            <div>
              <h3>
                {selectedItem.processingStatus === 'FAILED'
                  ? 'Article capture failed'
                  : 'Article not prepared'}
              </h3>
              <p>
                {selectedItem.lastError ||
                  (selectedItem.processingStatus === 'FAILED'
                    ? 'The latest capture could not be completed.'
                    : 'Readable content has not been fetched yet.')}
              </p>
            </div>
            <button
              class="button secondary"
              disabled={retryingUid === selectedItem.uid}
              onclick={() => void retryCapture(selectedItem!)}
              type="button"
            >
              {retryingUid === selectedItem.uid ? 'Retrying…' : 'Retry'}
            </button>
          </section>
        {:else if !selectedItem.contentAvailable}
          <section class="content-state unavailable">
            <div>
              <h3>No readable text</h3>
              <p>The page was saved, but no article text was extracted.</p>
            </div>
          </section>
        {/if}

        {#if detailPollingError}<StatusMessage tone="error">{detailPollingError}</StatusMessage
          >{/if}
        {#if detailProcessingError}
          <StatusMessage tone="error">{detailProcessingError}</StatusMessage>
        {/if}

        <dl class="detail-metadata">
          <div>
            <dt>Source</dt>
            <dd>{selectedItem.siteName || hostname(selectedItem) || 'Unknown site'}</dd>
          </div>
          <div>
            <dt>Capture</dt>
            <dd class={`processing-state ${processingClass(selectedItem)}`}>
              {processingLabel(selectedItem)}
            </dd>
          </div>
          <div>
            <dt>Added</dt>
            <dd>{formatNoteTimestamp(selectedItem.createdAt, session.workspace.timezone)}</dd>
          </div>
          <div>
            <dt>Reading</dt>
            <dd>{selectedItem.readAt ? 'Read' : 'Unread'}</dd>
          </div>
          {#if selectedItem.author}
            <div>
              <dt>Author</dt>
              <dd>{selectedItem.author}</dd>
            </div>
          {/if}
          {#if selectedItem.publishedAt}
            <div>
              <dt>Published</dt>
              <dd>{formatNoteTimestamp(selectedItem.publishedAt, session.workspace.timezone)}</dd>
            </div>
          {/if}
        </dl>

        {#if selectedItem.excerpt}
          <section class="excerpt-section">
            <h3>Excerpt</h3>
            <p>{selectedItem.excerpt}</p>
          </section>
        {/if}

        <section class="source-section">
          <h3>Source</h3>
          {#if safeLink(selectedItem)}
            <a href={safeLink(selectedItem) ?? undefined} rel="noreferrer noopener" target="_blank">
              <span>{selectedItem.originalUrl}</span>
              <span aria-hidden="true">↗</span>
            </a>
          {:else}
            <p>{selectedItem.originalUrl}</p>
          {/if}
          {#if selectedItem.tags.length > 0}
            <div aria-label="Tags" class="detail-tags">
              {#each selectedItem.tags as tag}<span>#{tag}</span>{/each}
            </div>
          {/if}
        </section>

        <section class="captures-section">
          <div class="section-heading">
            <h3>Capture history</h3>
            <span>{selectedItem.captures.length}</span>
          </div>
          {#if selectedItem.captures.length === 0}
            <p class="detail-empty">No selections or notes were added when this link was saved.</p>
          {:else}
            <div class="capture-history">
              {#each selectedItem.captures as capture (capture.uid)}
                <article class="capture-record">
                  <header>
                    <span>{capture.capturedTitle || 'Manual capture'}</span>
                    <time datetime={capture.createdAt}
                      >{formatNoteTimestamp(capture.createdAt, session.workspace.timezone)}</time
                    >
                  </header>
                  {#if capture.selectedText}<blockquote>{capture.selectedText}</blockquote>{/if}
                  {#if capture.note}<p>{capture.note}</p>{/if}
                </article>
              {/each}
            </div>
          {/if}
        </section>
      {:else}
        <div class="detail-placeholder">
          <Icon name="library" size={22} />
          <h2>Item details</h2>
          <p>Select a saved link to review its source, state, and capture history.</p>
        </div>
      {/if}
    </aside>
  </div>
{/if}

<ConfirmDialog
  busy={deleteBusy}
  confirmLabel="Delete item"
  error={deleteError}
  message="This saved link and its capture history will be permanently deleted."
  onCancel={cancelDelete}
  onConfirm={confirmDelete}
  onFallbackFocus={focusAfterDelete}
  open={Boolean(pendingDelete)}
  title="Delete this Library item?"
/>

<style>
  .library-page {
    display: grid;
    width: min(100%, 1240px);
    min-height: 100%;
    grid-template-columns: minmax(0, 1fr) 356px;
    gap: 36px;
    padding: 38px 44px 72px;
    margin: 0 auto;
  }

  .library-primary {
    min-width: 0;
  }

  .library-index {
    padding-top: 30px;
  }

  .library-toolbar {
    display: flex;
    gap: 20px;
    align-items: end;
    justify-content: space-between;
  }

  .library-toolbar h2 {
    margin-bottom: 1px;
    font-size: 17px;
    font-weight: 660;
    letter-spacing: -0.015em;
  }

  .library-toolbar p {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .library-search {
    width: min(280px, 48%);
  }

  .library-filters {
    display: flex;
    min-height: 42px;
    gap: 18px;
    align-items: end;
    border-bottom: 1px solid var(--color-border);
  }

  .library-filters button {
    min-height: 40px;
    padding: 0 1px;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-bottom: 2px solid transparent;
    font-size: 12px;
    font-weight: 600;
  }

  .library-filters button:hover,
  .library-filters button.active {
    color: var(--color-text);
  }

  .library-filters button.active {
    color: var(--color-accent-hover);
    border-bottom-color: var(--color-accent);
  }

  .library-filters > span {
    margin: 0 0 11px auto;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .library-list {
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .library-list li {
    position: relative;
    display: grid;
    min-width: 0;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    border-bottom: 1px solid var(--color-border-soft);
    transition:
      background-color 150ms ease,
      opacity 150ms ease;
  }

  .library-list li::before {
    position: absolute;
    top: 16px;
    bottom: 16px;
    left: 0;
    width: 2px;
    background: transparent;
    border-radius: 2px;
    content: '';
  }

  .library-list li:hover,
  .library-list li.selected {
    background: var(--color-surface-muted);
  }

  .library-list li.selected::before {
    background: var(--color-accent);
  }

  .library-list li.busy {
    opacity: 0.68;
  }

  .item-select {
    display: grid;
    min-width: 0;
    gap: 4px;
    padding: 17px 8px 17px 12px;
    color: inherit;
    background: transparent;
    border: 0;
    text-align: left;
  }

  .item-title,
  .item-source,
  .item-excerpt {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-title,
  .item-source {
    white-space: nowrap;
  }

  .item-title {
    font-size: 14px;
    font-weight: 640;
    letter-spacing: -0.01em;
  }

  .item-source {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .item-excerpt {
    display: -webkit-box;
    color: var(--color-text-muted);
    font-family: var(--font-ui);
    font-size: 12px;
    line-height: 1.5;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .item-meta,
  .item-tags {
    display: flex;
    min-width: 0;
    flex-wrap: wrap;
    gap: 5px 12px;
    align-items: center;
    color: var(--color-text-muted);
    font-size: 10px;
  }

  .item-tags span,
  .detail-tags span {
    color: var(--color-accent-hover);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .processing-state {
    font-weight: 620;
  }

  .processing-state.ready {
    color: var(--color-accent-hover);
  }

  .processing-state.failed {
    color: var(--color-danger);
  }

  .item-actions {
    display: flex;
    gap: 2px;
    align-items: center;
    padding-right: 7px;
  }

  .text-action {
    min-height: 34px;
    padding: 0 7px;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    font-size: 11px;
  }

  .text-action:hover:not(:disabled) {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
  }

  .library-detail {
    position: sticky;
    top: 24px;
    min-width: 0;
    height: fit-content;
    max-height: calc(100dvh - 48px);
    padding: 3px 0 32px 28px;
    overflow: auto;
    border-left: 1px solid var(--color-border);
    scrollbar-width: thin;
  }

  .detail-header {
    display: flex;
    gap: 16px;
    align-items: start;
    justify-content: space-between;
    margin-bottom: 20px;
  }

  .detail-header h2 {
    margin: 0;
    font-family: var(--font-ui);
    font-size: 21px;
    font-weight: 650;
    line-height: 1.35;
    letter-spacing: -0.015em;
    overflow-wrap: anywhere;
  }

  .detail-loading {
    margin: -12px 0 14px;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .detail-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
    padding-bottom: 20px;
    border-bottom: 1px solid var(--color-border);
  }

  .detail-actions .button {
    gap: 6px;
    padding-inline: 10px;
  }

  .detail-actions .button.active {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
    border-color: var(--color-accent-soft);
  }

  .detail-actions .delete-action {
    color: var(--color-danger);
    background: transparent;
    border-color: transparent;
  }

  .detail-actions .delete-action:hover:not(:disabled) {
    background: var(--color-danger-soft);
    border-color: var(--color-danger-soft);
  }

  .content-state {
    display: flex;
    gap: 12px;
    align-items: start;
    justify-content: space-between;
    padding: 18px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .content-state.pending {
    justify-content: flex-start;
    animation: content-state-enter 180ms ease both;
  }

  .content-state-mark {
    width: 16px;
    height: 2px;
    margin-top: 9px;
    background: var(--color-accent);
    border-radius: 2px;
  }

  .content-state h3 {
    margin: 0 0 3px;
    font-size: 12px;
    font-weight: 660;
  }

  .content-state p {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 11px;
    line-height: 1.55;
  }

  .content-state .button {
    flex: none;
  }

  .detail-metadata {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px 20px;
    padding: 20px 0;
    margin: 0;
    border-bottom: 1px solid var(--color-border);
  }

  .detail-metadata div {
    min-width: 0;
  }

  .detail-metadata dt {
    margin-bottom: 2px;
    color: var(--color-text-muted);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .detail-metadata dd {
    margin: 0;
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .excerpt-section,
  .source-section,
  .captures-section {
    padding-top: 23px;
  }

  .excerpt-section h3,
  .source-section h3,
  .captures-section h3 {
    margin-bottom: 10px;
    font-size: 12px;
    font-weight: 660;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .excerpt-section p {
    margin: 0;
    color: var(--color-text-muted);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.68;
  }

  .source-section > a,
  .source-section > p {
    display: flex;
    min-width: 0;
    gap: 8px;
    align-items: start;
    justify-content: space-between;
    margin: 0;
    color: var(--color-accent-hover);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.6;
    overflow-wrap: anywhere;
  }

  .source-section > a:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .detail-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 7px 12px;
    margin-top: 14px;
  }

  .section-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    border-bottom: 1px solid var(--color-border);
  }

  .section-heading > span {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .detail-empty {
    padding: 18px 0;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .capture-record {
    padding: 18px 0;
    border-bottom: 1px solid var(--color-border-soft);
  }

  .capture-record header {
    display: flex;
    gap: 12px;
    justify-content: space-between;
    margin-bottom: 9px;
    color: var(--color-text-muted);
    font-size: 10px;
  }

  .capture-record time {
    flex: none;
    font-family: var(--font-mono);
  }

  .capture-record blockquote {
    padding-left: 13px;
    margin: 0 0 10px;
    border-left: 2px solid var(--color-accent);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.65;
  }

  .capture-record p {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 12px;
    white-space: pre-wrap;
  }

  .detail-placeholder {
    display: grid;
    min-height: 300px;
    align-content: center;
    justify-items: start;
    color: var(--color-text-muted);
  }

  .detail-placeholder h2 {
    margin: 13px 0 4px;
    color: var(--color-text);
    font-size: 15px;
  }

  .detail-placeholder p {
    max-width: 250px;
    font-size: 12px;
  }

  .detail-backdrop {
    display: none;
  }

  @keyframes content-state-enter {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
  }

  @media (max-width: 1199px) {
    .library-page {
      display: block;
      width: min(100%, 940px);
      padding: 32px 36px 72px;
    }

    .library-detail {
      position: fixed;
      top: calc(56px + env(safe-area-inset-top));
      right: 0;
      bottom: 0;
      z-index: 70;
      width: min(420px, calc(100vw - 64px));
      height: auto;
      max-height: none;
      padding: 28px 28px 56px;
      background: var(--color-surface);
      border-left: 1px solid var(--color-border);
      box-shadow: var(--shadow-floating);
      transform: translateX(102%);
      visibility: hidden;
      transition:
        transform 190ms ease,
        visibility 190ms step-end;
    }

    .library-detail.open {
      transform: translateX(0);
      visibility: visible;
      transition:
        transform 190ms ease,
        visibility 0ms step-start;
    }

    .detail-backdrop {
      position: fixed;
      top: calc(56px + env(safe-area-inset-top));
      right: 0;
      bottom: 0;
      left: 64px;
      z-index: 68;
      display: block;
      padding: 0;
      background: color-mix(in oklch, var(--color-text), transparent 78%);
      border: 0;
    }
  }

  @media (max-width: 767px) {
    .library-page {
      width: 100%;
      padding: 24px 16px 54px;
    }

    .library-toolbar {
      display: grid;
      gap: 14px;
      align-items: start;
    }

    .library-search {
      width: 100%;
    }

    .library-filters button {
      min-width: 68px;
      min-height: 44px;
    }

    .library-list li {
      grid-template-columns: minmax(0, 1fr);
      gap: 0;
      padding-bottom: 8px;
    }

    .item-select {
      min-height: 44px;
      padding: 16px 8px 9px 12px;
    }

    .item-actions {
      justify-content: flex-end;
      padding: 0 7px;
    }

    .item-actions .icon-button,
    .text-action {
      min-width: 44px;
      min-height: 44px;
    }

    .library-detail {
      top: calc(56px + env(safe-area-inset-top));
      bottom: 0;
      width: 100%;
      padding: 24px 18px 36px;
      border-left: 0;
      box-shadow: none;
    }

    .detail-backdrop {
      top: calc(56px + env(safe-area-inset-top));
      bottom: 0;
      left: 0;
    }

    .detail-actions .button {
      min-height: 44px;
    }

    .content-state .button {
      min-height: 44px;
    }
  }
</style>
