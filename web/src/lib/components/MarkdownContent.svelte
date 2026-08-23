<script lang="ts">
  import { renderMarkdown } from '../markdown';

  let { content }: { content: string } = $props();
  let html = $derived(renderMarkdown(content));
</script>

<div class="markdown-content">{@html html}</div>

<style>
  .markdown-content {
    min-width: 0;
    color: var(--color-text);
    overflow-wrap: anywhere;
    font-size: 15px;
    line-height: 26px;
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
    margin: 0 0 0.8em;
  }

  .markdown-content :global(ul),
  .markdown-content :global(ol) {
    padding-left: 1.55em;
  }

  .markdown-content :global(li + li) {
    margin-top: 0.16em;
  }

  .markdown-content :global(ul:has(> li > input[type='checkbox'])) {
    padding-left: 0;
    list-style: none;
  }

  .markdown-content :global(input[type='checkbox']) {
    width: 15px;
    height: 15px;
    min-height: 0;
    margin: 0 7px 0 0;
    accent-color: var(--color-accent);
    vertical-align: -2px;
  }

  .markdown-content :global(blockquote) {
    padding-left: 14px;
    color: var(--color-text-muted);
    border-left: 2px solid var(--color-accent);
  }

  .markdown-content :global(code) {
    padding: 0.12em 0.34em;
    background: var(--color-surface-muted);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.86em;
  }

  .markdown-content :global(pre) {
    max-width: 100%;
    padding: 13px 15px;
    overflow-x: auto;
    color: var(--color-text);
    background: var(--color-surface-muted);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    line-height: 1.55;
  }

  .markdown-content :global(pre code) {
    padding: 0;
    background: transparent;
  }

  .markdown-content :global(a) {
    color: var(--color-accent-hover);
    text-decoration: underline;
    text-decoration-color: color-mix(in oklch, var(--color-accent), transparent 55%);
    text-underline-offset: 2px;
  }

  .markdown-content :global(hr) {
    margin: 1.4em 0;
    border: 0;
    border-top: 1px solid var(--color-border);
  }
</style>
