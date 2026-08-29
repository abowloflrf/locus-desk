<script lang="ts" module>
  import { type VariantProps, tv } from 'tailwind-variants';

  export const inputGroupTextareaVariants = tv({
    base: 'rounded-none border-0 bg-transparent py-2 shadow-none ring-0 focus-visible:ring-0 aria-invalid:ring-0 dark:bg-transparent flex-1 resize-none',
    variants: {
      tone: {
        default: '',
        secondary: 'text-muted-foreground focus-visible:text-foreground',
      },
    },
    defaultVariants: {
      tone: 'default',
    },
  });

  export type InputGroupTextareaTone = VariantProps<typeof inputGroupTextareaVariants>['tone'];
</script>

<script lang="ts">
  import { Textarea } from '$lib/components/ui/textarea/index.js';
  import { cn } from '$lib/utils.js';
  import type { ComponentProps } from 'svelte';

  let {
    ref = $bindable(null),
    value = $bindable(),
    tone = 'default',
    class: className,
    ...props
  }: ComponentProps<typeof Textarea> & {
    tone?: InputGroupTextareaTone;
  } = $props();
</script>

<Textarea
  bind:ref
  data-slot="input-group-control"
  class={cn(inputGroupTextareaVariants({ tone }), className)}
  bind:value
  {...props}
/>
