<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';
  import Flag from '@lucide/svelte/icons/flag';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import { tick } from 'svelte';
  import type { Task, UpdateTaskRequest } from '../api/types';
  import { errorMessage } from '../api/client';
  import { targetDateLabel } from '../utils/task-date';
  import TaskProperties from './TaskProperties.svelte';
  import { Button } from './ui/button';
  import { Checkbox } from './ui/checkbox';
  import * as Field from './ui/field';
  import { Input } from './ui/input';
  import * as Popover from './ui/popover';
  import { Textarea } from './ui/textarea';
  import { Spinner } from './ui/spinner';

  let {
    task,
    today,
    mode = 'all',
    busy,
    onToggle,
    onSave,
    onDelete,
  }: {
    task: Task;
    today: string;
    mode?: 'todo' | 'all';
    busy: boolean;
    onToggle: (task: Task) => Promise<void>;
    onSave: (task: Task, payload: UpdateTaskRequest) => Promise<void>;
    onDelete: (task: Task) => void;
  } = $props();

  let editing = $state(false);
  let title = $state('');
  let titleError = $state<string | null>(null);
  let titleSaving = $state(false);
  let titleInput = $state<HTMLInputElement | null>(null);
  let titleButton = $state<HTMLButtonElement | null>(null);
  let rowElement = $state<HTMLElement>();
  let propertiesOpen = $state(false);
  let detailsOpen = $state(false);
  let description = $state('');
  let descriptionBase = $state('');
  let detailsError = $state<string | null>(null);
  let detailsSaving = $state(false);
  let detailsButton = $state<HTMLButtonElement | null>(null);
  let dateLabel = $derived(targetDateLabel(task.dueDate, today));
  let enteringEdit = false;
  let wasPopoverOpen = false;

  function saveOnLeave() {
    queueMicrotask(() => {
      if (
        editing &&
        !enteringEdit &&
        !propertiesOpen &&
        !detailsOpen &&
        !rowElement?.contains(document.activeElement)
      ) {
        void saveTitle(false);
      }
    });
  }

  $effect(() => {
    const popoverOpen = propertiesOpen || detailsOpen;
    if (wasPopoverOpen && !popoverOpen) saveOnLeave();
    wasPopoverOpen = popoverOpen;
  });

  async function beginEdit() {
    if (busy) return;
    title = task.title;
    titleError = null;
    enteringEdit = true;
    editing = true;
    await tick();
    titleInput?.focus();
    titleInput?.select();
    enteringEdit = false;
  }

  async function cancelTitle() {
    if (titleSaving) return;
    editing = false;
    titleError = null;
    await tick();
    titleButton?.focus();
  }

  async function saveTitle(restoreFocus = true) {
    if (!editing || titleSaving) return;
    const nextTitle = title.trim();
    if (!nextTitle) {
      titleError = 'Enter a task title.';
      return;
    }
    titleError = null;
    titleSaving = true;
    try {
      if (nextTitle !== task.title) await onSave(task, { title: nextTitle });
      const shouldRestore = restoreFocus && document.activeElement === titleInput;
      editing = false;
      await tick();
      if (shouldRestore) titleButton?.focus();
    } catch (cause) {
      titleError = errorMessage(cause, 'Unable to save the title.');
    } finally {
      titleSaving = false;
    }
  }

  function titleKeydown(event: KeyboardEvent) {
    if (event.isComposing) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      void cancelTitle();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      void saveTitle();
    }
  }

  function changeDetailsOpen(value: boolean) {
    if (detailsSaving) return;
    if (value && description === descriptionBase) {
      description = task.description;
      descriptionBase = task.description;
    }
    detailsError = null;
    detailsOpen = value;
  }

  async function saveDetails(event?: SubmitEvent) {
    event?.preventDefault();
    if (busy || detailsSaving) return;
    detailsSaving = true;
    detailsError = null;
    try {
      await onSave(task, { description: description.trim() });
      description = description.trim();
      descriptionBase = description;
      detailsOpen = false;
    } catch (cause) {
      detailsError = errorMessage(cause, 'Unable to save the details.');
    } finally {
      detailsSaving = false;
    }
  }
