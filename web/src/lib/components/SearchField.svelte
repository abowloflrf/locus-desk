<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import { Input } from './ui/input';
  import * as Kbd from './ui/kbd';
  import { cn } from '../utils';

  let {
    label,
    placeholder,
    value,
    oninput,
    ref = $bindable(null),
    shortcut,
    fullWidth = false,
  }: {
    label: string;
    placeholder: string;
    value: string;
    oninput: (event: Event) => void;
    ref?: HTMLInputElement | null;
    shortcut?: string;
    fullWidth?: boolean;
  } = $props();
</script>

<label class={cn('search-field', fullWidth && 'search-field-wide')}>
  <Search
    aria-hidden="true"
    class="pointer-events-none absolute left-3 size-4 text-muted-foreground"
  />
  <span class="sr-only">{label}</span>
  <Input
    class={cn('pl-9', shortcut && 'pr-12')}
    autocomplete="off"
    bind:ref
    {oninput}
    {placeholder}
    type="search"
    {value}
    variant="flat"
  />
  {#if shortcut}<Kbd.Root class="pointer-events-none absolute right-3">{shortcut}</Kbd.Root>{/if}
</label>
