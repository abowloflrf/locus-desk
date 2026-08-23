<script lang="ts">
  import { tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import type { Task, TaskPriority, UpdateTaskRequest } from '../api/types';
  import { isTaskOverdue, taskDateLabel } from '../utils/date';
  import Icon from './Icon.svelte';

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
    mode?: 'today' | 'all';
    busy: boolean;
    onToggle: (task: Task) => Promise<void>;
    onSave: (task: Task, payload: UpdateTaskRequest) => Promise<void>;
    onDelete: (task: Task) => void;
  } = $props();

  let editing = $state(false);
  let title = $state('');
  let description = $state('');
  let priority = $state<TaskPriority>(0);
  let dueDate = $state('');
  let dueTime = $state('');
  let error = $state<string | null>(null);
  let titleInput = $state<HTMLInputElement>();
  let editButton = $state<HTMLButtonElement>();
  let dateLabel = $derived(taskDateLabel(task, today));

  async function beginEdit(): Promise<void> {
    title = task.title;
    description = task.description;
    priority = task.priority;
    dueDate = task.dueDate ?? '';
    dueTime = task.dueTime ?? '';
    error = null;
    editing = true;
    await tick();
    titleInput?.focus();
    titleInput?.select();
  }

  async function closeEditor(): Promise<void> {
    editing = false;
    error = null;
    await tick();
    editButton?.focus();
  }

  async function save(event?: SubmitEvent): Promise<void> {
    event?.preventDefault();
    if (!title.trim()) {
      error = 'Enter a task title.';
      return;
    }

    error = null;
    try {
      await onSave(task, {
        description: description.trim(),
        dueDate: dueDate || null,
        dueTime: dueDate && dueTime ? dueTime : null,
        priority,
        title: title.trim(),
      });
      await closeEditor();
    } catch (cause) {
      error = errorMessage(cause, 'Unable to save the task.');
    }
  }

  function handleEditorKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      void closeEditor();
      return;
    }
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
      event.preventDefault();
      void save();
    }
  }
</script>

<article
  class:task-done={task.status === 'DONE'}
  class:task-row-full={mode === 'all'}
  class="task-row"
  data-focus-uid={task.uid}
  tabindex="-1"
