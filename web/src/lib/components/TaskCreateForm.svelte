<script lang="ts">
  import type { CreateTaskRequest, TaskPriority } from '../api/types';
  import { errorMessage } from '../api/client';
  import Icon from './Icon.svelte';

  let {
    mode,
    busy,
    onCreate,
  }: {
    mode: 'today' | 'all';
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
  let priorityOpen = $state(false);
  let priorityButton = $state<HTMLButtonElement>();
  let formBusy = $derived(busy || submitting);

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
      await onCreate({
        description: description.trim() || undefined,
        dueDate: dueDate || undefined,
        dueTime: dueDate && dueTime ? dueTime : undefined,
        priority,
        title: trimmedTitle,
      });
      title = '';
      description = '';
      dueTime = '';
      priority = 0;
      priorityOpen = false;
      dueDate = '';
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
    priorityOpen = false;
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape' || !priorityOpen) return;
    event.preventDefault();
    priorityOpen = false;
    requestAnimationFrame(() => priorityButton?.focus());
  }

  function selectPriority(value: TaskPriority): void {
    priority = value;
    priorityOpen = false;
    requestAnimationFrame(() => priorityButton?.focus());
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<form
  class:task-create-compact={mode === 'today'}
  class="task-create"
  onfocusin={() => (compactFocused = true)}
  onfocusout={handleFocusOut}
  onsubmit={submit}
>
  <div class="field task-title-field">
    <label class="sr-only" for={`new-task-${mode}`}>Task title</label>
    {#if mode === 'today'}
      <span class="task-add-icon" aria-hidden="true"><Icon name="plus" size={19} /></span>
    {/if}
    <input
      autocomplete="off"
      disabled={formBusy}
      id={`new-task-${mode}`}
      maxlength="500"
      placeholder={mode === 'today' ? 'Add a task…' : 'What needs to be done?'}
      bind:value={title}
    />
  </div>

  {#if mode === 'all'}
    <div class="field task-description-field">
      <label class="sr-only" for="new-task-description">Task details</label>
      <input
        autocomplete="off"
        disabled={formBusy}
        id="new-task-description"
        placeholder="Details (optional)"
        bind:value={description}
      />
    </div>
  {/if}

  {#if mode === 'all'}
    <div class="task-create-options">
      <label class="compact-field">
        <span>Date</span>
        <input aria-label="Due date" disabled={formBusy} type="date" bind:value={dueDate} />
      </label>
      <label class="compact-field">
        <span>Time</span>
        <input
          aria-label="Due time"
          disabled={formBusy || !dueDate}
          type="time"
          bind:value={dueTime}
        />
      </label>
      <label class="priority-toggle">
        <input
          checked={priority === 1}
          disabled={formBusy}
          onchange={(event) => (priority = event.currentTarget.checked ? 1 : 0)}
          type="checkbox"
        />
        <span>Priority</span>
      </label>
      <button
        class="button primary task-add-button"
        disabled={formBusy || !title.trim()}
        type="submit"
      >
        {formBusy ? 'Adding…' : 'Add task'}
      </button>
    </div>
  {:else}
    <div aria-hidden={!compactFocused} class:visible={compactFocused} class="quick-actions">
      <label
        class:active={Boolean(dueDate)}
        class="quick-action date-action"
        title={dueDate ? `Due ${dueDate}` : 'Set due date'}
      >
        <Icon name="calendar" size={17} />
        <input
          aria-label="Due date"
          disabled={formBusy}
          tabindex={compactFocused ? 0 : -1}
          type="date"
          bind:value={dueDate}
        />
      </label>
      <div class="priority-picker">
        <button
          aria-expanded={priorityOpen}
          aria-haspopup="menu"
          aria-label="Set priority"
          bind:this={priorityButton}
          class:active={priority === 1}
          class="quick-action"
          disabled={formBusy}
          onclick={() => (priorityOpen = !priorityOpen)}
          tabindex={compactFocused ? 0 : -1}
          title={priority === 1 ? 'Priority task' : 'Set priority'}
          type="button"
        >
          <Icon name="flag" size={17} />
        </button>
        {#if priorityOpen}
          <div aria-label="Task priority" class="priority-menu" role="menu">
            <button
              aria-checked={priority === 0}
              onclick={() => selectPriority(0)}
              role="menuitemradio"
              type="button"
            >
              <Icon name="flag" size={16} />
              <span>Regular</span>
            </button>
            <button
              aria-checked={priority === 1}
              class="priority-option"
              onclick={() => selectPriority(1)}
              role="menuitemradio"
              type="button"
            >
              <Icon name="flag" size={16} />
              <span>Priority</span>
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if error}
    <p aria-live="assertive" class="form-error">{error}</p>
  {/if}
</form>

<style>
  .task-create {
    padding: 16px;
    margin-bottom: 28px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 10px;
  }

  .task-create-compact {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px;
    align-items: center;
    order: 20;
    padding: 5px 6px 5px 10px;
    margin: auto 0 0;
    background: var(--color-surface);
    transition:
      border-color 150ms ease,
      box-shadow 150ms ease;
  }

  .task-create-compact:focus-within {
    border-color: var(--color-focus);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-focus), transparent 84%);
  }

  .task-create-compact .task-title-field {
    display: grid;
    min-width: 0;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 7px;
    align-items: center;
  }

  .task-create-compact .task-title-field input {
    min-width: 0;
    background: transparent;
    border: 0;
    box-shadow: none;
  }

  .task-add-icon {
    color: var(--color-text-muted);
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

  .quick-action {
    position: relative;
    display: inline-grid;
    width: 32px;
    height: 32px;
    flex: none;
    padding: 0;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-radius: 7px;
    cursor: pointer;
    place-items: center;
  }

  .quick-action:hover,
  .quick-action:focus-visible,
  .quick-action:focus-within {
    color: var(--color-text);
    background: var(--color-surface-muted);
    outline: 0;
  }

  .quick-action.active {
    color: var(--color-accent-hover);
    background: var(--color-accent-soft);
  }

  .date-action input {
    position: absolute;
    inset: 0;
    min-height: 0;
    padding: 0;
    opacity: 0;
    cursor: pointer;
  }

  .priority-picker {
    position: relative;
  }

  .priority-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 8px);
    z-index: 30;
    display: grid;
    width: 138px;
    gap: 3px;
    padding: 6px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 9px;
    box-shadow: 0 12px 32px color-mix(in oklch, var(--color-text), transparent 86%);
  }

  .priority-menu button {
    display: flex;
    min-height: 34px;
    gap: 9px;
    align-items: center;
    padding: 6px 9px;
    color: var(--color-text-muted);
    background: transparent;
    border: 0;
    border-radius: 6px;
    font-size: 12px;
    text-align: left;
  }

  .priority-menu button:hover,
  .priority-menu button[aria-checked='true'] {
    color: var(--color-text);
    background: var(--color-surface-muted);
  }

  .priority-menu .priority-option {
    color: var(--color-accent-hover);
  }

  .task-create-compact .form-error {
    grid-column: 1 / -1;
  }

  .task-create-options {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: end;
    margin-top: 11px;
  }

  .task-create-options .compact-field {
    flex: 0 1 145px;
  }

  .task-create-options input[type='date'],
  .task-create-options input[type='time'] {
    min-height: 34px;
    padding: 6px 8px;
    font-size: 12px;
  }

  .task-description-field {
    margin-top: 8px;
  }

  .task-add-button {
    margin-left: auto;
  }

  @media (max-width: 767px) {
    .task-create {
      padding: 12px;
    }

    .task-create-options {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .task-create-options .compact-field,
    .task-create-options .priority-toggle {
      min-width: 0;
    }

    .task-add-button {
      grid-column: 1 / -1;
      width: 100%;
      margin-left: 0;
    }

    .task-create-compact {
      padding: 5px 6px 5px 10px;
    }
  }
</style>
