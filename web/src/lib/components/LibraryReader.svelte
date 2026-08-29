<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import BookOpenCheck from '@lucide/svelte/icons/book-open-check';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Maximize2 from '@lucide/svelte/icons/maximize-2';
  import X from '@lucide/svelte/icons/x';
  import { onMount, tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import { getLibraryContent } from '../api/library';
  import type { LibraryContentResponse, LibraryItem } from '../api/types';
  import { safeLibrarySourceUrl, sanitizeLibraryHtml } from '../library-content';
  import { formatNoteTimestamp } from '../utils/date';
  import StatusMessage from './StatusMessage.svelte';
  import { Button } from './ui/button';
  import * as Empty from './ui/empty';
  import { Spinner } from './ui/spinner';

  let {
    actionStatus = null,
    busy = false,
    item,
    onBack,
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
  let readerHeading = $state<HTMLHeadingElement>();
  let contentController: AbortController | null = null;
  let contentRequestId = 0;
  let requestedContentKey = '';

  const sourceUrl = $derived(
    safeLibrarySourceUrl(item.canonicalUrl ?? item.normalizedUrl ?? item.originalUrl),
  );
  const sanitizedHtml = $derived(
    content ? sanitizeLibraryHtml(content.safeHtml, sourceUrl ?? item.originalUrl) : '',
  );
  const plainText = $derived(content?.plainText.trim() ?? '');

  onMount(() => {
    if (mode === 'full') void tick().then(() => readerHeading?.focus());

    return () => {
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
    if (
      mode === 'preview' ||
      event.defaultPrevented ||
      event.key !== 'Escape' ||
      document.querySelector('dialog[open]')
    ) {
      return;
    }
    event.preventDefault();
    void onBack();
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
    if (!item.publishedAt) return null;
    const date = new Date(item.publishedAt);
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

<svelte:window onkeydown={handleWindowKeydown} />

<section
  aria-busy={loading || item.processingStatus === 'PENDING' || busy}
  aria-labelledby="library-reader-title"
  class:preview={mode === 'preview'}
  class="library-reader"
>
  <header class="reader-toolbar">
    {#if mode === 'full'}
      <Button onclick={() => void onBack()} variant="ghost">
        <ArrowLeft data-icon="inline-start" />
        Back to Library
      </Button>
    {:else}
      <span class="reader-toolbar-label">Article preview</span>
    {/if}

    <div class="reader-actions">
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
      {#if mode === 'preview' && onExpand}
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

  <div class="reader-column">
    <header class="reader-heading">
      <p class="reader-source">{item.siteName || sourceHostname() || 'Saved article'}</p>
      <h1 bind:this={readerHeading} id="library-reader-title" tabindex="-1">{displayTitle()}</h1>
      {#if item.excerpt}<p class="reader-excerpt">{item.excerpt}</p>{/if}
      <p class="reader-byline">
        {#if item.author}<span>{item.author}</span>{/if}
        {#if publishedLabel()}<span>{publishedLabel()}</span>{/if}
        {#if content?.fetchedAt}
          <span>Saved {formatNoteTimestamp(content.fetchedAt, timeZone)}</span>
        {/if}
      </p>
    </header>

    {#if operationError}<StatusMessage tone="error">{operationError}</StatusMessage>{/if}
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
    {:else if loadError}
      <div class="reader-state reader-error">
        <StatusMessage tone="error">{loadError}</StatusMessage>
        <Button onclick={() => void loadContent()} variant="secondary">Try again</Button>
      </div>
    {:else if sanitizedHtml}
      <article class="reader-content">{@html sanitizedHtml}</article>
    {:else if plainText}
      <article class="reader-content reader-plain-text">{plainText}</article>
    {:else}
      <Empty.Root class="reader-state">
        <Empty.Header>
          <Empty.Media variant="icon"><BookOpenCheck /></Empty.Media>
          <Empty.Title>No readable text</Empty.Title>
          <Empty.Description>This saved page did not include article text.</Empty.Description>
        </Empty.Header>
        {#if sourceUrl}
          <Empty.Content>
            <Button href={sourceUrl} rel="noopener noreferrer" target="_blank" variant="secondary">
              Open source
              <ExternalLink data-icon="inline-end" />
            </Button>
          </Empty.Content>
        {/if}
      </Empty.Root>
    {/if}
  </div>
</section>

<style>
  .library-reader {
    width: 100%;
    min-height: 100%;
    padding: 24px 32px 88px;
    animation: reader-enter 180ms ease both;
  }

  .reader-toolbar {
    display: flex;
    width: min(100%, 1040px);
    min-height: 44px;
    gap: 24px;
    align-items: center;
    justify-content: space-between;
    margin: 0 auto 52px;
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
    width: min(100%, 720px);
    margin-inline: auto;
  }

  .library-reader.preview {
    min-height: 0;
    padding: 20px 28px 64px;
  }

  .library-reader.preview .reader-toolbar {
    position: sticky;
    top: -3px;
    z-index: 2;
    width: 100%;
    padding: 3px 0 14px;
    margin-bottom: 30px;
    background: var(--background);
  }

  .library-reader.preview .reader-column {
    width: min(100%, 680px);
  }

  .library-reader.preview .reader-heading h1 {
    font-size: clamp(26px, 3vw, 38px);
  }

  .library-reader.preview .reader-excerpt {
    font-size: 16px;
  }

  .library-reader.preview .reader-content {
    font-size: 16px;
    line-height: 1.76;
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
    font-family: var(--font-sans);
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
    margin: 20px 0 0;
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 18px;
    line-height: 1.65;
  }

  .reader-byline {
    display: flex;
    flex-wrap: wrap;
    gap: 5px 16px;
    margin: 18px 0 0;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
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
    font-family: var(--font-sans);
    font-size: 18px;
    line-height: 1.82;
    overflow-wrap: anywhere;
  }

  .reader-plain-text {
    white-space: pre-wrap;
  }

  .reader-content :global(p),
  .reader-content :global(blockquote),
  .reader-content :global(pre),
  .reader-content :global(ul),
  .reader-content :global(ol),
  .reader-content :global(table),
  .reader-content :global(figure) {
    margin: 0 0 1.45em;
  }

  .reader-content :global(h1),
  .reader-content :global(h2),
  .reader-content :global(h3),
  .reader-content :global(h4),
  .reader-content :global(h5),
  .reader-content :global(h6) {
    margin: 1.8em 0 0.65em;
    font-family: var(--font-sans);
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

  .reader-content :global(a) {
    color: var(--primary);
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
  }

  .reader-content :global(blockquote) {
    padding-left: 20px;
    color: var(--muted-foreground);
    border-left: 2px solid var(--primary);
    font-size: 1.02em;
  }

  .reader-content :global(pre) {
    max-width: 100%;
    padding: 16px;
    overflow: auto;
    background: var(--muted);
    border-radius: var(--radius-md);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.65;
  }

  .reader-content :global(:not(pre) > code) {
    padding: 2px 4px;
    background: var(--muted);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.82em;
  }

  .reader-content :global(table) {
    display: block;
    width: 100%;
    max-width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
    font-family: var(--font-sans);
    font-size: 13px;
    line-height: 1.55;
  }

  .reader-content :global(th),
  .reader-content :global(td) {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  .reader-content :global(hr) {
    width: 64px;
    margin: 44px auto;
    border: 0;
    border-top: 1px solid var(--border);
  }

  .reader-content :global(figcaption) {
    color: var(--muted-foreground);
    font-family: var(--font-sans);
    font-size: 12px;
  }

  @keyframes reader-enter {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  @media (max-width: 767px) {
    .library-reader {
      padding: calc(12px + env(safe-area-inset-top)) calc(16px + env(safe-area-inset-right))
        calc(48px + env(safe-area-inset-bottom)) calc(16px + env(safe-area-inset-left));
    }

    .reader-toolbar {
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
      font-size: 17px;
    }

    .reader-content {
      font-size: 17px;
      line-height: 1.78;
    }

    .reader-content :global(pre) {
      padding: 14px;
      font-size: 12px;
    }
  }
</style>
