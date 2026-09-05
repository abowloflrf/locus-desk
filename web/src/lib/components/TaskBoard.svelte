<script lang="ts">
  import { onDestroy } from 'svelte';

  import { createTask, deleteTask, listTasks, updateTask } from '../api/tasks';
  import { errorMessage } from '../api/client';
  import type { CreateTaskRequest, Task, TaskStatus, UpdateTaskRequest } from '../api/types';
  import { captureListFocus, restoreListFocus, type ListFocusSnapshot } from '../utils/focus';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import StatusMessage from './StatusMessage.svelte';
  import TaskCreateForm from './TaskCreateForm.svelte';
  import TaskRow from './TaskRow.svelte';
  import { Button } from './ui/button';
  import * as Empty from './ui/empty';
  import { Spinner } from './ui/spinner';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  let {
    mode,
    today,
    refreshToken = 0,
  }: {
    mode: 'todo' | 'all';
    today: string;
    refreshToken?: number;
  } = $props();

  let tasks = $state<Task[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let operationError = $state<string | null>(null);
  let actionStatus = $state<string | null>(null);
  let creating = $state(false);
  let busyUids = $state<Set<string>>(new Set());
  let pendingDelete = $state<Task | null>(null);
  let deleteBusy = $state(false);
  let deleteError = $state<string | null>(null);
  let completedOpen = $state(true);
  let requestId = 0;
  let activeController: AbortController | null = null;
  let loadKey = $derived(`${mode}:${refreshToken}`);
  let previousLoadKey = '';
  let boardElement = $state<HTMLElement>();
  let deleteFocusSnapshot: ListFocusSnapshot | null = null;

  let openTasks = $derived(
    tasks
      .filter((task) => task.status === 'TODO')
      .sort((a, b) => {
        return (
          (a.dueDate ?? '9999').localeCompare(b.dueDate ?? '9999') ||
          a.sortKey - b.sortKey ||
          a.createdAt.localeCompare(b.createdAt) ||
          a.uid.localeCompare(b.uid)
        );
      }),
  );
  let completedTasks = $derived(
    tasks
      .filter((task) => task.status === 'DONE')
      .sort((a, b) => (b.completedAt ?? '').localeCompare(a.completedAt ?? '')),
  );

  onDestroy(() => activeController?.abort());

  $effect(() => {
    const nextKey = loadKey;
    if (nextKey === previousLoadKey) return;
    previousLoadKey = nextKey;
    queueMicrotask(() => void loadCollection());
  });

  async function loadCollection(): Promise<void> {
    const id = ++requestId;
    activeController?.abort();
    activeController = new AbortController();
    loading = true;
    loadError = null;

    try {
      const response = await listTasks(
        {
          status: mode === 'todo' ? 'TODO' : undefined,
        },
        activeController.signal,
      );
      if (id === requestId) tasks = response.items;
    } catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') return;
      if (id === requestId) loadError = errorMessage(cause, 'Unable to load tasks.');
    } finally {
      if (id === requestId) loading = false;
    }
  }

  async function handleCreate(payload: CreateTaskRequest): Promise<void> {
    if (creating) return;
    invalidateTaskRequest();
    creating = true;
    operationError = null;
    try {
      const task = await createTask(payload);
      tasks = [...tasks, task];
      actionStatus = `Task added: ${task.title}.`;
      window.dispatchEvent(new CustomEvent('locus:tasks-changed'));
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to add the task.');
      void loadCollection();
      throw cause;
    } finally {
      creating = false;
    }
  }

  async function handleToggle(task: Task): Promise<void> {
    if (busyUids.has(task.uid)) return;
    if (mode === 'all' && task.status === 'TODO') completedOpen = true;
    const focusSnapshot = captureListFocus(boardElement, task.uid);
    invalidateTaskRequest();
    const previous = task;
    const nextStatus: TaskStatus = task.status === 'DONE' ? 'TODO' : 'DONE';
    const optimistic = { ...task, status: nextStatus };
    let focusTask = optimistic;
    markBusy(task.uid, true);
    operationError = null;
    actionStatus = null;
    replaceTask(optimistic);
    await restoreListFocus(
      boardElement,
      focusSnapshot,
      taskMatchesCurrentFilter(optimistic) ? task.uid : null,
    );

    try {
      const updated = await updateTask(task.uid, { status: nextStatus });
      replaceTask(updated);
      focusTask = updated;
      actionStatus =
        mode === 'todo' && !taskMatchesCurrentFilter(updated)
          ? `Task completed and removed from Todo: ${task.title}.`
          : nextStatus === 'DONE'
            ? `Task completed: ${task.title}.`
            : `Task restored: ${task.title}.`;
      window.dispatchEvent(new CustomEvent('locus:tasks-changed'));
    } catch (cause) {
      replaceTask(previous);
      focusTask = previous;
      operationError = errorMessage(cause, 'Unable to update the task.');
      void loadCollection();
    } finally {
      markBusy(task.uid, false);
      await restoreListFocus(
        boardElement,
        focusSnapshot,
        taskMatchesCurrentFilter(focusTask) ? task.uid : null,
      );
    }
  }

  async function handleSave(task: Task, payload: UpdateTaskRequest): Promise<void> {
    const focusSnapshot = captureListFocus(boardElement, task.uid);
    invalidateTaskRequest();
    markBusy(task.uid, true);
    operationError = null;
    actionStatus = null;
    try {
      const updated = await updateTask(task.uid, payload);
      const remainsVisible = taskMatchesCurrentFilter(updated);
      replaceTask(updated);
      if (!remainsVisible) {
        actionStatus =
          mode === 'todo'
            ? `Task updated and removed from Todo: ${task.title}.`
            : `Task updated and removed from the current view: ${task.title}.`;
        await restoreListFocus(boardElement, focusSnapshot);
      } else {
        actionStatus = `Task saved: ${updated.title}.`;
      }
      window.dispatchEvent(new CustomEvent('locus:tasks-changed'));
    } catch (cause) {
      operationError = errorMessage(cause, 'Unable to save the task.');
      void loadCollection();
      throw cause;
    } finally {
      markBusy(task.uid, false);
    }
  }

  async function confirmDelete(): Promise<void> {
    if (!pendingDelete) return;
    const task = pendingDelete;
    invalidateTaskRequest();
    deleteBusy = true;
    deleteError = null;
    try {
      await deleteTask(task.uid);
      tasks = tasks.filter((item) => item.uid !== task.uid);
      pendingDelete = null;
      deleteError = null;
      actionStatus = `Task deleted: ${task.title}.`;
      window.dispatchEvent(new CustomEvent('locus:tasks-changed'));
    } catch (cause) {
      deleteError = errorMessage(cause, 'Unable to delete the task.');
      void loadCollection();
    } finally {
      deleteBusy = false;
    }
  }

  function requestDelete(task: Task): void {
    deleteFocusSnapshot = captureListFocus(boardElement, task.uid);
    deleteError = null;
    actionStatus = null;
    pendingDelete = task;
  }

  function cancelDelete(): void {
    deleteFocusSnapshot = null;
    deleteError = null;
    pendingDelete = null;
  }

  async function focusAfterDelete(): Promise<void> {
    const snapshot = deleteFocusSnapshot;
    deleteFocusSnapshot = null;
    if (snapshot) await restoreListFocus(boardElement, snapshot);
    else document.getElementById('main-content')?.focus();
  }

  function replaceTask(task: Task): void {
    if (!taskMatchesCurrentFilter(task)) {
      tasks = tasks.filter((item) => item.uid !== task.uid);
      return;
    }

    tasks = tasks.some((item) => item.uid === task.uid)
      ? tasks.map((item) => (item.uid === task.uid ? task : item))
      : [...tasks, task];
  }

  function taskMatchesCurrentFilter(task: Task): boolean {
    if (mode === 'todo') {
      return task.status === 'TODO';
    }
    return true;
  }

  function markBusy(uid: string, busy: boolean): void {
    const next = new Set(busyUids);
    if (busy) next.add(uid);
    else next.delete(uid);
    busyUids = next;
  }

  function invalidateTaskRequest(): void {
    requestId += 1;
    activeController?.abort();
    activeController = null;
    loading = false;
  }
