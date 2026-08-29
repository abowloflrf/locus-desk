<script lang="ts" module>
  import { getContext, setContext } from 'svelte';
  import { toggleVariants } from '$lib/components/ui/toggle/index.js';
  import type { VariantProps } from 'tailwind-variants';

  type ToggleVariants = VariantProps<typeof toggleVariants>;

  interface ToggleGroupContext extends ToggleVariants {
    spacing?: number;
    orientation?: 'horizontal' | 'vertical';
  }

  export function setToggleGroupCtx(props: ToggleGroupContext) {
    setContext('toggleGroup', props);
  }

  export function getToggleGroupCtx() {
    return getContext<Required<ToggleGroupContext>>('toggleGroup');
  }
</script>

<script lang="ts">
  import { ToggleGroup as ToggleGroupPrimitive } from 'bits-ui';
  import { cn } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    value = $bindable(),
    class: className,
    size = 'default',
    spacing = 0,
    orientation = 'horizontal',
    variant = 'default',
    ...restProps
  }: ToggleGroupPrimitive.RootProps &
    ToggleVariants & {
      spacing?: number;
      orientation?: 'horizontal' | 'vertical';
    } = $props();

  setToggleGroupCtx({
    get variant() {
      return variant;
    },
    get size() {
      return size;
    },
    get spacing() {
      return spacing;
    },
    get orientation() {
      return orientation;
    },
  });
</script>

<!--
Discriminated Unions + Destructing (required for bindable) do not
get along, so we shut typescript up by casting `value` to `never`.
-->
<ToggleGroupPrimitive.Root
  bind:value={value as never}
  bind:ref
  {orientation}
  data-slot="toggle-group"
  data-value={typeof value === 'string' ? value : undefined}
  data-variant={variant}
  data-size={size}
  data-spacing={spacing}
  style={`--gap: ${spacing}`}
  class={cn(
    'rounded-md data-[spacing=0]:data-[variant=outline]:shadow-xs group/toggle-group flex w-fit flex-row items-center gap-[--spacing(var(--gap))] data-vertical:flex-col data-vertical:items-stretch',
    className,
  )}
  {...restProps}
/>

<style>
  :global([data-slot='toggle-group'][data-variant='workspace']) {
    position: relative;
    isolation: isolate;
    gap: 2px;
    padding: 3px;
    background: color-mix(in oklch, var(--popover), transparent 4%);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 1px 2px color-mix(in oklch, var(--foreground), transparent 94%);
  }

  :global([data-slot='toggle-group'][data-variant='workspace']::before) {
    position: absolute;
    top: 3px;
    left: 3px;
    z-index: 0;
    width: 40px;
    height: 40px;
    background: var(--accent);
    border-radius: var(--radius-md);
    content: '';
    transition: transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  :global([data-slot='toggle-group'][data-variant='workspace'][data-value='split']::before) {
    transform: translateX(42px);
  }

  :global([data-slot='toggle-group'][data-variant='workspace'][data-value='todo']::before) {
    transform: translateX(84px);
  }

  :global([data-slot='toggle-group'][data-variant='workspace'] [data-slot='toggle-group-item']) {
    position: relative;
    z-index: 1;
    color: var(--muted-foreground);
    background: transparent;
  }

  :global(
    [data-slot='toggle-group'][data-variant='workspace'] [data-slot='toggle-group-item']:hover
  ) {
    color: var(--foreground);
    background: transparent;
  }

  :global(
    [data-slot='toggle-group'][data-variant='workspace']
      [data-slot='toggle-group-item'][data-state='on']
  ) {
    color: var(--primary);
    background: transparent;
  }

  :global([data-slot='toggle-group'][data-variant='workspace'] svg) {
    width: 22px;
    height: 18px;
    stroke-width: 1.5;
  }

  @media (prefers-reduced-motion: reduce) {
    :global([data-slot='toggle-group'][data-variant='workspace']::before) {
      transition-duration: 0.01ms;
    }
  }
</style>