</script>

<article
  bind:this={rowElement}
  onfocusout={saveOnLeave}
  class="task-row"
  class:task-row-todo={mode === 'todo'}
  class:task-done={task.status === 'DONE'}
  class:task-selected={editing || propertiesOpen || detailsOpen}
  data-focus-uid={task.uid}
  tabindex="-1"
>
  <div class="task-check">
    <Checkbox
      class="size-5"
      aria-label={task.status === 'DONE' ? `Restore ${task.title}` : `Complete ${task.title}`}
      checked={task.status === 'DONE'}
      disabled={busy || editing}
      onclick={() => void onToggle(task)}
    />
  </div>
  <div class="task-copy">
    {#if editing}
      <Field.Field class="gap-1" data-invalid={Boolean(titleError)}>
        <Field.Label class="sr-only" for={`task-title-${task.uid}`}>Task title</Field.Label>
        <Input
          variant="task-title"
          bind:ref={titleInput}
          bind:value={title}
          id={`task-title-${task.uid}`}
          aria-invalid={Boolean(titleError)}
          aria-describedby={titleError ? `task-title-error-${task.uid}` : undefined}
          readonly={titleSaving}
          maxlength={500}
          onkeydown={titleKeydown}
        />
        {#if titleError}<Field.Error id={`task-title-error-${task.uid}`} role="alert"
            >{titleError}</Field.Error
          >{/if}
      </Field.Field>
    {:else}
      <button
        bind:this={titleButton}
        class="task-title-button"
        aria-label={`Edit ${task.title}`}
        title={task.title}
        disabled={busy}
        onclick={beginEdit}>{task.title}</button
      >
    {/if}
  </div>
  {#if task.priority === 1 && task.status === 'TODO'}
    <span class="task-priority" role="img" aria-label="High priority" title="High priority"
      ><Flag /></span
    >
  {/if}
  <div class="task-metadata">
    <Popover.Root open={detailsOpen} onOpenChange={changeDetailsOpen}>
      <Popover.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            bind:ref={detailsButton}
            aria-label={`Details for ${task.title}`}
            title={task.description ? 'View task details' : 'Add details'}
            disabled={busy}
            variant="ghost"
            size="icon-sm"
            class={task.description
              ? 'task-details-trigger'
              : 'task-details-trigger task-details-empty'}><FileText /></Button
          >
        {/snippet}
      </Popover.Trigger>
      <Popover.Content
        align="end"
        class="w-[min(352px,calc(100vw-24px))] rounded-xl"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          detailsButton?.focus();
        }}
      >
        <form onsubmit={saveDetails}>
          <Field.FieldGroup class="gap-4">
            <Field.Field data-invalid={Boolean(detailsError)}>
              <Field.Label for={`task-details-${task.uid}`}>Details</Field.Label>
              <Textarea
                id={`task-details-${task.uid}`}
                bind:value={description}
                disabled={busy || detailsSaving}
                rows={4}
                class="max-h-[40dvh]"
                onkeydown={(event) => {
                  if (
                    !event.isComposing &&
                    (event.ctrlKey || event.metaKey) &&
                    event.key === 'Enter'
                  ) {
                    event.preventDefault();
                    void saveDetails();
                  }
                }}
                placeholder="Add a note…"
              />
              {#if detailsError}<Field.Error role="alert">{detailsError}</Field.Error>{/if}
            </Field.Field>
            <div class="details-actions">
              <Button
                aria-label={`Delete ${task.title}`}
                title="Delete task"
                disabled={busy}
                variant="ghost"
                size="icon-sm"
                onclick={() => onDelete(task)}><Trash2 /></Button
              >
              <div class="flex gap-2">
                <Button
                  variant="ghost"
                  disabled={busy}
                  onclick={() => {
                    description = task.description;
                    descriptionBase = description;
                    detailsOpen = false;
                  }}>Cancel</Button
                >
                <Button type="submit" disabled={busy || detailsSaving}
                  >{#if detailsSaving}<Spinner data-icon="inline-start" />{/if}Save</Button
                >
              </div>
            </div>
          </Field.FieldGroup>
        </form>
      </Popover.Content>
    </Popover.Root>
    {#if dateLabel && task.status === 'TODO'}<span
        class="task-date"
        title={task.dueTime ? `${task.dueDate} · ${task.dueTime}` : (task.dueDate ?? undefined)}
        >{dateLabel}</span
      >{/if}
  </div>
  <div class="task-options">
    <TaskProperties
      {today}
      dueDate={task.dueDate}
      priority={task.priority}
      {busy}
      label={`Target date and priority for ${task.title}`}
      bind:open={propertiesOpen}
      onChange={(payload) => onSave(task, payload)}
    />
  </div>
</article>

<style>
  .task-row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    min-height: 56px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    transition: background-color 150ms ease;
  }
  .task-row:hover,
  .task-row:focus-within,
  .task-selected {
    background: var(--muted);
  }
  .task-check {
    display: grid;
    flex: 0 0 24px;
    min-height: 32px;
    place-items: center;
  }
  .task-copy {
    flex: 1;
    min-width: 0;
  }
  .task-title-button {
    display: block;
    width: 100%;
    min-height: 36px;
    padding: 4px 0;
    border: 0;
    background: transparent;
    color: inherit;
    font-size: 15px;
    line-height: 24px;
    text-align: left;
    overflow-wrap: anywhere;
    cursor: text;
    border-radius: 2px;
  }
  .task-priority {
    flex: none;
    display: flex;
    color: var(--priority-high);
  }
  .task-priority :global(svg) {
    width: 16px;
    height: 16px;
    fill: currentColor;
  }
  .task-metadata {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--muted-foreground);
  }
  .task-date {
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: nowrap;
  }
  .task-options {
    flex: none;
    color: var(--muted-foreground);
  }
  .task-done {
    color: var(--muted-foreground);
  }
  .task-done .task-title-button {
    text-decoration: line-through;
  }
  .task-done .task-check {
    opacity: 0.5;
  }
  .details-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .task-row :global(.task-details-empty) {
    opacity: 0;
  }
  .task-row:hover :global(.task-details-empty),
  .task-row:focus-within :global(.task-details-empty) {
    opacity: 1;
  }
  .task-row-todo {
    gap: 4px;
    min-height: 48px;
    padding: 4px;
    flex-wrap: wrap;
  }
  .task-row-todo .task-title-button {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .task-row-todo .task-metadata {
    gap: 4px;
  }
  .task-row-todo .task-date {
    max-width: 84px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 11px;
  }
  @media (max-width: 767px) {
    .task-row {
      display: grid;
      grid-template-columns: 32px minmax(0, 1fr) auto 44px 44px;
      gap: 0 4px;
      padding: 8px 4px;
    }
    .task-check {
      grid-column: 1;
      grid-row: 1;
      min-height: 44px;
    }
    .task-copy {
      grid-column: 2;
      grid-row: 1;
    }
    .task-title-button {
      min-height: 44px;
      font-size: 16px;
    }
    .task-priority {
      grid-column: 3;
      grid-row: 1;
    }
    .task-options {
      grid-column: 5;
      grid-row: 1;
    }
    .task-metadata {
      display: contents;
    }
    .task-row :global(.task-details-trigger) {
      grid-column: 4;
      grid-row: 1;
    }
    .task-date {
      grid-column: 2;
      grid-row: 2;
    }
    .task-row :global(.task-details-empty) {
      opacity: 1;
    }
    .task-row :global(button[data-slot='button']) {
      min-width: 44px;
      min-height: 44px;
    }
  }
  @media (hover: none) {
    .task-row :global(.task-details-empty) {
      opacity: 1;
    }
  }
</style>
