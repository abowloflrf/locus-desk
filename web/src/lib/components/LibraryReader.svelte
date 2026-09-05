<script lang="ts">
  import '../vitesse-light.css';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import BookOpenCheck from '@lucide/svelte/icons/book-open-check';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Maximize2 from '@lucide/svelte/icons/maximize-2';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import X from '@lucide/svelte/icons/x';
  import { onMount, tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import { getLibraryContent } from '../api/library';
  import type { LibraryContentResponse, LibraryItem } from '../api/types';
  import {
    highlightLibraryHtml,
    safeLibrarySourceUrl,
    sanitizeLibraryHtml,
  } from '../library-content';
  import {
    DEFAULT_READER_PREFERENCES,
    loadReaderPreferences,
    saveReaderPreferences,
    type ReaderPreferences,
  } from '../reader-preferences';
  import { formatNoteTimestamp } from '../utils/date';
  import { createReaderOutline } from '../reader-outline';
  import ReaderPreferencesControl from './ReaderPreferences.svelte';
  import ReaderTableOfContents from './ReaderTableOfContents.svelte';
  import StatusMessage from './StatusMessage.svelte';
  import { Button } from './ui/button';
  import * as Alert from './ui/alert';
  import * as Empty from './ui/empty';
  import { Spinner } from './ui/spinner';

  let {
    actionStatus = null,
    busy = false,
    item,
    onBack,
    onAcceptRefresh,
    onDiscardRefresh,
    onExpand,
    onRetry,
    onToggleRead,
    operationError = null,
    mode = 'full',
    timeZone,
  }: {
    actionStatus?: string | null;
    busy?: boolean;
    item: LibraryItem;
    onBack: () => void | Promise<void>;
    onAcceptRefresh?: (item: LibraryItem) => void | Promise<void>;
    onDiscardRefresh?: (item: LibraryItem) => void | Promise<void>;
    onExpand?: () => void | Promise<void>;
    onRetry?: (item: LibraryItem) => void | Promise<void>;
    onToggleRead: (item: LibraryItem) => void | Promise<void>;
    operationError?: string | null;
    mode?: 'preview' | 'full';
    timeZone: string;
  } = $props();

  let content = $state<LibraryContentResponse | null>(null);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let loadStatus = $state('');
  let readerPreferences = $state<ReaderPreferences>({ ...DEFAULT_READER_PREFERENCES });
  let readerRoot = $state<HTMLElement>();
  let readerHeading = $state<HTMLHeadingElement>();
  let readerToolbar = $state<HTMLElement>();
  let readerArticle = $state<HTMLElement>();
  const readerId = $props.id();
  let toolbarHidden = $state(false);
  let contentController: AbortController | null = null;
  let contentRequestId = 0;
  let requestedContentKey = '';
  let readerScrollElement = $state<HTMLElement | null>(null);
  let lastScrollTop = 0;
  let scrollDelta = 0;
  let keyboardInteraction = false;
  let toolbarKeyboardFocus = false;

  const sourceUrl = $derived(
    safeLibrarySourceUrl(item.canonicalUrl ?? item.normalizedUrl ?? item.originalUrl),
  );
  const sanitizedHtml = $derived(
    content ? sanitizeLibraryHtml(content.safeHtml, sourceUrl ?? item.originalUrl) : '',
  );
  let highlighted = $state<{ source: string; html: string } | null>(null);
  const readerHtml = $derived(
    highlighted?.source === sanitizedHtml ? highlighted.html : sanitizedHtml,
  );
  const outline = $derived(createReaderOutline(readerHtml, readerId));
  const showOutline = $derived(
    !loading && !loadError && item.processingStatus === 'READY' && outline.headings.length > 1,
  );

  $effect(() => {
    const source = sanitizedHtml;
    let active = true;
    void highlightLibraryHtml(source, sourceUrl ?? item.originalUrl)
      .then((html) => {
        if (active) highlighted = { source, html };
      })
      .catch(() => {
        // Keep the sanitized article readable if highlighting fails.
      });
    return () => {
      active = false;
    };
  });

  const plainText = $derived(content?.plainText.trim() ?? '');

  onMount(() => {
    readerPreferences = loadReaderPreferences(window.localStorage);
    if (mode === 'full') void tick().then(() => readerHeading?.focus());
    readerScrollElement =
      readerRoot?.closest<HTMLElement>('.library-detail, .workspace-column') ?? null;
    const scrollTarget: EventTarget = readerScrollElement ?? window;
    lastScrollTop = currentScrollTop();
    scrollTarget.addEventListener('scroll', handleReaderScroll, { passive: true });

    return () => {
      scrollTarget.removeEventListener('scroll', handleReaderScroll);
      contentRequestId += 1;
      contentController?.abort();
      contentController = null;
    };
  });

  $effect(() => {
    const ready = item.processingStatus === 'READY' && item.contentAvailable;
    const contentKey = `${item.uid}:${item.contentVersion}`;
    if (!ready) {
      loading = false;
      return;
    }
    if (requestedContentKey === contentKey) return;
    requestedContentKey = contentKey;
    void loadContent();
  });

  async function loadContent(): Promise<void> {
    const id = ++contentRequestId;
    contentController?.abort();
    contentController = new AbortController();
    loading = true;
    loadError = null;
    loadStatus = '';

    try {
      const response = await getLibraryContent(item.uid, contentController.signal);
      if (id !== contentRequestId) return;
      content = response;
      loadStatus = response.safeHtml.trim() || response.plainText.trim() ? 'Article ready.' : '';
    } catch (cause) {
      if (isAbortError(cause)) return;
      if (id === contentRequestId) {
        content = null;
        loadError = errorMessage(cause, 'Unable to load this article.');
      }
    } finally {
      if (id === contentRequestId) loading = false;
    }
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    keyboardInteraction = true;
    if (readerToolbar?.contains(document.activeElement)) {
      toolbarKeyboardFocus = true;
      toolbarHidden = false;
    }
    if (
      mode === 'preview' ||
      event.defaultPrevented ||
      event.key !== 'Escape' ||
      document.querySelector(
        'dialog[open], [data-slot="popover-content"], [data-slot="sheet-content"]',
      )
    ) {
      return;
    }
    event.preventDefault();
    void onBack();
  }

  function handlePointerInteraction(): void {
    keyboardInteraction = false;
    toolbarKeyboardFocus = false;
  }

  function handleToolbarFocusIn(): void {
    toolbarKeyboardFocus = keyboardInteraction;
    toolbarHidden = false;
  }

  function handleToolbarFocusOut(event: FocusEvent): void {
    if (!(event.relatedTarget instanceof Node) || !readerToolbar?.contains(event.relatedTarget)) {
      toolbarKeyboardFocus = false;
    }
  }

  function updateReaderPreferences(preferences: ReaderPreferences): void {
    readerPreferences = preferences;
    saveReaderPreferences(window.localStorage, preferences);
  }

  function currentScrollTop(): number {
    return Math.max(0, readerScrollElement?.scrollTop ?? window.scrollY);
  }

  function handleReaderScroll(): void {
    const scrollTop = currentScrollTop();
    const delta = scrollTop - lastScrollTop;
    scrollDelta = delta > 0 ? Math.max(0, scrollDelta) + delta : Math.min(0, scrollDelta) + delta;

    if (toolbarKeyboardFocus && readerToolbar?.contains(document.activeElement)) {
      toolbarHidden = false;
      scrollDelta = 0;
    } else if (scrollTop <= 16) {
      toolbarHidden = false;
      scrollDelta = 0;
    } else if (scrollTop > 72 && scrollDelta >= 16) {
      toolbarHidden = true;
      scrollDelta = 0;
    } else if (scrollDelta <= -10) {
      toolbarHidden = false;
      scrollDelta = 0;
    }
    lastScrollTop = scrollTop;
  }

  function displayTitle(): string {
    return item.title.trim() || item.siteName || sourceHostname() || 'Untitled article';
  }

  function sourceHostname(): string {
    try {
      return new URL(sourceUrl ?? item.originalUrl).hostname.replace(/^www\./, '');
    } catch {
      return '';
    }
  }

  function publishedLabel(): string | null {
    const date = new Date(item.publishedAt ?? item.createdAt);
    if (Number.isNaN(date.getTime())) return null;
    return new Intl.DateTimeFormat('en', {
      day: 'numeric',
      month: 'short',
      timeZone,
      year: 'numeric',
    }).format(date);
  }

  function isAbortError(cause: unknown): boolean {
    return cause instanceof DOMException && cause.name === 'AbortError';
  }
</script>

<svelte:window
  onkeydown={handleWindowKeydown}
  onpointerdown={handlePointerInteraction}
  onwheel={handlePointerInteraction}
/>

<section
  aria-busy={loading ||
    item.processingStatus === 'PENDING' ||
    item.refreshStatus === 'PENDING' ||
    busy}
  aria-labelledby="library-reader-title"
  bind:this={readerRoot}
  class:preview={mode === 'preview'}
  class="library-reader"
  data-reader-font={readerPreferences.fontPreset}
  data-reader-line-height={readerPreferences.lineHeight}
  data-reader-size={readerPreferences.fontSize}
  data-reader-width={readerPreferences.width}
>
  <header
    bind:this={readerToolbar}
    class:toolbar-hidden={toolbarHidden}
    class="reader-toolbar"
    onfocusin={handleToolbarFocusIn}
    onfocusout={handleToolbarFocusOut}
  >
    {#if mode === 'full'}
      <Button onclick={() => void onBack()} variant="ghost">
        <ArrowLeft data-icon="inline-start" />
        Back to Library
      </Button>
    {:else}
      <span class="reader-toolbar-label">Article preview</span>
    {/if}

    <div class="reader-actions">
      <ReaderPreferencesControl
        onChange={updateReaderPreferences}
        preferences={readerPreferences}
      />
      {#if sourceUrl}
        <Button
          aria-label={`Open source: ${sourceHostname() || 'saved page'}`}
          href={sourceUrl}
          rel="noopener noreferrer"
          target="_blank"
          title="Open source"
          variant="ghost"
        >
          <span class="reader-action-label">{sourceHostname() || 'Open source'}</span>
          <ExternalLink data-icon="inline-end" />
        </Button>
      {/if}
      {#if onRetry && item.processingStatus === 'READY'}
        <Button
          aria-label="Refresh article"
          disabled={busy || item.refreshStatus === 'PENDING'}
          onclick={() => void onRetry(item)}
          title="Refresh article"
          variant="ghost"
        >
          {#if busy || item.refreshStatus === 'PENDING'}<Spinner
              data-icon="inline-start"
            />{:else}<RefreshCw data-icon="inline-start" />{/if}
          <span class="reader-action-label"
            >{item.refreshStatus === 'PENDING' ? 'Refreshing' : 'Refresh'}</span
          >
        </Button>
      {/if}
      <Button
        aria-label={item.readAt ? 'Mark as unread' : 'Mark as read'}
        aria-pressed={Boolean(item.readAt)}
        disabled={busy}
        onclick={() => void onToggleRead(item)}
        variant={item.readAt ? 'secondary' : 'ghost'}
      >
        <BookOpenCheck data-icon="inline-start" />
        <span class="reader-action-label">{item.readAt ? 'Read' : 'Mark read'}</span>
      </Button>
      {#if mode === 'preview' && onExpand && item.processingStatus === 'READY' && item.contentAvailable}
        <Button
          aria-label="Open full screen"
          data-library-expand={item.uid}
          onclick={() => void onExpand()}
          size="icon"
          title="Open full screen"
          variant="ghost"
        >
          <Maximize2 />
        </Button>
      {/if}
      {#if mode === 'preview'}
        <Button
          aria-label="Collapse preview"
          onclick={() => void onBack()}
          size="icon"
          title="Collapse preview"
          variant="ghost"
        >
          <X />
        </Button>
      {/if}
    </div>
  </header>

  <div class="reader-layout" class:has-outline={showOutline}>
    <div class="reader-column">
      <header class="reader-heading">
        <p class="reader-source">{item.siteName || sourceHostname() || 'Saved article'}</p>
        <h1 bind:this={readerHeading} id="library-reader-title" tabindex="-1">{displayTitle()}</h1>
        {#if item.excerpt}
          <aside aria-label="Article excerpt" class="reader-excerpt">
            <p class="reader-excerpt-label">Article excerpt</p>
            <p class="reader-excerpt-copy">{item.excerpt}</p>
          </aside>
        {/if}
        <p class="reader-byline">
          {#if item.author}<span>{item.author}</span>{/if}
          {#if publishedLabel()}<span>{publishedLabel()}</span>{/if}
          {#if content?.fetchedAt}
            <span>Saved {formatNoteTimestamp(content.fetchedAt, timeZone)}</span>
          {/if}
        </p>
      </header>

      {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
      {#if item.refreshStatus === 'PENDING' && item.contentAvailable}
        <Alert.Root>
          <Spinner />
          <Alert.Title>Refreshing saved article</Alert.Title>
          <Alert.Description
            >The current saved version remains available while this runs.</Alert.Description
          >
        </Alert.Root>
      {:else if item.refreshStatus === 'FAILED' && item.contentAvailable}
        <Alert.Root variant="destructive">
          <TriangleAlert />
          <Alert.Title>Refresh failed</Alert.Title>
          <Alert.Description>
            {item.refreshError || 'The source could not be refreshed. The saved version was kept.'}
          </Alert.Description>
        </Alert.Root>
      {:else if item.refreshStatus === 'REVIEW' && item.contentAvailable}
        <Alert.Root>
          <TriangleAlert />
          <Alert.Title>Shorter refresh needs review</Alert.Title>
          <Alert.Description>
            The refreshed text is much shorter than the saved version. The saved version is still
            displayed.
          </Alert.Description>
          {#if onAcceptRefresh && onDiscardRefresh}
            <div class="refresh-review-actions">
              <Button
                disabled={busy}
                onclick={() => void onDiscardRefresh(item)}
                size="sm"
                variant="secondary">Keep saved version</Button
              >
              <Button
                disabled={busy}
                onclick={() => void onAcceptRefresh(item)}
                size="sm"
                variant="outline">Use refreshed version</Button
              >
            </div>
          {/if}
        </Alert.Root>
      {/if}
      <div aria-atomic="true" aria-live="polite" class="sr-only" role="status">
        {actionStatus ?? ''}
      </div>
      <div aria-atomic="true" aria-live="polite" class="sr-only" role="status">
        {loadStatus}
      </div>

      {#if loading}
        <div aria-live="polite" class="reader-state">
          <Spinner />
          <p>Opening saved article…</p>
        </div>
      {:else if item.processingStatus === 'PENDING'}
        <div aria-live="polite" class="reader-state">
          <Spinner />
          <p>Preparing article…</p>
        </div>
      {:else if item.processingStatus === 'FAILED'}
        <div class="reader-state reader-error">
          <StatusMessage tone="error">
            {item.lastError || 'Article capture failed.'}
          </StatusMessage>
          {#if onRetry}
            <Button disabled={busy} onclick={() => void onRetry(item)} variant="secondary">
              Retry
            </Button>
          {/if}
        </div>
      {:else if item.processingStatus === 'NOT_FETCHED'}
        <div class="reader-state">
          <p>Article not prepared.</p>
          {#if onRetry}
            <Button disabled={busy} onclick={() => void onRetry(item)} variant="secondary">
              Prepare article
            </Button>
          {/if}
        </div>
      {:else if loadError}
        <div class="reader-state reader-error">
          <StatusMessage tone="error">{loadError}</StatusMessage>
          <Button onclick={() => void loadContent()} variant="secondary">Try again</Button>
        </div>
      {:else if sanitizedHtml}
        <article bind:this={readerArticle} class="reader-content prose-content">
          {@html outline.html}
        </article>
      {:else if plainText}
        <article class="reader-content prose-content reader-plain-text">{plainText}</article>
      {:else}
        <Empty.Root class="reader-state">
          <Empty.Header>
            <Empty.Media variant="icon"><BookOpenCheck /></Empty.Media>
            <Empty.Title>No readable text</Empty.Title>
            <Empty.Description>This saved page did not include article text.</Empty.Description>
          </Empty.Header>
          {#if sourceUrl}
            <Empty.Content>
              <Button
                href={sourceUrl}
                rel="noopener noreferrer"
                target="_blank"
                variant="secondary"
              >
                Open source
                <ExternalLink data-icon="inline-end" />
              </Button>
            </Empty.Content>
          {/if}
        </Empty.Root>
      {/if}
    </div>
    {#if showOutline}
      <ReaderTableOfContents
        headings={outline.headings}
        article={readerArticle}
        scrollElement={readerScrollElement}
        titleElement={readerHeading}
      />
    {/if}
  </div>
</section>

<style>
  .library-reader {
    --reader-font-sans: var(--font-sans);
    --reader-font-mono: var(--font-mono);
    --prose-font-mono: var(--reader-font-mono);
    --prose-block-gap: 1.45em;
    --reader-font-size: 18px;
    --reader-line-height: 1.82;
    --reader-column-width: 800px;
    --reader-gutter: 32px;
    width: 100%;
    min-height: 100%;
    padding: 24px var(--reader-gutter) 88px;
    animation: reader-enter 180ms ease both;
  }

  .library-reader[data-reader-font='plex'] {
    --reader-font-sans: 'IBM Plex Sans Variable', 'Noto Sans SC', sans-serif;
  }

  .library-reader[data-reader-font='system'] {
    --reader-font-sans:
      ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans SC',
      'PingFang SC', 'Microsoft YaHei', sans-serif;
    --reader-font-mono: ui-monospace, 'SFMono-Regular', Consolas, monospace;
  }

  .library-reader[data-reader-size='small'] {
    --reader-font-size: 16px;
  }

  .library-reader[data-reader-size='large'] {
    --reader-font-size: 20px;
  }

  .library-reader[data-reader-line-height='compact'] {
    --reader-line-height: 1.62;
  }

  .library-reader[data-reader-line-height='spacious'] {
    --reader-line-height: 2;
  }

  .library-reader[data-reader-width='narrow'] {
    --reader-column-width: 680px;
  }

  .library-reader[data-reader-width='wide'] {
    --reader-column-width: 920px;
  }

  .reader-toolbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    width: min(100%, 1040px);
    min-height: 44px;
    gap: 24px;
    align-items: center;
    justify-content: space-between;
    margin: 0 auto 52px;
    background: var(--background);
    transition:
      opacity 160ms ease,
      transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
    will-change: transform;
  }

  .reader-toolbar.toolbar-hidden {
    opacity: 0;
    pointer-events: none;
    transform: translateY(calc(-100% - 4px));
  }

  .refresh-review-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    grid-column: 2;
    margin-top: 8px;
  }

  .reader-toolbar-label {
    color: var(--muted-foreground);
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .reader-actions {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .reader-column {
    min-width: 0;
    width: min(100%, var(--reader-column-width));
    margin-inline: auto;
    transition: width 180ms ease;
  }

  .reader-layout.has-outline {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 44px;
    gap: calc(var(--reader-gutter) + 8px);
    margin-inline-end: calc(8px - var(--reader-gutter));
  }

  .reader-content :global(h2[id]),
  .reader-content :global(h3[id]) {
    scroll-margin-top: 128px;
  }

  .reader-content :global(h2:focus-visible),
  .reader-content :global(h3:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: 4px;
  }

  .library-reader.preview {
    --reader-gutter: 28px;
    min-height: 0;
    padding: 20px var(--reader-gutter) 64px;
  }

  .library-reader.preview .reader-toolbar {
    top: -3px;
    width: 100%;
    padding: 3px 0 14px;
    margin-bottom: 30px;
    background: var(--background);
  }

  .library-reader.preview .reader-heading h1 {
    font-size: clamp(26px, 3vw, 38px);
  }

  .library-reader.preview .reader-excerpt-copy {
    font-size: 16px;
  }

  .reader-heading {
    padding-bottom: 34px;
  }

  .reader-source {
    margin-bottom: 12px;
    color: var(--primary);
    font-size: 11px;
    font-weight: 660;
    letter-spacing: 0.045em;
    text-transform: uppercase;
  }

  .reader-heading h1 {
    margin: 0;
    font-family: var(--reader-font-sans);
    font-size: clamp(32px, 4vw, 46px);
    font-weight: 650;
    line-height: 1.14;
    letter-spacing: -0.035em;
    overflow-wrap: anywhere;
  }

  .reader-heading h1:focus-visible {
    outline: none;
  }

  .reader-excerpt {
    max-width: 660px;
    margin: 24px 0 0;
    padding-inline-start: 16px;
    border-inline-start: 2px solid var(--border);
  }

  .reader-excerpt-label {
    margin: 0 0 8px;
    color: var(--muted-foreground);
    font-family: var(--reader-font-mono);
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.06em;
    line-height: 1.4;
    text-transform: uppercase;
  }

  .reader-excerpt-copy {
    margin: 0;
    color: var(--muted-foreground);
    font-family: var(--reader-font-sans);
    font-size: 17px;
    line-height: 1.65;
  }

  .reader-byline {
    display: flex;
    flex-wrap: wrap;
    gap: 5px 16px;
    margin: 18px 0 0;
    color: var(--muted-foreground);
    font-family: var(--reader-font-mono);
    font-size: 10px;
  }

  .reader-state {
    display: grid;
    min-height: 300px;
    align-content: center;
    justify-items: center;
    color: var(--muted-foreground);
    text-align: center;
  }

  .reader-state p {
    margin: 10px 0 0;
    font-size: 13px;
  }

  .reader-error :global(.status-message) {
    max-width: 440px;
    margin-bottom: 16px;
    text-align: left;
  }

  .reader-content {
    color: var(--foreground);
    font-family: var(--reader-font-sans);
    font-size: var(--reader-font-size);
    line-height: var(--reader-line-height);
    overflow-wrap: anywhere;
    transition:
      font-size 160ms ease,
      line-height 160ms ease;
  }

  .reader-plain-text {
    white-space: pre-wrap;
  }

  .reader-content :global(h1),
  .reader-content :global(h2),
  .reader-content :global(h3),
  .reader-content :global(h4),
  .reader-content :global(h5),
  .reader-content :global(h6) {
    margin: 1.8em 0 0.65em;
    font-family: var(--reader-font-sans);
    font-weight: 680;
    line-height: 1.3;
    letter-spacing: -0.018em;
  }

  .reader-content :global(h1) {
    font-size: 30px;
  }

  .reader-content :global(h2) {
    font-size: 25px;
  }

  .reader-content :global(h3) {
    font-size: 21px;
  }

  .reader-content :global(pre) {
    font-size: 13px;
    line-height: 1.65;
  }

  .reader-content :global(pre[data-language]::before) {
    content: attr(data-language);
    display: block;
    margin-bottom: 8px;
    color: var(--muted-foreground);
    font-size: 11px;
  }

  .reader-content :global(figcaption) {
    color: var(--muted-foreground);
    font-family: var(--reader-font-sans);
    font-size: 12px;
  }

  @keyframes reader-enter {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  @media (max-width: 767px) {
    .reader-layout.has-outline {
      grid-template-columns: minmax(0, 1fr);
      gap: 12px;
      margin-inline-end: 0;
    }

    .has-outline .reader-column {
      grid-row: 2;
    }

    .library-reader {
      padding: calc(12px + env(safe-area-inset-top)) calc(16px + env(safe-area-inset-right))
        calc(48px + env(safe-area-inset-bottom)) calc(16px + env(safe-area-inset-left));
    }

    .reader-toolbar {
      top: env(safe-area-inset-top);
      gap: 8px;
      margin-bottom: 34px;
    }

    .reader-actions {
      gap: 4px;
    }

    .reader-action-label {
      display: none;
    }

    .reader-actions :global([data-slot='button']) {
      min-width: 44px;
      padding-inline: 10px;
    }

    .reader-heading {
      padding-bottom: 27px;
    }

    .reader-heading h1 {
      font-size: clamp(30px, 10vw, 38px);
    }

    .reader-excerpt {
      padding-inline-start: 12px;
    }

    .reader-excerpt-copy {
      font-size: 16px;
    }

    .reader-content :global(pre) {
      font-size: 12px;
    }
  }

  @media (min-width: 768px) and (max-width: 1199px) {
    .library-reader:not(.preview) .reader-toolbar {
      top: calc(56px + env(safe-area-inset-top));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .library-reader,
    .reader-toolbar,
    .reader-column,
    .reader-content {
      transition-duration: 0.01ms;
      animation-duration: 0.01ms;
    }
  }
</style>
