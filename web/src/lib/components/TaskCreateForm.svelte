<script lang="ts">
  import { tick } from 'svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import Flag from '@lucide/svelte/icons/flag';
  import type { CreateTaskRequest, TaskPriority, UpdateTaskRequest } from '../api/types';
  import { errorMessage } from '../api/client';
  import { targetDateLabel } from '../utils/task-date';
  import TaskProperties from './TaskProperties.svelte';
  import * as Field from './ui/field';
  import * as InputGroup from './ui/input-group';
  import { Spinner } from './ui/spinner';

  let {
    mode,
    today,
    busy,
    onCreate,
  }: {
    mode: 'todo' | 'all';
    today: string;
    busy: boolean;
    onCreate: (payload: CreateTaskRequest) => Promise<void>;
  } = $props();

  let title = $state('');
  let priority = $state<TaskPriority>(0);
  let dueDate = $state<string | null>(null);
  let error = $state<string | null>(null);
  let submitting = $state(false);
  let propertiesOpen = $state(false);
  let titleInput = $state<HTMLInputElement | null>(null);
  let formBusy = $derived(busy || submitting);

  async function submit(event?: SubmitEvent) {
    event?.preventDefault();
    if (formBusy) return;
    if (!title.trim()) {
      error = 'Enter a task title.';
      return;
    }
    error = null;
    submitting = true;
    try {
      await onCreate({ title: title.trim(), priority, ...(dueDate ? { dueDate } : {}) });
      title = '';
      priority = 0;
      dueDate = null;
      propertiesOpen = false;
    } catch (cause) {
      error = errorMessage(cause, 'Unable to add the task.');
    } finally {
      submitting = false;
      await tick();
      titleInput?.focus();
    }
  }

  async function changeProperties(payload: UpdateTaskRequest) {
    if (payload.dueDate !== undefined) dueDate = payload.dueDate;
    if (payload.priority !== undefined) priority = payload.priority;
  }
</script>

<form class="task-create" onsubmit={submit}>
  <Field.Field class="gap-2" data-invalid={Boolean(error)}>
    <Field.Label class="sr-only" for={`new-task-${mode}`}>Task title</Field.Label>
    <InputGroup.Root variant="task">
      <InputGroup.Addon><Plus /></InputGroup.Addon>
      <InputGroup.Input
        bind:ref={titleInput}
        bind:value={title}
        id={`new-task-${mode}`}
        placeholder="Add a task…"
        autocomplete="off"
        maxlength={500}
        disabled={formBusy}
        onkeydown={(event) => {
          if (!event.isComposing && (event.ctrlKey || event.metaKey) && event.key === 'Enter') {
            event.preventDefault();
            void submit();
          }
        }}
        aria-invalid={Boolean(error)}
        aria-describedby={error ? `new-task-error-${mode}` : undefined}
      />
      <InputGroup.Addon align="inline-end" class="gap-1">
        <TaskProperties
          {today}
          {dueDate}
          {priority}
          busy={formBusy}
          bind:open={propertiesOpen}
          label="New task options"
          onChange={changeProperties}
        />
        {#if title.trim() || formBusy}
          <InputGroup.Button type="submit" size="sm" variant="default" disabled={formBusy}
            >{#if formBusy}<Spinner data-icon="inline-start" />{/if}{formBusy
              ? 'Adding…'
              : 'Add'}</InputGroup.Button
          >
        {/if}
      </InputGroup.Addon>
    </InputGroup.Root>
    {#if dueDate || priority === 1}
      <div class="creation-metadata">
        {#if dueDate}<span>{targetDateLabel(dueDate, today)}</span>{/if}
        {#if priority === 1}<span
            class="priority-marker"
            role="img"
            aria-label="High priority"
            title="High priority"><Flag /></span
          >{/if}
      </div>
    {/if}
    {#if error}<Field.Error id={`new-task-error-${mode}`} role="alert">{error}</Field.Error>{/if}
  </Field.Field>
</form>

<style>
  .task-create {
    margin-bottom: 12px;
  }
  .creation-metadata {
    display: flex;
    gap: 12px;
    padding-inline: 12px;
    color: var(--muted-foreground);
    font-size: 12px;
  }
  .creation-metadata span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .priority-marker {
    color: var(--priority-high);
  }
  .creation-metadata :global(svg) {
    width: 12px;
    height: 12px;
    fill: currentColor;
  }
</style>