>
  {#if editing}
    <form class="task-edit-form" onsubmit={save}>
      <label class="field">
        <span>Title</span>
        <input
          bind:this={titleInput}
          disabled={busy}
          maxlength="500"
          onkeydown={handleEditorKeydown}
          bind:value={title}
        />
      </label>
      <label class="field">
        <span>Details</span>
        <textarea disabled={busy} onkeydown={handleEditorKeydown} rows="2" bind:value={description}
        ></textarea>
      </label>
      <div class="task-edit-grid">
        <label class="compact-field">
          <span>Date</span>
          <input disabled={busy} onkeydown={handleEditorKeydown} type="date" bind:value={dueDate} />
        </label>
        <label class="compact-field">
          <span>Time</span>
          <input
            disabled={busy || !dueDate}
            onkeydown={handleEditorKeydown}
            type="time"
            bind:value={dueTime}
          />
        </label>
        <label class="priority-toggle">
          <input
            checked={priority === 1}
            disabled={busy}
            onchange={(event) => (priority = event.currentTarget.checked ? 1 : 0)}
            onkeydown={handleEditorKeydown}
            type="checkbox"
          />
          <span>Priority</span>
        </label>
      </div>
      {#if error}<p aria-live="assertive" class="form-error">{error}</p>{/if}
      <div class="inline-form-actions">
        <button
          class="button secondary"
          disabled={busy}
          onclick={() => void closeEditor()}
          onkeydown={handleEditorKeydown}
          type="button">Cancel</button
        >
        <button
          class="button primary"
          disabled={busy || !title.trim()}
          onkeydown={handleEditorKeydown}
          type="submit"
        >
          {busy ? 'Saving…' : 'Save'}
        </button>
      </div>
    </form>
  {:else}
    <button
      aria-label={task.status === 'DONE' ? `Restore ${task.title}` : `Complete ${task.title}`}
      aria-pressed={task.status === 'DONE'}
      class="task-checkbox"
      disabled={busy}
      onclick={() => void onToggle(task)}
      type="button"
    >
      <span aria-hidden="true">{task.status === 'DONE' ? '✓' : ''}</span>
    </button>
    <div class="task-copy">
      <div class="task-title-line">
        <h3>{task.title}</h3>
        {#if mode === 'all' && task.priority === 1 && task.status === 'TODO'}<span
            class="priority-mark">Priority</span
          >{/if}
      </div>
      {#if task.description}<p>{task.description}</p>{/if}
      {#if dateLabel}
        <span class:overdue={isTaskOverdue(task, today)} class="task-date">{dateLabel}</span>
      {/if}
    </div>
    <div class="row-actions">
      <button
        aria-label={`Edit ${task.title}`}
        bind:this={editButton}
        class="icon-button"
        disabled={busy}
        onclick={() => void beginEdit()}
        type="button"
      >
        <Icon name="edit" size={16} />
      </button>
      <button
        aria-label={`Delete ${task.title}`}
        class="icon-button danger-quiet"
        disabled={busy}
        onclick={() => onDelete(task)}
        type="button"
      >
        <Icon name="delete" size={16} />
      </button>
    </div>
  {/if}
</article>

<style>
  .task-row {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    gap: 11px;
    align-items: start;
    padding: 13px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .task-checkbox {
    display: grid;
    width: 19px;
    height: 19px;
    padding: 0;
    margin-top: 2px;
    color: var(--color-surface);
    background: transparent;
    border: 1px solid color-mix(in oklch, var(--color-text-muted), transparent 25%);
    border-radius: 5px;
    font-size: 12px;
    font-weight: 700;
    place-items: center;
  }

  .task-checkbox[aria-pressed='true'] {
    background: var(--color-accent);
    border-color: var(--color-accent);
  }

  .task-copy {
    min-width: 0;
  }

  .task-title-line {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }

  .task-title-line h3 {
    margin-bottom: 0;
    overflow-wrap: anywhere;
    font-size: 13px;
    font-weight: 560;
    line-height: 20px;
  }

  .priority-mark {
    flex: none;
    color: var(--color-accent-hover);
    font-size: 10px;
    font-weight: 620;
  }

  .task-copy p {
    margin: 3px 0 0;
    color: var(--color-text-muted);
    overflow-wrap: anywhere;
    font-size: 12px;
    line-height: 18px;
    white-space: pre-wrap;
  }

  .task-date {
    display: block;
    margin-top: 4px;
    color: var(--color-accent-hover);
    font-size: 11px;
  }

  .task-date.overdue {
    color: var(--color-danger);
    font-weight: 580;
  }

  .task-done .task-title-line h3 {
    color: var(--color-text-muted);
    text-decoration: line-through;
    text-decoration-color: var(--color-border);
  }

  .row-actions {
    display: flex;
    gap: 1px;
    opacity: 0;
    transition: opacity 140ms ease;
  }

  .task-row:hover .row-actions,
  .task-row:focus-within .row-actions {
    opacity: 1;
  }

  .task-edit-form {
    grid-column: 1 / -1;
    display: grid;
    gap: 10px;
    padding: 12px;
    background: var(--color-canvas);
    border: 1px solid var(--color-border);
    border-radius: 8px;
  }

  .task-edit-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr)) auto;
    gap: 8px;
    align-items: end;
  }

  .task-edit-grid input[type='date'],
  .task-edit-grid input[type='time'] {
    min-height: 34px;
    padding: 6px 8px;
    font-size: 12px;
  }

  .task-row-full {
    grid-template-columns: 28px minmax(0, 1fr) 76px;
    padding: 16px 4px;
  }

  .task-row-full .task-title-line h3 {
    font-size: 14px;
  }

  @media (max-width: 1199px) {
    .row-actions {
      opacity: 1;
    }
  }

  @media (max-width: 767px) {
    .task-row,
    .task-row-full {
      grid-template-columns: 44px minmax(0, 1fr) 44px;
      align-items: start;
    }

    .task-checkbox {
      min-width: 44px;
      min-height: 44px;
      margin: 0;
    }

    .row-actions {
      display: grid;
    }

    .task-edit-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .task-edit-grid .priority-toggle {
      grid-column: 1 / -1;
    }
  }

  @media (hover: none) {
    .row-actions {
      opacity: 1;
    }
  }
</style>
