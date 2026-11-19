<script lang="ts">
  import type { ListContinuationError, ListLoad } from '../utils/pagination';
  import StatusMessage from './StatusMessage.svelte';
  import { Button } from './ui/button';
  import { Spinner } from './ui/spinner';

  let {
    error,
    hasMore,
    loading,
    loadingMore,
    label,
    onLoad,
  }: {
    error: ListContinuationError | null;
    hasMore: boolean;
    loading: boolean;
    loadingMore: boolean;
    label: string;
    onLoad: (load: ListLoad, lastPage?: number) => Promise<boolean>;
  } = $props();

  function load(): void {
    if (loading || loadingMore) return;
    void onLoad(error?.load ?? 'append', error?.lastPage);
  }
</script>

{#if error}
  <StatusMessage tone="error">{error.message}</StatusMessage>
{/if}
{#if error || hasMore}
  <Button
    aria-disabled={loading || loadingMore}
    class="load-more aria-disabled:opacity-50"
    data-list-continuation
    onclick={load}
    variant="secondary"
  >
    {#if loadingMore || (loading && error)}<Spinner data-icon="inline-start" />{/if}
    {loadingMore || (loading && error)
      ? 'Loading…'
      : error
        ? error.load === 'append'
          ? 'Retry loading more'
          : 'Retry refreshing'
        : label}
  </Button>
{/if}
