<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';

  const inputGroupButtonVariants = tv({
    base: 'gap-2 text-sm flex items-center shadow-none',
    variants: {
      size: {
        xs: "relative h-6 gap-1 rounded-[calc(var(--radius)-5px)] px-1.5 [&>svg:not([class*='size-'])]:size-3.5 pointer-coarse:min-h-7 pointer-coarse:before:absolute pointer-coarse:before:-inset-y-2 pointer-coarse:before:inset-x-0",
        sm: 'relative h-8 gap-1 px-2.5 pointer-coarse:min-h-8 pointer-coarse:before:absolute pointer-coarse:before:-inset-y-1.5 pointer-coarse:before:inset-x-0',
        'icon-xs':
          'relative size-6 rounded-[calc(var(--radius)-5px)] p-0 has-[>svg]:p-0 pointer-coarse:size-7 pointer-coarse:min-h-7 pointer-coarse:before:absolute pointer-coarse:before:-inset-2',
        'icon-sm':
          'relative size-8 p-0 has-[>svg]:p-0 pointer-coarse:size-8 pointer-coarse:min-h-8 pointer-coarse:before:absolute pointer-coarse:before:-inset-1.5',
      },
    },
    defaultVariants: {
      size: 'xs',
    },
  });

  export type InputGroupButtonSize = VariantProps<typeof inputGroupButtonVariants>['size'];
</script>

<script lang="ts">
  import { Button } from '$lib/components/ui/button/index.js';
  import { cn } from '$lib/utils.js';
  import type { ComponentProps } from 'svelte';

  let {
    ref = $bindable(null),
    class: className,
    children,
    type = 'button',
    variant = 'ghost',
    size = 'xs',
    ...restProps
  }: Omit<ComponentProps<typeof Button>, 'href' | 'size'> & {
    size?: InputGroupButtonSize;
  } = $props();
</script>

<Button
  bind:ref
  {type}
  data-size={size}
  {variant}
  class={cn(inputGroupButtonVariants({ size }), className)}
  {...restProps}
>
  {@render children?.()}
</Button>
