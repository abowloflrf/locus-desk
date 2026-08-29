<script lang="ts" module>
  import { type VariantProps, tv } from 'tailwind-variants';

  export const textareaVariants = tv({
    base: 'rounded-md border border-input bg-transparent px-2.5 py-2 text-base shadow-xs transition-[color,box-shadow] aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 md:text-sm dark:bg-input/30 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 flex field-sizing-content min-h-16 w-full outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50',
    variants: {
      variant: {
        default: 'focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50',
        quiet: 'focus-visible:border-input focus-visible:ring-1 focus-visible:ring-foreground/10',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  });

  export type TextareaVariant = VariantProps<typeof textareaVariants>['variant'];
</script>

<script lang="ts">
  import { cn, type WithElementRef, type WithoutChildren } from '$lib/utils.js';
  import type { HTMLTextareaAttributes } from 'svelte/elements';

  let {
    ref = $bindable(null),
    value = $bindable(),
    variant = 'default',
    class: className,
    'data-slot': dataSlot = 'textarea',
    ...restProps
  }: WithoutChildren<WithElementRef<HTMLTextareaAttributes>> & {
    variant?: TextareaVariant;
  } = $props();
</script>

<textarea
  bind:this={ref}
  data-slot={dataSlot}
  class={cn(textareaVariants({ variant }), className)}
  bind:value
  {...restProps}></textarea>
