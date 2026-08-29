<script lang="ts" module>
  import { type VariantProps, tv } from 'tailwind-variants';

  export const inputGroupInputVariants = tv({
    base: 'rounded-none border-0 bg-transparent shadow-none ring-0 focus-visible:ring-0 aria-invalid:ring-0 dark:bg-transparent flex-1',
    variants: {
      emphasis: {
        default: '',
        title: 'font-semibold text-foreground',
      },
    },
    defaultVariants: {
      emphasis: 'default',
    },
  });

  export type InputGroupInputEmphasis = VariantProps<typeof inputGroupInputVariants>['emphasis'];
</script>

<script lang="ts">
  import { Input } from '$lib/components/ui/input/index.js';
  import { cn } from '$lib/utils.js';
  import type { ComponentProps } from 'svelte';

  let {
    ref = $bindable(null),
    value = $bindable(),
    emphasis = 'default',
    class: className,
    ...props
  }: ComponentProps<typeof Input> & {
    emphasis?: InputGroupInputEmphasis;
  } = $props();
</script>

<Input
  bind:ref
  data-slot="input-group-control"
  class={cn(inputGroupInputVariants({ emphasis }), className)}
  bind:value
  {...props}
/>
