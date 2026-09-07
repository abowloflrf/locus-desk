<script lang="ts">
  import { onMount } from 'svelte';
  import { createSummaryReveal } from '../summary-markdown';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { getArticleSummary, generateArticleSummary, type ArticleSummary } from '../api/summaries';
  import { errorMessage } from '../api/client';
  import { Button } from './ui/button';
  import { Spinner } from './ui/spinner';
  import StatusMessage from './StatusMessage.svelte';

  let { uid, contentVersion }: { uid: string; contentVersion: number } = $props();
  const id = $props.id();
  const controller = new AbortController();
  let summary = $state<ArticleSummary | null>(null);
  let loading = $state(true);
  let loaded = $state(false);
  let generating = $state(false);
  let expanded = $state(true);
  let error = $state('');
  let status = $state('');
  let revealProgress = $state<number | null>(null);
  const rendered = $derived(createSummaryReveal(summary?.content ?? ''));
  const visibleHtml = $derived(
    revealProgress === null ? rendered.html : rendered.frame(revealProgress),
  );
  let revealing = $state(false);
  let animationFrame = 0;

  function reveal() {
    cancelAnimationFrame(animationFrame);
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
      revealProgress = null;
      status = 'AI summary generated.';
      return;
    }
    const duration = Math.min(2200, Math.max(220, rendered.textLength * 4));
    const started = performance.now();
    revealing = true;
    revealProgress = 0;
    function frame(now: number) {
      const progress = Math.max(0, Math.min(1, (now - started) / duration));
      revealProgress = progress;
      if (progress < 1) {
        animationFrame = requestAnimationFrame(frame);
      } else {
        revealing = false;
        revealProgress = null;
        status = 'AI summary generated.';
      }
    }
    animationFrame = requestAnimationFrame(frame);
  }

  onMount(() => {
    void load();
    return () => {
      controller.abort();
      cancelAnimationFrame(animationFrame);
    };
  });

  async function load() {
    loading = true;
    error = '';
    try {
      summary = await getArticleSummary(uid, controller.signal);
      loaded = true;
    } catch (cause) {
      if (!controller.signal.aborted) error = errorMessage(cause, 'Unable to load the AI summary.');
    } finally {
      loading = false;
    }
  }

  async function generate() {
    if (generating || revealing || !loaded) return;
    generating = true;
    error = '';
    status = '';
    try {
      const result = await generateArticleSummary(uid, controller.signal);
      if (controller.signal.aborted) return;
      summary = result;
      reveal();
    } catch (cause) {
      if (!controller.signal.aborted) error = errorMessage(cause, 'Unable to generate a summary.');
    } finally {
      generating = false;
    }
  }
</script>

<aside class="ai-summary" aria-labelledby={`${id}-title`}>
  <div class="summary-header">
    <div class="summary-label">
      <Sparkles size={16} aria-hidden="true" />
      <h2 id={`${id}-title`}>AI summary</h2>
      {#if summary}<span class="summary-language"
          >{summary.language === 'zh-CN' ? '中文' : 'English'}</span
        >{/if}
    </div>
    <Button
      variant="ghost"
      size="icon"
      aria-label={expanded ? 'Collapse AI summary' : 'Expand AI summary'}
      aria-expanded={expanded}
      aria-controls={`${id}-body`}
      onclick={() => (expanded = !expanded)}
    >
      {#if expanded}<ChevronUp />{:else}<ChevronDown />{/if}
    </Button>
  </div>
  <div id={`${id}-body`} hidden={!expanded}>
    <p class="summary-description">AI-generated overview · separate from the original article</p>
    {#if summary}
      {#if summary.contentVersion !== contentVersion}
        <StatusMessage tone="warning"
          >The article has changed. Regenerate to summarize the current version.</StatusMessage
        >
      {/if}
      <div class="summary-text" lang={summary.language}>
        <div class="summary-markdown" aria-hidden={revealing ? 'true' : undefined}>
          {@html visibleHtml}
        </div>
        {#if revealing}<div class="sr-only select-none">{@html rendered.html}</div>{/if}
      </div>
    {:else if !loading && loaded}
      <p class="summary-description">
        Generate a summary in your Settings language. Runs only when you click.
      </p>
    {/if}
    {#if loading}<p role="status" class="summary-progress"><Spinner /> Loading summary…</p>{/if}
    {#if error}<StatusMessage tone="error">{error}</StatusMessage>{/if}
    {#if !loading && !loaded}<Button
        variant="outline"
        class="mt-3 min-h-11"
        onclick={() => void load()}>Retry summary</Button
      >{/if}
    {#if loaded}
      <Button
        variant="outline"
        class="mt-3 min-h-11"
        disabled={generating || revealing}
        onclick={() => void generate()}
      >
        {#if generating}<Spinner data-icon="inline-start" />{:else if summary}<RefreshCw
            data-icon="inline-start"
          />{:else}<Sparkles data-icon="inline-start" />{/if}
        {generating ? 'Generating…' : summary ? 'Regenerate' : 'Generate summary'}
      </Button>
    {/if}
  </div>
  <span role="status" class="sr-only">{generating ? 'Generating AI summary…' : status}</span>
</aside>

<style>
  .ai-summary {
    margin-bottom: 32px;
    padding: 16px 20px;
    border-radius: 12px;
    background: var(--muted);
    color: var(--foreground);
    font-family: var(--font-sans);
  }
  .summary-header,
  .summary-label {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .summary-header {
    justify-content: space-between;
  }
  .summary-label {
    flex-wrap: wrap;
  }
  h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 650;
  }
  .summary-language {
    font-size: 11px;
    color: var(--muted-foreground);
  }
  .summary-description {
    margin: 4px 0 0;
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.7;
  }
  .summary-text {
    margin: 16px 0 0;
    font-size: 14px;
    line-height: 1.8;
    overflow-wrap: anywhere;
  }
  .summary-markdown :global(p) {
    margin: 0;
    white-space: pre-wrap;
  }
  .summary-markdown :global(ul),
  .summary-markdown :global(ol) {
    margin: 0;
    padding-inline-start: 24px;
  }
  .summary-markdown :global(ul) {
    list-style-type: disc;
  }
  .summary-markdown :global(ol) {
    list-style-type: decimal;
  }
  .summary-markdown :global(li + li) {
    margin-top: 4px;
  }
  .summary-markdown :global(p + p),
  .summary-markdown :global(p + ul),
  .summary-markdown :global(p + ol),
  .summary-markdown :global(ul + p),
  .summary-markdown :global(ol + p) {
    margin-top: 12px;
  }
  .summary-markdown :global(strong) {
    font-weight: 650;
  }
  .summary-markdown :global(code) {
    padding: 2px 4px;
    border-radius: 4px;
    background: var(--accent);
    font-family: var(--font-mono);
    font-size: 0.9em;
    white-space: pre-wrap;
  }
  .summary-progress {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    margin-top: 12px;
  }
  @media (max-width: 767px) {
    .ai-summary {
      padding: 12px 16px;
    }
    .summary-header :global(button) {
      min-width: 44px;
      min-height: 44px;
    }
  }
</style>
