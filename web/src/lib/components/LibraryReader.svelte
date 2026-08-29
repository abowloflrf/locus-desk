<script lang="ts">
  import { onMount, tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import { getLibraryContent } from '../api/library';
  import type { LibraryContentResponse, LibraryItem } from '../api/types';
  import { safeLibrarySourceUrl, sanitizeLibraryHtml } from '../library-content';
  import { formatNoteTimestamp } from '../utils/date';
  import Icon from './Icon.svelte';
  import StatusMessage from './StatusMessage.svelte';

  let {
    actionStatus = null,
    busy = false,
    item,
    onBack,
    onToggleRead,
    operationError = null,
    timeZone,
  }: {
    actionStatus?: string | null;
    busy?: boolean;
    item: LibraryItem;
    onBack: () => void | Promise<void>;
    onToggleRead: (item: LibraryItem) => void | Promise<void>;
    operationError?: string | null;
    timeZone: string;
  } = $props();

  let content = $state<LibraryContentResponse | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let loadStatus = $state('');
  let readerHeading = $state<HTMLHeadingElement>();
  let contentController: AbortController | null = null;
  let contentRequestId = 0;

  const sourceUrl = $derived(
    safeLibrarySourceUrl(item.canonicalUrl ?? item.normalizedUrl ?? item.originalUrl),
  );
  const sanitizedHtml = $derived(
    content ? sanitizeLibraryHtml(content.safeHtml, sourceUrl ?? item.originalUrl) : '',
  );
  const plainText = $derived(content?.plainText.trim() ?? '');

  onMount(() => {
    void loadContent();
    void tick().then(() => readerHeading?.focus());

    return () => {
      contentRequestId += 1;
      contentController?.abort();
      contentController = null;
    };
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

<section aria-busy={loading} aria-labelledby="library-reader-title" class="library-reader">
  <header class="reader-toolbar">
    <button class="reader-back" onclick={() => void onBack()} type="button">
      <span aria-hidden="true">←</span>
      Back to Library
    </button>

    <div class="reader-actions">
      {#if sourceUrl}
        <a href={sourceUrl} rel="noopener noreferrer" target="_blank">
          {sourceHostname() || 'Open source'}
          <span aria-hidden="true">↗</span>
        </a>
      {/if}
      <button
        aria-label={item.readAt ? 'Mark as unread' : 'Mark as read'}
        aria-pressed={Boolean(item.readAt)}
        class:active={Boolean(item.readAt)}
        class="reader-read-state"
        disabled={busy}
        onclick={() => void onToggleRead(item)}
        type="button"
      >
        <Icon name="reader" size={16} />
        {item.readAt ? 'Read' : 'Mark read'}
      </button>
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
        <span class="reader-state-mark" aria-hidden="true"></span>
        <p>Opening saved article…</p>
      </div>
    {:else if loadError}
      <div class="reader-state reader-error">
        <StatusMessage tone="error">{loadError}</StatusMessage>
        <button class="button secondary" onclick={() => void loadContent()} type="button"
          >Try again</button
        >
      </div>
    {:else if sanitizedHtml}
      <article class="reader-content">{@html sanitizedHtml}</article>
    {:else if plainText}
      <article class="reader-content reader-plain-text">{plainText}</article>
    {:else}
      <div class="reader-state reader-empty">
        <Icon name="reader" size={22} />
        <h2>No readable text</h2>
        <p>This saved page did not include article text.</p>
        {#if sourceUrl}
          <a class="button secondary" href={sourceUrl} rel="noopener noreferrer" target="_blank"
            >Open source</a
          >
        {/if}
      </div>
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

  .reader-back,
  .reader-read-state {
    display: inline-flex;
    min-height: 44px;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    font-size: 12px;
    font-weight: 620;
  }

  .reader-back:hover:not(:disabled),
  .reader-read-state:hover:not(:disabled),
  .reader-read-state.active {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
  }

  .reader-actions {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .reader-actions > a {
    display: inline-flex;
    min-height: 44px;
    gap: 5px;
    align-items: center;
    padding: 6px 8px;
    color: var(--color-text-muted);
    border-radius: var(--radius-control);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .reader-actions > a:hover {
    color: var(--color-accent-hover);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .reader-column {
    width: min(100%, 720px);
    margin-inline: auto;
  }

  .reader-heading {
    padding-bottom: 34px;
  }

  .reader-source {
    margin-bottom: 12px;
    color: var(--color-accent-hover);
    font-size: 11px;
    font-weight: 660;
    letter-spacing: 0.045em;
    text-transform: uppercase;
  }

  .reader-heading h1 {
    margin: 0;
    font-family: var(--font-ui);
    font-size: clamp(32px, 4vw, 46px);
    font-weight: 650;
    line-height: 1.14;
    letter-spacing: -0.035em;
    overflow-wrap: anywhere;
  }

  .reader-excerpt {
    max-width: 660px;
    margin: 20px 0 0;
    color: var(--color-text-muted);
    font-family: var(--font-ui);
    font-size: 18px;
    line-height: 1.65;
  }

  .reader-byline {
    display: flex;
    flex-wrap: wrap;
    gap: 5px 16px;
    margin: 18px 0 0;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .reader-state {
    display: grid;
    min-height: 300px;
    align-content: center;
    justify-items: center;
    color: var(--color-text-muted);
    text-align: center;
  }

  .reader-state p {
    margin: 10px 0 0;
    font-size: 13px;
  }

  .reader-state-mark {
    width: 18px;
    height: 2px;
    background: var(--color-accent);
    border-radius: 2px;
  }

  .reader-error :global(.status-message) {
    max-width: 440px;
    margin-bottom: 16px;
    text-align: left;
  }

  .reader-empty h2 {
    margin: 14px 0 0;
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 21px;
  }

  .reader-empty .button {
    margin-top: 18px;
  }

  .reader-content {
    color: var(--color-text);
    font-family: var(--font-ui);
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
    font-family: var(--font-ui);
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
    color: var(--color-accent-hover);
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
  }

  .reader-content :global(blockquote) {
    padding-left: 20px;
    color: var(--color-text-muted);
    border-left: 2px solid var(--color-accent);
    font-size: 1.02em;
  }

  .reader-content :global(pre) {
    max-width: 100%;
    padding: 16px;
    overflow: auto;
    background: var(--color-surface-muted);
    border-radius: var(--radius-control);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.65;
  }

  .reader-content :global(:not(pre) > code) {
    padding: 2px 4px;
    background: var(--color-surface-muted);
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
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 1.55;
  }

  .reader-content :global(th),
  .reader-content :global(td) {
    padding: 8px 10px;
    border-bottom: 1px solid var(--color-border);
    text-align: left;
    vertical-align: top;
  }

  .reader-content :global(hr) {
    width: 64px;
    margin: 44px auto;
    border: 0;
    border-top: 1px solid var(--color-border);
  }

  .reader-content :global(figcaption) {
    color: var(--color-text-muted);
    font-family: var(--font-ui);
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
      padding: 12px 16px 72px;
    }

    .reader-toolbar {
      gap: 8px;
      margin-bottom: 34px;
    }

    .reader-back,
    .reader-read-state,
    .reader-actions > a {
      min-width: 44px;
      min-height: 44px;
    }

    .reader-actions > a {
      width: 44px;
      padding: 0;
      justify-content: center;
      font-size: 0;
    }

    .reader-actions > a span {
      font-size: 16px;
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
