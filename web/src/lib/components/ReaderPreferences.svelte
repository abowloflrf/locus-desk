<script lang="ts">
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import TypeIcon from '@lucide/svelte/icons/type';

  import {
    DEFAULT_READER_PREFERENCES,
    READER_FONT_PRESETS,
    READER_FONT_SIZES,
    READER_LINE_HEIGHTS,
    READER_WIDTHS,
    type ReaderFontPreset,
    type ReaderFontSize,
    type ReaderLineHeight,
    type ReaderPreferences,
    type ReaderWidth,
  } from '../reader-preferences';
  import { Button, buttonVariants } from './ui/button';
  import * as DropdownMenu from './ui/dropdown-menu';
  import * as Field from './ui/field';
  import * as Popover from './ui/popover';
  import * as ToggleGroup from './ui/toggle-group';

  let {
    onChange,
    preferences,
  }: {
    onChange: (preferences: ReaderPreferences) => void;
    preferences: ReaderPreferences;
  } = $props();

  const selectedFont = $derived(
    READER_FONT_PRESETS.find((option) => option.value === preferences.fontPreset) ??
      READER_FONT_PRESETS[0],
  );

  function setFontPreset(value: string): void {
    if (!READER_FONT_PRESETS.some((option) => option.value === value)) return;
    update({ fontPreset: value as ReaderFontPreset });
  }

  function setFontSize(value: string): void {
    if (!READER_FONT_SIZES.some((option) => option.value === value)) return;
    update({ fontSize: value as ReaderFontSize });
  }

  function setLineHeight(value: string): void {
    if (!READER_LINE_HEIGHTS.some((option) => option.value === value)) return;
    update({ lineHeight: value as ReaderLineHeight });
  }

  function setWidth(value: string): void {
    if (!READER_WIDTHS.some((option) => option.value === value)) return;
    update({ width: value as ReaderWidth });
  }

  function update(change: Partial<ReaderPreferences>): void {
    onChange({ ...preferences, ...change });
  }
</script>

<Popover.Root>
  <Popover.Trigger
    aria-label="Reading preferences"
    class={buttonVariants({ size: 'icon', variant: 'ghost' })}
    title="Reading preferences"
  >
    <TypeIcon />
  </Popover.Trigger>
  <Popover.Content
    align="end"
    variant="menu"
    class="max-h-[min(calc(100dvh-2rem),var(--bits-popover-content-available-height))] w-[min(20rem,calc(100vw-2rem))] gap-3 overflow-y-auto p-3"
    sideOffset={8}
  >
    <div class="flex items-center justify-between gap-2">
      <Popover.Title>Reading preferences</Popover.Title>
      <Button
        onclick={() => onChange({ ...DEFAULT_READER_PREFERENCES })}
        size="sm"
        variant="ghost"
        class="max-md:min-h-11 pointer-coarse:min-h-11"
      >
        <RotateCcw data-icon="inline-start" />
        Reset
      </Button>
    </div>

    <Field.Group class="gap-2">
      <Field.Field orientation="horizontal" class="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-2">
        <Field.Title id="reader-font-label">Font</Field.Title>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button
                {...props}
                id="reader-font-trigger"
                aria-labelledby="reader-font-label reader-font-value"
                variant="outline"
                size="sm"
                class="w-full min-w-0 justify-between max-md:min-h-11 pointer-coarse:min-h-11"
              >
                <span id="reader-font-value" class="truncate">{selectedFont.label}</span>
                <ChevronDown data-icon="inline-end" />
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="w-56" sideOffset={4}>
            <DropdownMenu.RadioGroup
              value={preferences.fontPreset}
              onValueChange={setFontPreset}
              aria-labelledby="reader-font-label"
            >
              {#each READER_FONT_PRESETS as option}
                <DropdownMenu.RadioItem
                  value={option.value}
                  aria-label={`${option.label} typeface`}
                  class="min-h-8 max-md:min-h-11 pointer-coarse:min-h-11"
                >
                  <span style:font-family={option.family}>{option.label}</span>
                </DropdownMenu.RadioItem>
              {/each}
            </DropdownMenu.RadioGroup>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </Field.Field>

      <Field.Field orientation="horizontal" class="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-2">
        <Field.Title id="reader-size-label">Size</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-size-label"
          class="w-full"
          onValueChange={setFontSize}
          size="sm"
          type="single"
          value={preferences.fontSize}
          variant="outline"
        >
          {#each READER_FONT_SIZES as option}
            <ToggleGroup.Item
              aria-label={`${option.label} pixel text`}
              class="min-w-0 flex-1 max-md:min-h-11 pointer-coarse:min-h-11"
              value={option.value}
            >
              {option.label}
            </ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>

      <Field.Field orientation="horizontal" class="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-2">
        <Field.Title id="reader-spacing-label">Spacing</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-spacing-label"
          class="w-full"
          onValueChange={setLineHeight}
          size="sm"
          type="single"
          value={preferences.lineHeight}
          variant="outline"
        >
          {#each READER_LINE_HEIGHTS as option}
            <ToggleGroup.Item
              class="min-w-0 flex-1 max-md:min-h-11 pointer-coarse:min-h-11"
              value={option.value}
            >
              {option.label}
            </ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>

      <Field.Field orientation="horizontal" class="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-2">
        <Field.Title id="reader-width-label">Width</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-width-label"
          class="w-full"
          onValueChange={setWidth}
          size="sm"
          type="single"
          value={preferences.width}
          variant="outline"
        >
          {#each READER_WIDTHS as option}
            <ToggleGroup.Item
              aria-label={`${option.label} article width`}
              class="min-w-0 flex-1 max-md:min-h-11 pointer-coarse:min-h-11"
              value={option.value}
            >
              {option.label}
            </ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>
    </Field.Group>
  </Popover.Content>
</Popover.Root>
