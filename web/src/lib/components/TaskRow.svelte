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
  let error = $state<string | null>(null);
  let titleInput = $state<HTMLInputElement>();
  let editButton = $state<HTMLButtonElement>();
  let dateLabel = $derived(taskDateLabel(task, today));

  async function beginEdit(): Promise<void> {
    title = task.title;
    description = task.description;
    priority = task.priority;
    dueDate = task.dueDate ?? '';
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
        dueTime: null,
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
  class:task-editing={editing}
  class:task-row-full={mode === 'all'}
  class="task-row"
  data-focus-uid={task.uid}
  tabindex="-1"
>
  {#if editing}
    <form class="task-edit-form" onsubmit={save}>
      <div class="task-edit-copy">
        <label class="field task-edit-title">
          <span class="sr-only">Title</span>
          <input
            bind:this={titleInput}
            disabled={busy}
            maxlength="500"
            onkeydown={handleEditorKeydown}
            bind:value={title}
          />
        </label>
        <label class="field task-edit-details">
          <span class="sr-only">Details</span>
          <textarea
            disabled={busy}
            onkeydown={handleEditorKeydown}
            rows="2"
            bind:value={description}></textarea>
        </label>
      </div>
      <div class="task-edit-grid">
        <label class="compact-field">
          <span>Date</span>
          <input disabled={busy} onkeydown={handleEditorKeydown} type="date" bind:value={dueDate} />
        </label>
        <div class="compact-field task-priority-field">
          <span>Priority</span>
          <label class:active={priority === 1} class="priority-toggle task-priority-toggle">
            <input
              checked={priority === 1}
              disabled={busy}
              onchange={(event) => (priority = event.currentTarget.checked ? 1 : 0)}
              onkeydown={handleEditorKeydown}
              type="checkbox"
            />
            <Icon name="flag" size={15} />
            <span>{priority === 1 ? 'Priority' : 'Regular'}</span>
          </label>
        </div>
      </div>
      {#if error}<p aria-live="assertive" class="form-error">{error}</p>{/if}
      <div class="task-edit-footer">
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
    gap: 13px;
    padding: 7px 0 2px;
    animation: editor-enter 180ms ease both;
  }

  .task-editing {
    padding-block: 9px 17px;
    border-bottom-color: transparent;
  }

  .task-edit-copy {
    display: grid;
    gap: 11px;
    padding: 10px 12px 12px;
    background: color-mix(in oklch, var(--color-surface-muted), var(--color-surface) 35%);
    border-radius: 12px;
    transition: box-shadow 150ms ease;
  }

  .task-edit-copy:focus-within {
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-focus), transparent 84%);
  }

  .task-edit-copy :is(input, textarea),
  .task-edit-copy :is(input, textarea):focus {
    padding-inline: 0;
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
  }

  .task-edit-form .compact-field > span {
    padding-left: 2px;
    font-size: 11px;
    font-weight: 650;
  }

  .task-edit-title input {
    min-height: 34px;
    font-size: 14px;
    font-weight: 620;
  }

  .task-edit-details textarea {
    min-height: 68px;
    padding-block: 2px 0;
    resize: none;
  }

  .task-edit-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    align-items: end;
  }

  .task-edit-grid input[type='date'] {
    min-height: 34px;
    padding: 6px 8px;
    background: color-mix(in oklch, var(--color-surface-muted), var(--color-surface) 35%);
    border-color: transparent;
    border-radius: 9px;
    font-size: 12px;
  }

  .task-edit-grid input[type='date']:focus {
    background: var(--color-surface);
    border-color: transparent;
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-focus), transparent 84%);
  }

  .task-priority-field {
    min-width: 0;
  }

  .task-priority-toggle {
    position: relative;
    width: 100%;
    min-height: 34px;
    gap: 8px;
    padding: 6px 9px;
    background: color-mix(in oklch, var(--color-surface-muted), var(--color-surface) 35%);
    border-radius: 9px;
    cursor: pointer;
    transition:
      color 150ms ease,
      background-color 150ms ease,
      box-shadow 150ms ease;
  }

  .task-priority-toggle input {
    position: absolute;
    width: 1px;
    height: 1px;
    min-height: 0;
    opacity: 0;
  }

  .task-priority-toggle.active {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
  }

  .task-priority-toggle:focus-within {
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-focus), transparent 84%);
  }

  .task-edit-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding-top: 2px;
  }

  .task-edit-footer .inline-form-actions {
    margin-top: 0;
  }

  .task-edit-footer .button {
    min-height: 38px;
    border-radius: 9px;
  }

  .task-edit-footer .button.secondary {
    background: transparent;
    border-color: transparent;
  }

  .task-row-full {
    grid-template-columns: 28px minmax(0, 1fr) 76px;
    padding: 16px 4px;
  }

  .task-row-full .task-title-line h3 {
    font-size: 14px;
  }

  .task-row-full .task-edit-grid {
    grid-template-columns: repeat(2, minmax(130px, 180px));
  }

  @keyframes editor-enter {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
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

    .task-row-full .task-edit-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (hover: none) {
    .row-actions {
      opacity: 1;
    }
  }
</style>