</script>

<div
  aria-busy={loading}
  bind:this={boardElement}
  class:full-task-board={mode === 'all'}
  class:todo-task-board={mode === 'todo'}
  class="task-board"
>
  {#if mode === 'all'}
    <header class="page-header">
      <h1>Tasks</h1>
    </header>
  {/if}

  <TaskCreateForm busy={creating} {mode} {today} onCreate={handleCreate} />

  {#if operationError}
    <StatusMessage tone="error">{operationError}</StatusMessage>
  {/if}
  <div aria-atomic="true" aria-live="polite" class="sr-only" data-action-status role="status">
    {actionStatus ?? ''}
  </div>

  {#if loading && tasks.length === 0}
    <div aria-live="polite" class="loading-state flex items-center justify-center gap-2">
      <Spinner />
      Loading tasks…
    </div>
  {:else if loadError}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>Unable to load tasks</Empty.Title>
        <Empty.Description>{loadError}</Empty.Description>
      </Empty.Header>
      <Empty.Content>
        <Button onclick={() => void loadCollection()} variant="secondary">Try again</Button>
      </Empty.Content>
    </Empty.Root>
  {:else if tasks.length === 0}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>{mode === 'todo' ? 'No open tasks' : 'No tasks in this view'}</Empty.Title>
        <Empty.Description
          >{mode === 'todo'
            ? 'Add one small next step to get started.'
            : 'Create a task to get started.'}</Empty.Description
        >
      </Empty.Header>
    </Empty.Root>
  {:else}
    <div class="task-list" aria-label="Open tasks">
      {#each openTasks as task (task.uid)}
        <TaskRow
          busy={busyUids.has(task.uid)}
          {mode}
          onDelete={requestDelete}
          onSave={handleSave}
          onToggle={handleToggle}
          {task}
          {today}
        />
      {/each}
    </div>
    {#if mode === 'all' && completedTasks.length > 0}
      <section class="completed-section" aria-labelledby={`completed-${mode}`}>
        <h2 id={`completed-${mode}`}>
          <button
            class="completed-toggle"
            aria-expanded={completedOpen}
            aria-controls={`completed-list-${mode}`}
            onclick={() => (completedOpen = !completedOpen)}
          >
            {#if completedOpen}<ChevronDown />{:else}<ChevronRight />{/if}
            Completed <span>{completedTasks.length}</span>
          </button>
        </h2>
        {#if completedOpen}
          <div class="task-list" id={`completed-list-${mode}`}>
            {#each completedTasks as task (task.uid)}
              <TaskRow
                busy={busyUids.has(task.uid)}
                {mode}
                onDelete={requestDelete}
                onSave={handleSave}
                onToggle={handleToggle}
                {task}
                {today}
              />
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  {/if}
</div>

<ConfirmDialog
  busy={deleteBusy}
  confirmLabel="Delete task"
  error={deleteError}
  message={pendingDelete ? `“${pendingDelete.title}” will be permanently deleted.` : ''}
  onCancel={cancelDelete}
  onConfirm={confirmDelete}
  onFallbackFocus={focusAfterDelete}
  open={Boolean(pendingDelete)}
  title="Delete this task?"
/>

<style>
  .task-board {
    min-width: 0;
  }
  .todo-task-board {
    min-height: 0;
  }
  .task-list {
    display: grid;
    gap: 4px;
  }
  .completed-section {
    margin-top: 32px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .completed-section h2 {
    margin: 0 0 8px;
  }
  .completed-toggle {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 4px 12px;
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--foreground);
    font-size: 14px;
    font-weight: 550;
    cursor: pointer;
  }
  .completed-toggle:hover {
    background: var(--muted);
  }
  .completed-toggle span {
    color: var(--muted-foreground);
    font-size: 12px;
    font-weight: 400;
  }
  .completed-toggle :global(svg) {
    width: 16px;
    height: 16px;
    color: var(--muted-foreground);
  }
</style>
