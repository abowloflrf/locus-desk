<script lang="ts">
  import * as Alert from '$lib/components/ui/alert';
  import { cn } from '$lib/utils';

  let {
    tone = 'neutral',
    children,
  }: {
    tone?: 'neutral' | 'error' | 'success';
    children: import('svelte').Snippet;
  } = $props();
</script>

<Alert.Root
  aria-live={tone === 'error' ? 'assertive' : 'polite'}
  class={cn('status-message my-3', `status-${tone}`)}
  role={tone === 'error' ? 'alert' : 'status'}
  variant={tone === 'error' ? 'destructive' : 'default'}
>
  <Alert.Description>{@render children()}</Alert.Description>
</Alert.Root>
