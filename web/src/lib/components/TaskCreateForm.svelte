<script lang="ts">
  import { tick } from 'svelte';
  import { Spinner } from './ui/spinner';
  import Calendar from '@lucide/svelte/icons/calendar';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Flag from '@lucide/svelte/icons/flag';
  import Plus from '@lucide/svelte/icons/plus';

  import type { CreateTaskRequest, TaskPriority } from '../api/types';
  import { errorMessage } from '../api/client';
  import { Button } from './ui/button';
  import * as DropdownMenu from './ui/dropdown-menu';
  import * as Field from './ui/field';
  import { Input } from './ui/input';

  let {
    mode,
    busy,
    onCreate,
  }: {
    mode: 'todo' | 'all';
    busy: boolean;
    onCreate: (payload: CreateTaskRequest) => Promise<void>;
  } = $props();

  let title = $state('');
  let description = $state('');
  let priority = $state<TaskPriority>(0);
  let dueDate = $state('');
  let dueTime = $state('');
  let error = $state<string | null>(null);
  let submitting = $state(false);
  let compactFocused = $state(false);
  let detailsOpen = $state(false);
  let priorityOpen = $state(false);
  let moreButton = $state<HTMLButtonElement | null>(null);
  let titleInput = $state<HTMLInputElement | null>(null);
  let formElement = $state<HTMLFormElement | null>(null);
  let formBusy = $derived(busy || submitting);
  let quickActionsVisible = $derived(
    compactFocused ||
      priorityOpen ||
      Boolean(title.trim()) ||
      Boolean(dueDate) ||
      priority === 1 ||
      detailsOpen,
  );

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (busy || submitting) return;
    const trimmedTitle = title.trim();
    if (!trimmedTitle) {
      error = 'Enter a task title.';
      return;
    }

    error = null;
    submitting = true;
    try {
      const payload: CreateTaskRequest = {
        description: description.trim() || undefined,
        priority,
        title: trimmedTitle,
      };
      if (dueDate) {
        payload.dueDate = dueDate;
        if (dueTime) payload.dueTime = dueTime;
      }
      await onCreate(payload);
      title = '';
      description = '';
      dueTime = '';
      priority = 0;
      priorityOpen = false;
      dueDate = '';
      detailsOpen = false;
    } catch (cause) {
      error = errorMessage(cause, 'Unable to add the task.');
    } finally {
      submitting = false;
      await tick();
      titleInput?.focus();
    }
  }

  function handleFocusOut(event: FocusEvent & { currentTarget: HTMLFormElement }): void {
    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) {
      return;
    }
    compactFocused = false;
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (
      (event.ctrlKey || event.metaKey) &&
      event.key === 'Enter' &&
      event.target instanceof Node &&
      formElement?.contains(event.target)
    ) {
      event.preventDefault();
      formElement.requestSubmit();
      return;
    }
    if (event.key !== 'Escape') return;
    if (!detailsOpen) return;
    event.preventDefault();
    detailsOpen = false;
    requestAnimationFrame(() => moreButton?.focus());
  }

  function selectPriority(value: TaskPriority): void {
    priority = value;
    priorityOpen = false;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<form
  bind:this={formElement}
  class:has-advanced={mode === 'all' && detailsOpen}
  class="task-create task-create-compact"
  onfocusin={() => (compactFocused = true)}
  onfocusout={handleFocusOut}
  onsubmit={submit}
>
  <Field.Field
    class="task-title-field min-w-0 flex-row items-center gap-2"
    orientation="horizontal"
  >
    <Field.Label class="sr-only" for={`new-task-${mode}`}>Task title</Field.Label>
    <span class="task-add-icon" aria-hidden="true"><Plus /></span>
    <Input
      class="border-0 bg-transparent shadow-none focus-visible:ring-0"
      autocomplete="off"
      bind:ref={titleInput}
      disabled={formBusy}
      id={`new-task-${mode}`}
      maxlength={500}
      placeholder="Add a task…"
      bind:value={title}
    />
  </Field.Field>

  <div aria-hidden={!quickActionsVisible} class:visible={quickActionsVisible} class="quick-actions">
    <label
      class:active={Boolean(dueDate)}
      class="quick-action date-action"
      title={dueDate ? `Due ${dueDate}` : 'Set due date'}
    >
      <Calendar />
      {#if dueDate}<span class="selected-date">{dueDate}</span>{/if}
      <Input
        class="absolute inset-0 min-h-0 cursor-pointer p-0 opacity-0"
        aria-label="Due date"
        disabled={formBusy}
        tabindex={quickActionsVisible ? 0 : -1}
        type="date"
        bind:value={dueDate}
      />
    </label>
    <DropdownMenu.Root bind:open={priorityOpen}>
      <DropdownMenu.Trigger disabled={formBusy} tabindex={quickActionsVisible ? 0 : -1}>
        {#snippet child({ props })}
          <Button
            {...props}
            aria-label="Set priority"
            size="icon-sm"
            title={priority === 1 ? 'Priority task' : 'Set priority'}
            variant={priority === 1 ? 'secondary' : 'ghost'}
          >
            <Flag />
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      {#if priorityOpen}
        <DropdownMenu.Content align="end" class="w-36" forceMount>
          <DropdownMenu.Label>Task priority</DropdownMenu.Label>
          <DropdownMenu.RadioGroup value={String(priority)}>
            <DropdownMenu.RadioItem onclick={() => selectPriority(0)} value="0">
              <Flag />
              Regular
            </DropdownMenu.RadioItem>
            <DropdownMenu.RadioItem onclick={() => selectPriority(1)} value="1">
              <Flag />
              Priority
            </DropdownMenu.RadioItem>
          </DropdownMenu.RadioGroup>
        </DropdownMenu.Content>
      {/if}
    </DropdownMenu.Root>
    {#if mode === 'all'}
      <Button
        aria-controls="new-task-options"
        aria-expanded={detailsOpen}
        aria-label="More task options"
        bind:ref={moreButton}
        disabled={formBusy}
        onclick={() => (detailsOpen = !detailsOpen)}
        size="icon-sm"
        tabindex={quickActionsVisible ? 0 : -1}
        title="More task options"
        variant={detailsOpen || description || dueTime ? 'secondary' : 'ghost'}
      >
        <Ellipsis />
      </Button>
    {/if}
    {#if title.trim() || formBusy}
      <Button class="task-submit ml-auto" type="submit" disabled={formBusy} size="sm">
        {#if formBusy}<Spinner data-icon="inline-start" />{/if}
        {formBusy ? 'Adding…' : 'Add'}
      </Button>
    {/if}
  </div>

  {#if mode === 'all' && detailsOpen}
    <div class="task-advanced" id="new-task-options">
      <Field.Field class="min-w-0 gap-2">
        <Field.Label for={`new-task-details-${mode}`}>Details</Field.Label>
        <Input
          class="text-xs max-[767px]:text-base"
          autocomplete="off"
          disabled={formBusy}
          id={`new-task-details-${mode}`}
          bind:value={description}
        />
      </Field.Field>
      <Field.Field class="gap-2">
        <Field.Label for={`new-task-time-${mode}`}>Time</Field.Label>
        <Input
          aria-label="Due time"
          disabled={formBusy || !dueDate}
          id={`new-task-time-${mode}`}
          type="time"
          bind:value={dueTime}
        />
      </Field.Field>
    </div>
  {/if}

  {#if error}
    <Field.Error aria-live="assertive" class="col-span-full">{error}</Field.Error>
  {/if}
</form>

<style>
  .task-create {
    margin-bottom: 16px;
  }

  .task-add-icon {
    color: var(--muted-foreground);
  }

  .task-create-compact {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 4px;
    align-items: center;
    padding: 5px 6px 5px 10px;
    margin-bottom: 12px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    transition:
      border-color 150ms ease,
      box-shadow 150ms ease;
  }

  .task-create-compact:focus-within {
    border-color: var(--ring);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--ring), transparent 84%);
  }

  .quick-actions {
    display: none;
    grid-column: 1 / -1;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
  }

  .quick-actions.visible {
    display: flex;
  }

  .selected-date {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .date-action.active {
    display: inline-flex;
    width: auto;
    gap: 4px;
    padding-inline: 8px;
  }

  .date-action :global(svg) {
    width: 16px;
    height: 16px;
    flex: none;
  }

  .quick-action {
    position: relative;
    display: inline-grid;
    width: 32px;
    height: 32px;
    flex: none;
    padding: 0;
    color: var(--muted-foreground);
    background: transparent;
    border: 0;
    border-radius: 7px;
    cursor: pointer;
    place-items: center;
  }

  .quick-action:hover,
  .quick-action:focus-visible,
  .quick-action:focus-within {
    color: var(--foreground);
    background: var(--muted);
  }

  .quick-action:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .quick-action.active {
    color: var(--primary);
    background: var(--accent);
  }

  .task-advanced {
    display: grid;
    min-width: 0;
    grid-column: 1 / -1;
    grid-template-columns: minmax(0, 1fr) minmax(120px, 150px);
    gap: 10px;
    padding: 10px 2px 2px 26px;
    animation: details-enter 160ms ease both;
  }

  @keyframes details-enter {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (max-width: 767px) {
    .task-create-compact {
      padding: 5px 6px 5px 10px;
    }

    .quick-actions :global(button) {
      min-height: 44px;
      min-width: 44px;
    }

    .quick-action {
      width: 44px;
      min-height: 44px;
    }

    .task-advanced {
      grid-template-columns: minmax(0, 1fr);
      padding: 10px 2px 4px 26px;
    }
  }
</style>
