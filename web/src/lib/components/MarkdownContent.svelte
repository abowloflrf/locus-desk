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

<div class="markdown-content prose-content">{@html html}</div>
