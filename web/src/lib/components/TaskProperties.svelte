<script lang="ts">
  import { parseDate } from '@internationalized/date';
  import CalendarOff from '@lucide/svelte/icons/calendar-off';
  import CalendarDays from '@lucide/svelte/icons/calendar-days';
  import CalendarRange from '@lucide/svelte/icons/calendar-range';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import Flag from '@lucide/svelte/icons/flag';
  import Sun from '@lucide/svelte/icons/sun';
  import Sunrise from '@lucide/svelte/icons/sunrise';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import { tick } from 'svelte';
  import type { TaskPriority, UpdateTaskRequest } from '../api/types';
  import { errorMessage } from '../api/client';
  import { taskDateShortcuts } from '../utils/task-date';
  import { Button } from './ui/button';
  import { Calendar } from './ui/calendar';
  import * as Popover from './ui/popover';
  import * as ToggleGroup from './ui/toggle-group';
  import { Separator } from './ui/separator';
  import { Spinner } from './ui/spinner';
  import * as Field from './ui/field';

  let {
    today,
    dueDate,
    priority,
    busy = false,
    label,
    open = $bindable(false),
    onChange,
  }: {
    today: string;
    dueDate: string | null;
    priority: TaskPriority;
    busy?: boolean;
    label: string;
    open?: boolean;
    onChange: (payload: UpdateTaskRequest) => Promise<void>;
  } = $props();

  let calendarOpen = $state(false);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let chooseButton = $state<HTMLButtonElement | null>(null);
  let shortcuts = $derived(taskDateShortcuts(today));
  let selectedShortcut = $derived(
    dueDate === shortcuts.today
      ? 'today'
      : dueDate === shortcuts.tomorrow
        ? 'tomorrow'
        : dueDate === shortcuts.thisWeek
          ? 'thisWeek'
          : '',
  );
  let disabled = $derived(busy || saving);

  function changeOpen(value: boolean) {
    open = value;
    if (value) {
      calendarOpen = false;
      error = null;
    }
  }

  async function update(payload: UpdateTaskRequest) {
    if (disabled) return;
    saving = true;
    error = null;
    try {
      await onChange(payload);
      if (calendarOpen) {
        calendarOpen = false;
        await tick();
        chooseButton?.focus();
      }
    } catch (cause) {
      error = errorMessage(cause, 'Unable to update the task.');
    } finally {
      saving = false;
    }
  }

  function setDate(value: string | null) {
    if (value === dueDate) return;
    // Clearing the date also clears its clock time; other edits preserve existing time.
    void update(value === null ? { dueDate: null, dueTime: null } : { dueDate: value });
  }
</script>

<Popover.Root {open} onOpenChange={changeOpen}>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        aria-label={label}
        title="Target date and priority"
        disabled={busy}
        size="icon-sm"
        variant="ghost"
      >
        <SlidersHorizontal />
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content
    align="end"
    class="task-properties max-h-[var(--bits-popover-content-available-height)] w-[min(304px,calc(100vw-2rem))] overflow-y-auto overscroll-contain"
    aria-label="Task properties"
    data-calendar-open={calendarOpen}
    collisionPadding={4}
  >
    {#if calendarOpen}
      <Button
        variant="ghost"
        size="sm"
        class="self-start"
        onclick={async () => {
          calendarOpen = false;
          await tick();
          chooseButton?.focus();
        }}
      >
        <ArrowLeft data-icon="inline-start" /> Target date
      </Button>
      <Calendar
        type="single"
        captionLayout="dropdown"
        initialFocus
        {disabled}
        value={dueDate ? parseDate(dueDate) : undefined}
        placeholder={parseDate(dueDate || today)}
        onValueChange={(date) => setDate(date?.toString() ?? null)}
        class="mx-auto w-fit p-0"
      />
    {:else}
      <p class="property-label">Target date</p>
      <div class="date-choices">
        <ToggleGroup.Root
          type="single"
          bind:value={
            () => selectedShortcut,
            (value) => {
              if (value) setDate(shortcuts[value as keyof typeof shortcuts]);
            }
          }
          {disabled}
          aria-label="Target date shortcuts"
          class="grid w-full grid-cols-3 gap-0"
        >
          <ToggleGroup.Item value="today" aria-label="Today" title="Today" class="size-11 p-0"
            ><Sun /></ToggleGroup.Item
          >
          <ToggleGroup.Item
            value="tomorrow"
            aria-label="Tomorrow"
            title="Tomorrow"
            class="size-11 p-0"><Sunrise /></ToggleGroup.Item
          >
          <ToggleGroup.Item
            value="thisWeek"
            aria-label="This week"
            title={`This week · Sunday, ${shortcuts.thisWeek}`}
            class="size-11 p-0"><CalendarRange /></ToggleGroup.Item
          >
        </ToggleGroup.Root>
        <Button
          bind:ref={chooseButton}
          {disabled}
          variant="ghost"
          size="icon"
          class="size-11"
          aria-label="Choose date"
          title="Choose date"
          onclick={() => (calendarOpen = true)}><CalendarDays /></Button
        >
        <Button
          variant="ghost"
          size="icon"
          class="size-11"
          {disabled}
          aria-label="No date"
          title="No date"
          aria-pressed={!dueDate}
          onclick={() => setDate(null)}><CalendarOff /></Button
        >
      </div>
      <Separator />
      <ToggleGroup.Root
        type="single"
        variant="priority"
        spacing={1}
        aria-label="Task priority"
        bind:value={
          () => String(priority),
          (value) => {
            if (value) void update({ priority: value === '1' ? 1 : 0 });
          }
        }
        {disabled}
      >
        <ToggleGroup.Item value="0" aria-label="No priority" title="No priority" class="size-11 p-0"
          ><Flag /></ToggleGroup.Item
        >
        <ToggleGroup.Item
          value="1"
          aria-label="High priority"
          title="High priority"
          data-priority="high"
          class="size-11 p-0"><Flag fill="currentColor" /></ToggleGroup.Item
        >
      </ToggleGroup.Root>
    {/if}
    {#if saving}<span class="property-status" role="status"><Spinner /> Saving…</span>{/if}
    {#if error}<Field.Error>{error}</Field.Error>{/if}
  </Popover.Content>
</Popover.Root>

<style>
  .property-label {
    margin: 0 4px;
    color: var(--muted-foreground);
    font-size: 12px;
  }
  .date-choices {
    display: grid;
    grid-template-columns: minmax(0, 3fr) repeat(2, minmax(0, 1fr));
    align-items: center;
    justify-items: center;
  }
  .property-status {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
  }
</style>
