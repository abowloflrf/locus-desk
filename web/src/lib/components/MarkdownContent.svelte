<script lang="ts">
  import '../vitesse-light.css';
  import { renderMarkdown, renderHighlightedMarkdown } from '../markdown';

  let { content }: { content: string } = $props();
  let highlighted = $state<{ source: string; html: string } | null>(null);
  let html = $derived(highlighted?.source === content ? highlighted.html : renderMarkdown(content));

  $effect(() => {
    const source = content;
    let active = true;
    void renderHighlightedMarkdown(source)
      .then((html) => {
        if (active) highlighted = { source, html };
      })
      .catch(() => {
        // The synchronous rendering remains readable if highlighting fails.
      });
    return () => {
      active = false;
    };
  });
</script>

<div class="markdown-content">{@html html}</div>

<style>
  .markdown-content {
    min-width: 0;
    color: var(--foreground);
    overflow-wrap: anywhere;
    font-size: 15px;
    line-height: 24px;
  }

  .markdown-content > :global(:first-child) {
    margin-top: 0;
  }

  .markdown-content > :global(:last-child) {
    margin-bottom: 0;
  }

  .markdown-content :global(h1),
  .markdown-content :global(h2),
  .markdown-content :global(h3),
  .markdown-content :global(h4) {
    margin: 1.3em 0 0.45em;
    line-height: 1.35;
  }

  .markdown-content :global(h1) {
    font-size: 21px;
  }

  .markdown-content :global(h2) {
    font-size: 18px;
  }

  .markdown-content :global(h3),
  .markdown-content :global(h4) {
    font-size: 16px;
  }

  .markdown-content :global(p),
  .markdown-content :global(ul),
  .markdown-content :global(ol),
  .markdown-content :global(blockquote),
  .markdown-content :global(pre) {
    margin: 0 0 0.7em;
  }

  .markdown-content :global(ul),
  .markdown-content :global(ol) {
    padding-left: 1.55em;
  }

  .markdown-content :global(ol) {
    list-style: decimal outside;
  }

  .markdown-content :global(ul) {
    list-style: disc outside;
  }

  .markdown-content :global(ul ul) {
    list-style-type: circle;
  }

  .markdown-content :global(ul ul ul) {
    list-style-type: square;
  }

  .markdown-content :global(li > ul),
  .markdown-content :global(li > ol) {
    margin-block: 4px;
  }

  .markdown-content :global(li + li) {
    margin-top: 0.16em;
  }

  .markdown-content :global(li:has(> input[type='checkbox'])) {
    list-style: none;
  }

  .markdown-content :global(input[type='checkbox']) {
    width: 15px;
    height: 15px;
    min-height: 0;
    margin: 0 7px 0 0;
    accent-color: var(--primary);
    vertical-align: -2px;
  }

  .markdown-content :global(blockquote) {
    padding-left: 14px;
    color: var(--muted-foreground);
    border-left: 2px solid var(--primary);
  }

  .markdown-content :global(code) {
    padding: 0.12em 0.34em;
    background: var(--muted);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.86em;
  }

  .markdown-content :global(pre) {
    max-width: 100%;
    padding: 13px 15px;
    overflow-x: auto;
    scrollbar-width: none;
    color: var(--code-foreground);
    background: var(--code-background);
    border: 1px solid var(--border);
    border-radius: 8px;
    line-height: 1.55;
  }

  .markdown-content :global(pre::-webkit-scrollbar) {
    display: none;
  }

  .markdown-content :global(pre > span) {
    display: block;
    margin-bottom: 8px;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 16px;
    white-space: normal;
    overflow-wrap: anywhere;
    user-select: none;
  }

  .markdown-content :global(pre code) {
    padding: 0;
    background: transparent;
  }

  .markdown-content :global(a) {
    color: var(--primary);
    text-decoration: underline;
    text-decoration-color: color-mix(in oklch, var(--primary), transparent 55%);
    text-underline-offset: 2px;
  }

  .markdown-content :global(table) {
    display: block;
    max-width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
    margin-block: 12px;
    font-size: 13px;
  }

  .markdown-content :global(th),
  .markdown-content :global(td) {
    padding: 8px 12px;
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  .markdown-content :global(th) {
    background: var(--muted);
    font-weight: 600;
  }

  .markdown-content :global(h5),
  .markdown-content :global(h6) {
    margin-block: 1em 0.4em;
    font-size: 14px;
    font-weight: 600;
  }

  .markdown-content :global(hr) {
    margin: 1.4em 0;
    border: 0;
    border-top: 1px solid var(--border);
  }

  @media (max-width: 767px) {
    .markdown-content {
      font-size: 16px;
    }
  }
</style>
