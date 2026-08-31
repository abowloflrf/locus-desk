<script lang="ts">
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
  let formBusy = $derived(busy || submitting);
  let quickActionsVisible = $derived(compactFocused || priorityOpen);

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
    }
  }

  function handleFocusOut(event: FocusEvent & { currentTarget: HTMLFormElement }): void {
    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) {
      return;
    }
    compactFocused = false;
  }

  function handleKeydown(event: KeyboardEvent): void {
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
      disabled={formBusy}
      id={`new-task-${mode}`}
      maxlength={500}
      placeholder="Add a task…"
      bind:value={title}
    />
  </Field.Field>

  <div
    aria-hidden={!quickActionsVisible}
    class:wide={mode === 'all'}
    class:visible={quickActionsVisible}
    class="quick-actions"
  >
    <label
      class:active={Boolean(dueDate)}
      class="quick-action date-action"
      title={dueDate ? `Due ${dueDate}` : 'Set due date'}
    >
      <Calendar />
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
    margin-bottom: 24px;
  }

  .task-add-icon {
    color: var(--muted-foreground);
  }

  .task-create-compact {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px;
    align-items: center;
    padding: 5px 6px 5px 10px;
    margin-bottom: 18px;
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
    display: flex;
    width: 0;
    align-items: center;
    gap: 2px;
    opacity: 0;
    overflow: visible;
    pointer-events: none;
    transform: translateX(4px);
    visibility: hidden;
    transition:
      width 150ms ease,
      opacity 120ms ease,
      transform 150ms ease,
      visibility 120ms step-end;
  }

  .quick-actions.visible {
    width: 66px;
    opacity: 1;
    pointer-events: auto;
    transform: translateX(0);
    visibility: visible;
    transition:
      width 150ms ease,
      opacity 120ms ease 30ms,
      transform 150ms ease,
      visibility 0ms step-start;
  }

  .quick-actions.visible.wide {
    width: 100px;
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
    outline: 0;
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

    .quick-actions.visible {
      width: 90px;
    }

    .quick-actions.visible.wide {
      width: 136px;
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
