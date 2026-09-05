<script lang="ts">
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
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
  <Popover.Content align="end" class="w-[min(22rem,calc(100vw-2rem))]" sideOffset={8}>
    <Popover.Header>
      <Popover.Title>Reading preferences</Popover.Title>
      <Popover.Description>Saved in this browser for Library articles.</Popover.Description>
    </Popover.Header>

    <Field.Group class="gap-5">
      <Field.Field>
        <Field.Title id="reader-font-label">Typeface</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-font-label"
          class="w-full"
          onValueChange={setFontPreset}
          spacing={2}
          type="single"
          value={preferences.fontPreset}
          variant="outline"
        >
          {#each READER_FONT_PRESETS as option}
            <ToggleGroup.Item
              aria-label={`${option.label} typeface`}
              class="flex-auto px-2"
              data-font-option={option.value}
              value={option.value}
            >
              <span aria-hidden="true" class="font-preview">Aa</span>
              <span>{option.label}</span>
            </ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>

      <Field.Field>
        <Field.Title id="reader-size-label">Text size</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-size-label"
          class="w-full"
          onValueChange={setFontSize}
          spacing={2}
          type="single"
          value={preferences.fontSize}
          variant="outline"
        >
          {#each READER_FONT_SIZES as option}
            <ToggleGroup.Item
              aria-label={`${option.label} pixel text`}
              class="flex-1"
              value={option.value}
            >
              {option.label}
            </ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>

      <Field.Field>
        <Field.Title id="reader-spacing-label">Line spacing</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-spacing-label"
          class="w-full"
          onValueChange={setLineHeight}
          spacing={2}
          type="single"
          value={preferences.lineHeight}
          variant="outline"
        >
          {#each READER_LINE_HEIGHTS as option}
            <ToggleGroup.Item class="flex-1" value={option.value}>{option.label}</ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>

      <Field.Field>
        <Field.Title id="reader-width-label">Article width</Field.Title>
        <ToggleGroup.Root
          aria-labelledby="reader-width-label"
          class="w-full"
          onValueChange={setWidth}
          spacing={2}
          type="single"
          value={preferences.width}
          variant="outline"
        >
          {#each READER_WIDTHS as option}
            <ToggleGroup.Item
              aria-label={`${option.label} article width`}
              class="flex-1"
              value={option.value}
            >
              {option.label}
            </ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
      </Field.Field>
    </Field.Group>

    <div class="preference-footer">
      <Button onclick={() => onChange({ ...DEFAULT_READER_PREFERENCES })} size="sm" variant="ghost">
        <RotateCcw data-icon="inline-start" />
        Reset
      </Button>
    </div>
  </Popover.Content>
</Popover.Root>

<style>
  .preference-footer {
    display: flex;
    justify-content: flex-end;
  }

  :global([data-font-option]) {
    height: auto;
    min-height: 56px;
    flex-direction: column;
    gap: 1px;
  }

  .font-preview {
    font-size: 17px;
    font-weight: 600;
    line-height: 1;
  }

  :global([data-font-option='plex'] .font-preview) {
    font-family: 'IBM Plex Sans Variable', sans-serif;
  }

  :global([data-font-option='sans'] .font-preview) {
    font-family: var(--font-sans);
  }

  :global([data-font-option='system'] .font-preview) {
    font-family: ui-sans-serif, system-ui, sans-serif;
  }
</style>
