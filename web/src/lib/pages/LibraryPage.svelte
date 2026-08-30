<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import BookOpenCheck from '@lucide/svelte/icons/book-open-check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import ListFilter from '@lucide/svelte/icons/list-filter';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Search from '@lucide/svelte/icons/search';
  import Star from '@lucide/svelte/icons/star';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import { onMount, tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import {
    acceptLibraryRefreshCandidate,
    createLibraryItem,
    deleteLibraryItem,
    discardLibraryRefreshCandidate,
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
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import LibraryCaptureForm from '../components/LibraryCaptureForm.svelte';
  import LibraryReader from '../components/LibraryReader.svelte';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { Button } from '../components/ui/button';
  import * as DropdownMenu from '../components/ui/dropdown-menu';
  import * as Empty from '../components/ui/empty';
  import { Input } from '../components/ui/input';
  import { Spinner } from '../components/ui/spinner';
  import { captureListFocus, restoreListFocus } from '../utils/focus';
  import { formatCompactDate } from '../utils/date';

  let {
    onImmersiveChange = () => {},
    session,
  }: {
    onImmersiveChange?: (open: boolean) => void;
    session: SessionInfo;
  } = $props();

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
  let actionMenuUid = $state<string | null>(null);
  let deleteFocusSnapshot = $state<ReturnType<typeof captureListFocus> | null>(null);
  let listOnlyLayout = $state(false);
  let immersiveReader = $state(false);
  let pageElement = $state<HTMLElement>();
  let searchInput = $state<HTMLInputElement | null>(null);
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
  const READER_HISTORY_KEY = 'locusLibraryReader';

  onMount(() => {
    const listOnlyMedia = window.matchMedia('(max-width: 1199px)');
    const updateMedia = () => {
      const nextListOnlyLayout = listOnlyMedia.matches;
      if (nextListOnlyLayout && !listOnlyLayout && selectedUid && !readerItem) {
        void closeDetail();
      }
      listOnlyLayout = nextListOnlyLayout;
    };
    const handlePopState = () => {
      if (immersiveReader) void closeReader();
    };
    updateMedia();
    listOnlyMedia.addEventListener('change', updateMedia);
    window.addEventListener('popstate', handlePopState);
    void loadItems(true);

    return () => {
      listOnlyMedia.removeEventListener('change', updateMedia);
      window.removeEventListener('popstate', handlePopState);
      if (immersiveReader) onImmersiveChange(false);
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

    if (listOnlyLayout) {
      openReader(item, true);
      return;
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
    if (
      !item ||
      (item.processingStatus !== 'PENDING' && item.refreshStatus !== 'PENDING') ||
      selectedUid !== item.uid
    )
      return;
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
      if (updated.processingStatus === 'PENDING' || updated.refreshStatus === 'PENDING') {
        schedulePoll(uid, generation);
      }
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

  function openReader(item: LibraryItem, immersive = false): void {
    if (!immersive && (item.processingStatus !== 'READY' || !item.contentAvailable)) return;
    detailRequestId += 1;
    detailController?.abort();
    detailController = null;
    detailLoading = false;
    if (item.processingStatus === 'PENDING' || item.refreshStatus === 'PENDING') startPolling(item);
    else stopPolling();
    operationError = null;
    actionStatus = null;
    immersiveReader = immersive;
    if (immersive) {
      if (window.history.state?.[READER_HISTORY_KEY] !== item.uid) {
        window.history.pushState(
          { ...(window.history.state ?? {}), [READER_HISTORY_KEY]: item.uid },
          '',
          window.location.href,
        );
      }
      onImmersiveChange(true);
    }
    readerItem = item;
  }

  function leaveReader(): void {
    const returnThroughHistory =
      immersiveReader && window.history.state?.[READER_HISTORY_KEY] === readerItem?.uid;
    void closeReader();
    if (returnThroughHistory) window.history.back();
  }

  async function closeReader(): Promise<void> {
    const uid = readerItem?.uid;
    const wasImmersive = immersiveReader;
    readerItem = null;
    immersiveReader = false;
    if (wasImmersive) onImmersiveChange(false);

    if (wasImmersive) {
      stopPolling();
      selectedUid = null;
      selectedItem = null;
      await tick();
      if (!uid) return;
      [...(pageElement?.querySelectorAll<HTMLButtonElement>('[data-library-select]') ?? [])]
        .find((button) => button.dataset.librarySelect === uid)
        ?.focus();
      return;
    }

    if (selectedItem) startPolling(selectedItem);
    await tick();
    if (!uid) return;
    const expandButton = [
      ...(pageElement?.querySelectorAll<HTMLButtonElement>('[data-library-expand]') ?? []),
    ].find((button) => button.dataset.libraryExpand === uid);
    if (expandButton) {
      expandButton.focus();
    }
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

  async function resolveRefreshCandidate(item: LibraryItem, accept: boolean): Promise<void> {
    if (busyUids.has(item.uid)) return;
    markBusy(item.uid, true);
    operationError = null;
    actionStatus = null;
    try {
      const updated = accept
        ? await acceptLibraryRefreshCandidate(item.uid)
        : await discardLibraryRefreshCandidate(item.uid);
      replaceItem(updated);
      actionStatus = accept ? 'Refreshed version saved.' : 'Saved version kept.';
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to resolve the refreshed version.');
    } finally {
      markBusy(item.uid, false);
    }
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
    if (!listOnlyLayout && event.key === 'Escape') {
      event.preventDefault();
      void closeDetail();
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

  function faviconUrl(item: LibraryItem): string {
    const itemUrl = item.canonicalUrl ?? item.normalizedUrl ?? item.originalUrl;
    let domainUrl = itemUrl;
    try {
      domainUrl = new URL(itemUrl).origin;
    } catch {}
    return `https://www.google.com/s2/favicons?domain_url=${encodeURIComponent(domainUrl)}&sz=64`;
  }

  function faviconFallback(item: LibraryItem): string {
    return (hostname(item) || item.siteName || '?').charAt(0).toLocaleUpperCase();
  }

  function hideBrokenFavicon(event: Event): void {
    if (event.currentTarget instanceof HTMLImageElement) event.currentTarget.hidden = true;
  }

  function visibleProcessingLabel(item: LibraryItem): string | null {
    if (item.processingStatus === 'PENDING') return 'Processing';
    if (item.processingStatus === 'FAILED') return 'Fetch failed';
    if (item.refreshStatus === 'PENDING') return 'Refreshing';
    if (item.refreshStatus === 'FAILED') return 'Refresh failed';
    if (item.refreshStatus === 'REVIEW') return 'Review refresh';
    return null;
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if readerItem}
  <LibraryReader
    {actionStatus}
    busy={busyUids.has(readerItem.uid) || retryingUid === readerItem.uid}
    item={readerItem}
    onAcceptRefresh={(item) => resolveRefreshCandidate(item, true)}
    onBack={leaveReader}
    onDiscardRefresh={(item) => resolveRefreshCandidate(item, false)}
    onRetry={retryCapture}
    onToggleRead={toggleRead}
    {operationError}
    timeZone={session.workspace.timezone}
  />
{:else}
  <div bind:this={pageElement} class:detail-open={Boolean(selectedUid)} class="library-page">
    <div class="library-primary">
      <header class="library-page-header">
        <h1 id="library-title">Library</h1>
        <LibraryCaptureForm onCreate={handleCreate} />
      </header>

      <section aria-labelledby="library-title" class="library-index">
        <div class="library-toolbar">
          <label class="search-field library-search">
            <Search class="pointer-events-none absolute left-3 size-4 text-muted-foreground" />
            <span class="sr-only">Search links</span>
            <Input
              class="pl-9"
              autocomplete="off"
              bind:ref={searchInput}
              oninput={handleSearch}
              placeholder="Search links"
              type="search"
              value={query}
            />
          </label>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  aria-label={`Status: ${status === 'ACTIVE' ? 'Active' : 'Archived'}`}
                  class="status-filter"
                  variant="outline"
                >
                  <ListFilter data-icon="inline-start" />
                  {status === 'ACTIVE' ? 'Active' : 'Archived'}
                  <ChevronDown data-icon="inline-end" />
                </Button>
              {/snippet}
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end" class="w-40">
              <DropdownMenu.Group>
                <DropdownMenu.Label>Status</DropdownMenu.Label>
                <DropdownMenu.RadioGroup value={status}>
                  <DropdownMenu.RadioItem onclick={() => selectStatus('ACTIVE')} value="ACTIVE">
                    Active
                  </DropdownMenu.RadioItem>
                  <DropdownMenu.RadioItem onclick={() => selectStatus('ARCHIVED')} value="ARCHIVED">
                    Archived
                  </DropdownMenu.RadioItem>
                </DropdownMenu.RadioGroup>
              </DropdownMenu.Group>
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </div>

        <span aria-live="polite" class="sr-only" data-library-count
          >{`${total} ${
            total === 1 ? 'item' : 'items'
          } in this view.${loading && items.length > 0 ? ' Updating.' : ''}`}</span
        >

        {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
        <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
          {actionStatus ?? ''}
        </div>

        <div aria-busy={loading || loadingMore} class="library-results">
          {#if loading && items.length === 0}
            <div
              aria-live="polite"
              class="loading-state large flex items-center justify-center gap-2"
            >
              <Spinner />
              Loading Library…
            </div>
          {:else if loadError}
            <Empty.Root>
              <Empty.Header>
                <Empty.Title>Library unavailable</Empty.Title>
                <Empty.Description>{loadError}</Empty.Description>
              </Empty.Header>
              <Empty.Content>
                <Button onclick={() => void loadItems(true)} variant="secondary">Try again</Button>
              </Empty.Content>
            </Empty.Root>
          {:else if items.length === 0}
            <Empty.Root>
              <Empty.Header>
                <Empty.Title>
                  {query
                    ? 'No saved items found'
                    : status === 'ARCHIVED'
                      ? 'No archived items'
                      : 'Save your first link'}
                </Empty.Title>
                <Empty.Description>
                  {query
                    ? 'Try another keyword.'
                    : status === 'ARCHIVED'
                      ? 'Archived links will remain available here.'
                      : 'Use Add link to save an article URL.'}
                </Empty.Description>
              </Empty.Header>
              {#if query}
                <Empty.Content>
                  <Button
                    onclick={() => {
                      query = '';
                      if (searchInput) searchInput.value = '';
                      void loadItems(true);
                    }}
                    variant="secondary">Clear search</Button
                  >
                </Empty.Content>
              {/if}
            </Empty.Root>
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
                    <span aria-hidden="true" class="item-favicon">
                      <span>{faviconFallback(item)}</span>
                      <img
                        alt=""
                        decoding="async"
                        loading="lazy"
                        onerror={hideBrokenFavicon}
                        referrerpolicy="no-referrer"
                        src={faviconUrl(item)}
                      />
                    </span>
                    <span class="item-copy">
                      <span class="item-heading">
                        <span class="item-title">{displayTitle(item)}</span>
                        {#if !item.readAt}
                          <span aria-hidden="true" class="unread-mark"></span>
                          <span class="sr-only">Unread</span>
                        {/if}
                      </span>
                      <span class="item-meta">
                        <span class="item-source"
                          >{item.siteName || hostname(item) || item.originalUrl}</span
                        >
                        <span aria-hidden="true">·</span>
                        <span
                          >{formatCompactDate(
                            item.publishedAt ?? item.createdAt,
                            session.workspace.timezone,
                          )}</span
                        >
                        {#if visibleProcessingLabel(item)}
                          <span aria-hidden="true">·</span>
                          <span
                            class:failed={item.processingStatus === 'FAILED'}
                            class="processing-state"
                          >
                            {visibleProcessingLabel(item)}
                          </span>
                        {/if}
                      </span>
                    </span>
                  </button>
                  <div class="item-actions">
                    <DropdownMenu.Root
                      onOpenChange={(open) => (actionMenuUid = open ? item.uid : null)}
                      open={actionMenuUid === item.uid}
                    >
                      <DropdownMenu.Trigger disabled={busyUids.has(item.uid)}>
                        {#snippet child({ props })}
                          <Button
                            {...props}
                            aria-label={`Actions for ${displayTitle(item)}`}
                            size="icon-sm"
                            variant="ghost"
                          >
                            <Ellipsis />
                          </Button>
                        {/snippet}
                      </DropdownMenu.Trigger>
                      {#if actionMenuUid === item.uid}
                        <DropdownMenu.Content align="end" class="w-44" forceMount>
                          <DropdownMenu.Group>
                            <DropdownMenu.Item
                              aria-label={`${item.starred ? 'Unstar' : 'Star'} ${displayTitle(item)}`}
                              onclick={() => void toggleStar(item)}
                            >
                              <Star />
                              {item.starred ? 'Unstar' : 'Star'}
                            </DropdownMenu.Item>
                            <DropdownMenu.Item onclick={() => void toggleRead(item)}>
                              <BookOpenCheck />
                              {item.readAt ? 'Mark unread' : 'Mark read'}
                            </DropdownMenu.Item>
                            <DropdownMenu.Item
                              aria-label={`${item.status === 'ACTIVE' ? 'Archive' : 'Restore'} ${displayTitle(item)}`}
                              onclick={() => void toggleArchive(item)}
                            >
                              {#if item.status === 'ACTIVE'}<Archive />{:else}<RotateCcw />{/if}
                              {item.status === 'ACTIVE' ? 'Archive' : 'Restore'}
                            </DropdownMenu.Item>
                          </DropdownMenu.Group>
                          <DropdownMenu.Separator />
                          <DropdownMenu.Item
                            aria-label={`Delete ${displayTitle(item)}`}
                            onclick={() => requestDelete(item)}
                            variant="destructive"
                          >
                            <Trash2 />
                            Delete
                          </DropdownMenu.Item>
                        </DropdownMenu.Content>
                      {/if}
                    </DropdownMenu.Root>
                  </div>
                </li>
              {/each}
            </ol>

            {#if items.length < total}
              <Button
                class="load-more"
                disabled={loadingMore}
                onclick={() => void loadItems(false)}
                variant="secondary"
              >
                {#if loadingMore}<Spinner data-icon="inline-start" />{/if}
                {loadingMore ? 'Loading…' : 'Load more'}
              </Button>
            {/if}
          {/if}
        </div>
      </section>
    </div>

    <aside aria-label="Library article preview" class="library-detail">
      {#if selectedItem}
        {#key selectedItem.uid}
          <LibraryReader
            {actionStatus}
            busy={detailLoading ||
              busyUids.has(selectedItem.uid) ||
              retryingUid === selectedItem.uid}
            item={selectedItem}
            mode="preview"
            onAcceptRefresh={(item) => resolveRefreshCandidate(item, true)}
            onBack={closeDetail}
            onDiscardRefresh={(item) => resolveRefreshCandidate(item, false)}
            onExpand={() => openReader(selectedItem!)}
            onRetry={retryCapture}
            onToggleRead={toggleRead}
            operationError={operationError ||
              detailError ||
              detailPollingError ||
              detailProcessingError}
            timeZone={session.workspace.timezone}
          />
        {/key}
      {:else}
        <div aria-hidden="true" class="empty-preview"></div>
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
    width: 100%;
    min-height: 100%;
    grid-template-columns: minmax(360px, 440px) minmax(0, 1fr);
  }

  .library-primary {
    min-width: 0;
    padding: 28px 20px 72px 28px;
    border-right: 1px solid var(--border);
  }

  .library-page-header {
    display: flex;
    gap: 20px;
    align-items: center;
    justify-content: space-between;
  }

  .library-page-header h1 {
    margin: 0;
    font-size: 23px;
    font-weight: 680;
    line-height: 1.2;
    letter-spacing: -0.03em;
  }

  .library-index {
    padding-top: 20px;
  }

  .library-toolbar {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-bottom: 12px;
  }

  .library-search {
    min-width: 0;
    flex: 1;
  }

  .library-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .library-list li {
    position: relative;
    display: grid;
    min-width: 0;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px;
    align-items: center;
    border-radius: 8px;
    transition:
      background-color 150ms ease,
      opacity 150ms ease;
  }

  .library-list li::before {
    position: absolute;
    top: 12px;
    bottom: 12px;
    left: 0;
    width: 2px;
    background: transparent;
    border-radius: 2px;
    content: '';
  }

  .library-list li:hover,
  .library-list li.selected {
    background: var(--muted);
  }

  .library-list li.selected::before {
    background: var(--primary);
  }

  .library-list li.busy {
    opacity: 0.68;
  }

  .item-select {
    display: grid;
    min-width: 0;
    grid-template-columns: 18px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    padding: 12px 4px 12px 12px;
    color: inherit;
    background: transparent;
    border: 0;
    text-align: left;
  }

  .item-favicon {
    position: relative;
    display: grid;
    width: 18px;
    height: 18px;
    place-items: center;
    overflow: hidden;
    color: var(--muted-foreground);
    background: var(--muted);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 650;
  }

  .item-favicon img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .item-copy {
    display: grid;
    min-width: 0;
    gap: 4px;
  }

  .item-heading {
    display: flex;
    min-width: 0;
    gap: 7px;
    align-items: center;
  }

  .item-title,
  .item-source {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-title,
  .item-source {
    white-space: nowrap;
  }

  .item-title {
    min-width: 0;
    flex: 1;
    font-size: 14px;
    font-weight: 640;
    line-height: 1.35;
    letter-spacing: -0.01em;
  }

  .item-source {
    min-width: 0;
  }

  .item-meta {
    display: flex;
    min-width: 0;
    gap: 5px;
    align-items: center;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 10px;
    white-space: nowrap;
  }

  .processing-state {
    font-weight: 620;
  }

  .processing-state.failed {
    color: var(--destructive);
  }

  .unread-mark {
    width: 5px;
    height: 5px;
    flex: none;
    background: var(--primary);
    border-radius: 50%;
  }

  .item-actions {
    display: flex;
    gap: 2px;
    align-items: center;
    padding-right: 4px;
    opacity: 0;
    transition: opacity 150ms ease;
  }

  .library-list li:hover .item-actions,
  .library-list li:focus-within .item-actions,
  .library-list li.selected .item-actions {
    opacity: 1;
  }

  .library-detail {
    position: sticky;
    top: 0;
    min-width: 0;
    height: 100dvh;
    max-height: 100dvh;
    overflow: auto;
    scrollbar-width: thin;
  }

  .empty-preview {
    min-height: 100%;
  }

  @media (max-width: 1199px) {
    .library-page {
      display: block;
      width: min(100%, 940px);
      padding: 32px 36px 72px;
      margin-inline: auto;
    }

    .library-primary {
      padding: 0;
      border-right: 0;
    }

    .library-detail {
      display: none;
    }
  }

  @media (max-width: 767px) {
    .library-page {
      width: 100%;
      padding: 24px 16px 54px;
    }

    .item-select {
      min-height: 44px;
      padding-block: 12px;
    }

    .item-actions {
      padding-right: 0;
      opacity: 1;
    }
  }

  @media (hover: none) {
    .item-actions {
      opacity: 1;
    }
  }
</style>
