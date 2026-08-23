<script lang="ts">
  let {
    tone = 'neutral',
    children,
  }: {
    tone?: 'neutral' | 'error' | 'success';
    children: import('svelte').Snippet;
  } = $props();
</script>

<div
  aria-live={tone === 'error' ? 'assertive' : 'polite'}
  class:status-error={tone === 'error'}
  class="status-message"
>
  {@render children()}
</div>

<style>
  .status-message {
    padding: 10px 12px;
    margin: 12px 0;
    color: var(--color-text-muted);
    background: var(--color-surface-muted);
    border-left: 3px solid var(--color-border);
    border-radius: 4px;
    font-size: 12px;
  }

  .status-message.status-error {
    color: var(--color-danger);
    background: var(--color-danger-soft);
    border-left-color: var(--color-danger);
  }
</style>
