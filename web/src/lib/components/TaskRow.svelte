<script lang="ts">
  import { parseDate, type DateValue } from '@internationalized/date';
  import CalendarIcon from '@lucide/svelte/icons/calendar';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Flag from '@lucide/svelte/icons/flag';
  import Minus from '@lucide/svelte/icons/minus';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import { onMount, tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import type { Task, TaskPriority, UpdateTaskRequest } from '../api/types';
  import { isTaskOverdue, taskDateLabel } from '../utils/date';
  import { Button } from './ui/button';
  import { Calendar } from './ui/calendar';
  import { Checkbox } from './ui/checkbox';
  import * as DropdownMenu from './ui/dropdown-menu';
  import * as Field from './ui/field';
  import * as InputGroup from './ui/input-group';
  import * as Popover from './ui/popover';
  import * as Sheet from './ui/sheet';
  import { Spinner } from './ui/spinner';
  import * as ToggleGroup from './ui/toggle-group';

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

  let editorSurface = $state<'inline' | 'drawer' | null>(null);
  let mobile = $state(false);
  let actionsOpen = $state(false);
  let title = $state('');
  let description = $state('');
  let priority = $state<TaskPriority>(0);
  let dueDate = $state('');
  let datePickerOpen = $state(false);
  let error = $state<string | null>(null);
  let saving = $state(false);
  let rowElement = $state<HTMLElement | null>(null);
  let editForm = $state<HTMLFormElement | null>(null);
  let titleInput = $state<HTMLInputElement | null>(null);
  let descriptionInput = $state<HTMLTextAreaElement | null>(null);
  let titleButton = $state<HTMLButtonElement | null>(null);
  let drawerViewportStyle = $state<string | undefined>(undefined);
  let editing = $derived(editorSurface !== null);
  let dateLabel = $derived(taskDateLabel(task, today));
  let descriptionPreview = $derived(
    mode === 'todo' ? task.description.split(/\r?\n/, 1)[0] : task.description,
  );
  let calendarValue = $derived(dueDate ? parseDate(dueDate) : undefined);
  let calendarPlaceholder = $derived(parseDate(dueDate || today));

  onMount(() => {
    const mobileQuery = window.matchMedia?.('(max-width: 767px)');
    if (!mobileQuery) return;
    const updateMobile = () => (mobile = mobileQuery.matches);
    updateMobile();
    mobileQuery.addEventListener('change', updateMobile);
    return () => mobileQuery.removeEventListener('change', updateMobile);
  });

  $effect(() => {
    if (editorSurface !== 'drawer') {
      drawerViewportStyle = undefined;
      return;
    }
    const visualViewport = window.visualViewport;
    const updateDrawerViewport = () => {
      if (!visualViewport) {
        drawerViewportStyle = undefined;
        return;
      }
      const bottomInset = Math.max(
        0,
        Math.round(window.innerHeight - visualViewport.height - visualViewport.offsetTop),
      );
      const maxHeight = Math.max(0, Math.floor(visualViewport.height * 0.9));
      drawerViewportStyle = `bottom: ${bottomInset}px; max-height: min(42rem, ${maxHeight}px);`;
    };
    updateDrawerViewport();
    visualViewport?.addEventListener('resize', updateDrawerViewport);
    visualViewport?.addEventListener('scroll', updateDrawerViewport);
    window.addEventListener('resize', updateDrawerViewport);

    return () => {
      visualViewport?.removeEventListener('resize', updateDrawerViewport);
      visualViewport?.removeEventListener('scroll', updateDrawerViewport);
      window.removeEventListener('resize', updateDrawerViewport);
    };
  });

  async function beginEdit(): Promise<void> {
    title = task.title;
    description = task.description;
    priority = task.priority;
    dueDate = task.dueDate ?? '';
    datePickerOpen = false;
    error = null;
    actionsOpen = false;
    const nextSurface = mobile ? 'drawer' : 'inline';
    editorSurface = nextSurface;
    await tick();
    if (nextSurface === 'inline') {
      titleInput?.focus();
      titleInput?.select();
    }
  }

  async function closeEditor(restoreFocus = true): Promise<void> {
    datePickerOpen = false;
    editorSurface = null;
    error = null;
    await tick();
    if (restoreFocus) titleButton?.focus();
  }

  async function save(event?: SubmitEvent, restoreFocus = true): Promise<void> {
    event?.preventDefault();
    if (saving) return;
    if (!title.trim()) {
      error = 'Enter a task title.';
      return;
    }

    const payload: UpdateTaskRequest = {
      description: description.trim(),
      dueDate: dueDate || null,
      dueTime: null,
      priority,
      title: title.trim(),
    };
    const unchanged =
      payload.title === task.title &&
      payload.description === task.description &&
      payload.dueDate === task.dueDate &&
      payload.priority === task.priority;
    if (unchanged) {
      await closeEditor(restoreFocus);
      return;
    }

    error = null;
    saving = true;
    try {
      await onSave(task, payload);
      await closeEditor(restoreFocus);
    } catch (cause) {
      error = errorMessage(cause, 'Unable to save the task.');
    } finally {
      saving = false;
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

  function handleTitleKeydown(event: KeyboardEvent): void {
    if (event.isComposing) return;
    handleEditorKeydown(event);
    if (event.defaultPrevented || event.key !== 'Enter') return;
    event.preventDefault();
    descriptionInput?.focus();
  }

  function handleDelete(): void {
    onDelete(task);
  }

  function selectDueDate(value: DateValue | undefined): void {
    dueDate = value?.toString() ?? '';
    datePickerOpen = false;
  }

  function clearDueDate(): void {
    dueDate = '';
    datePickerOpen = false;
  }

  function handleEditorFocusout(event: FocusEvent & { currentTarget: HTMLFormElement }): void {
    if (editorSurface === 'drawer') return;
    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) {
      return;
    }
    queueMicrotask(() => {
      if (!editing || saving || datePickerOpen || editForm?.contains(document.activeElement))
        return;
      void save(undefined, false);
    });
  }

  function handleWindowPointerdown(event: PointerEvent): void {
    if (!editing || busy || saving || !(event.target instanceof Node)) return;
    if (editorSurface === 'drawer') return;
    if (rowElement?.contains(event.target)) return;
    if (event.target instanceof Element && event.target.closest('[data-slot="popover-content"]')) {
      return;
    }
    void save(undefined, false);
  }

  function handleDrawerOpenChange(open: boolean): void {
    if (open || editorSurface !== 'drawer') return;
    void closeEditor();
  }

  function handleDrawerOpenAutoFocus(event: Event): void {
    event.preventDefault();
    const input = titleInput;
    if (!input) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }

  function handleDrawerCloseAutoFocus(event: Event): void {
    event.preventDefault();
    titleButton?.focus();
  }
</script>

<svelte:window onpointerdown={handleWindowPointerdown} />

<article
  bind:this={rowElement}
  class:task-done={task.status === 'DONE'}
  class:task-editing={editorSurface === 'inline'}
  class:task-row-compact={mode === 'todo' && editorSurface !== 'inline' && !task.description}
  class:task-row-full={mode === 'all'}
  class:task-row-todo={mode === 'todo'}
  class="task-row"
  data-focus-uid={task.uid}
  tabindex="-1"
>
  {#snippet taskEditor(surface: 'inline' | 'drawer')}
    <form
      bind:this={editForm}
      class:task-edit-form-drawer={surface === 'drawer'}
      class="task-edit-form"
      onfocusout={handleEditorFocusout}
      onsubmit={save}
    >
      <Field.Field class="gap-2" data-invalid={Boolean(error)}>
        <Field.Label class="sr-only" for={`task-title-${task.uid}`}>Title</Field.Label>
        <InputGroup.Root class="task-edit-copy" layout="stacked" variant="quiet">
          <InputGroup.Input
            aria-invalid={error ? 'true' : undefined}
            bind:ref={titleInput}
            class="task-edit-title h-8 px-3"
            disabled={busy || saving}
            emphasis="title"
            id={`task-title-${task.uid}`}
            maxlength={500}
            onkeydown={handleTitleKeydown}
            bind:value={title}
          />
          <InputGroup.Textarea
            aria-label="Details"
            bind:ref={descriptionInput}
            class="task-edit-details min-h-11 max-h-[min(320px,45vh)] px-3 py-1"
            disabled={busy || saving}
            id={`task-details-${task.uid}`}
            onkeydown={handleEditorKeydown}
            rows={2}
            tone="secondary"
            bind:value={description}
          />
          <InputGroup.Addon align="block-end" class="task-edit-toolbar flex-wrap">
            <Popover.Root bind:open={datePickerOpen}>
              <Popover.Trigger disabled={busy || saving} id={`task-date-${task.uid}`}>
                {#snippet child({ props })}
                  <InputGroup.Button
                    {...props}
                    aria-label={dueDate ? `Due date ${dueDate}` : 'Set due date'}
                    size="xs"
                    title={dueDate ? `Due ${dueDate}` : 'Set due date'}
                    variant={dueDate ? 'secondary' : 'ghost'}
                  >
                    <CalendarIcon data-icon="inline-start" />
                    {#if dueDate}<span class="font-mono text-[11px]">{dueDate}</span>{/if}
                  </InputGroup.Button>
                {/snippet}
              </Popover.Trigger>
              <Popover.Content align="start" class="w-auto gap-0 overflow-hidden p-0">
                <Calendar
                  captionLayout="dropdown"
                  initialFocus
                  onValueChange={selectDueDate}
                  placeholder={calendarPlaceholder}
                  type="single"
                  value={calendarValue}
                />
                {#if dueDate}
                  <div class="flex justify-end px-2 pb-2">
                    <Button onclick={clearDueDate} size="xs" variant="ghost">Clear date</Button>
                  </div>
                {/if}
              </Popover.Content>
            </Popover.Root>
            <ToggleGroup.Root
              aria-label="Task priority"
              disabled={busy || saving}
              onValueChange={(value) => (priority = value === '1' ? 1 : 0)}
              size="xs"
              spacing={0}
              type="single"
              value={String(priority)}
              variant="outline"
            >
              <ToggleGroup.Item aria-label="Regular priority" title="Regular priority" value="0">
                <Minus />
              </ToggleGroup.Item>
              <ToggleGroup.Item aria-label="High priority" title="High priority" value="1">
                <Flag />
              </ToggleGroup.Item>
            </ToggleGroup.Root>
            <div class="task-edit-actions ml-auto flex items-center gap-1">
              <InputGroup.Button
                class="task-edit-action"
                disabled={busy || saving}
                onclick={() => void closeEditor()}
                onkeydown={handleEditorKeydown}
                size="xs"
                variant="ghost">Cancel</InputGroup.Button
              >
              <InputGroup.Button
                class="task-edit-action"
                disabled={busy || saving || !title.trim()}
                onkeydown={handleEditorKeydown}
                size="xs"
                type="submit"
                variant="default"
              >
                {#if busy || saving}<Spinner data-icon="inline-start" />{/if}
                {busy || saving ? 'Saving…' : 'Save'}
              </InputGroup.Button>
            </div>
          </InputGroup.Addon>
        </InputGroup.Root>
        {#if error}<Field.Error aria-live="assertive">{error}</Field.Error>{/if}
      </Field.Field>
    </form>
  {/snippet}

  {#if editorSurface === 'inline'}
    {@render taskEditor('inline')}
  {:else}
    <Checkbox
      aria-label={task.status === 'DONE' ? `Restore ${task.title}` : `Complete ${task.title}`}
      checked={task.status === 'DONE'}
      class="task-checkbox mt-0.5 size-5"
      disabled={busy}
      onclick={() => void onToggle(task)}
    />
    <div class="task-copy">
      <div class="task-title-line">
        <h3>
          <button
            aria-label={`Edit ${task.title}`}
            bind:this={titleButton}
            class="task-title-button"
            disabled={busy}
            onclick={() => void beginEdit()}
            type="button">{task.title}</button
          >
        </h3>
        {#if mode === 'all' && task.priority === 1 && task.status === 'TODO'}
          <span aria-label="Priority task" class="priority-mark" title="Priority">
            <Flag class="size-3" />
            {#if mode === 'all'}<span>Priority</span>{/if}
          </span>
        {/if}
      </div>
      {#if task.status === 'TODO' && descriptionPreview}<p>{descriptionPreview}</p>{/if}
      {#if task.status === 'TODO' && mode === 'all' && dateLabel}
        <span class:overdue={isTaskOverdue(task, today)} class="task-date">{dateLabel}</span>
      {/if}
    </div>
    <div aria-label={`Actions for ${task.title}`} class="row-actions">
      <DropdownMenu.Root onOpenChange={(open) => (actionsOpen = open)} open={actionsOpen}>
        <DropdownMenu.Trigger disabled={busy}>
          {#snippet child({ props })}
            <Button
              {...props}
              aria-label={`More actions for ${task.title}`}
              size="icon-sm"
              variant="ghost"
            >
              <Ellipsis />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        {#if actionsOpen}
          <DropdownMenu.Content align="end" class="w-36" forceMount>
            <DropdownMenu.Group>
              <DropdownMenu.Item
                aria-label={`Delete ${task.title}`}
                disabled={busy}
                onclick={handleDelete}
                variant="destructive"
              >
                <Trash2 />
                Delete
              </DropdownMenu.Item>
            </DropdownMenu.Group>
          </DropdownMenu.Content>
        {/if}
      </DropdownMenu.Root>
    </div>
  {/if}

  <Sheet.Root open={editorSurface === 'drawer'} onOpenChange={handleDrawerOpenChange}>
    <Sheet.Content
      class="task-editor-sheet max-h-[min(90dvh,42rem)] overflow-y-auto rounded-t-xl p-0 pb-[max(1rem,env(safe-area-inset-bottom))]"
      onCloseAutoFocus={handleDrawerCloseAutoFocus}
      onOpenAutoFocus={handleDrawerOpenAutoFocus}
      showCloseButton={false}
      side="bottom"
      style={drawerViewportStyle}
    >
      <Sheet.Header class="px-5 pt-5 pb-2 text-left">
        <Sheet.Title>Edit task</Sheet.Title>
        <Sheet.Description class="sr-only">
          Update the task title, details, due date, and priority.
        </Sheet.Description>
      </Sheet.Header>
      {@render taskEditor('drawer')}
    </Sheet.Content>
  </Sheet.Root>
</article>

<style>
  .task-row {
    position: relative;
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    gap: 7px;
    align-items: start;
    padding: 11px 0;
    border-bottom: 1px solid var(--border);
  }

  .task-row-compact {
    align-items: center;
    padding-block: 6px;
  }

  .task-copy {
    min-width: 0;
  }

  .task-title-line {
    display: flex;
    min-width: 0;
    gap: 8px;
    align-items: baseline;
  }

  .task-title-line h3 {
    min-width: 0;
    flex: 1;
    margin-bottom: 0;
    font-size: 13px;
    font-weight: 560;
    line-height: 20px;
  }

  .task-title-button {
    display: block;
    width: 100%;
    padding: 0;
    color: inherit;
    background: transparent;
    border: 0;
    border-radius: 3px;
    font: inherit;
    line-height: inherit;
    text-align: left;
    overflow-wrap: anywhere;
    cursor: text;
  }

  .task-title-button:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .task-title-button:disabled {
    cursor: default;
  }

  .priority-mark {
    display: inline-flex;
    flex: none;
    gap: 3px;
    align-items: center;
    color: var(--primary);
    font-size: 10px;
    font-weight: 620;
  }

  .task-copy p {
    margin: 3px 0 0;
    color: var(--muted-foreground);
    overflow-wrap: anywhere;
    font-size: 12px;
    line-height: 18px;
    white-space: pre-wrap;
  }

  .task-row-todo {
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 4px;
    padding-block: 8px;
  }

  .task-row-todo .task-title-button,
  .task-row-todo .task-copy p {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task-row-todo .task-copy p {
    margin-top: 1px;
  }

  .task-row-todo .row-actions {
    position: absolute;
    top: 6px;
    right: 0;
    background: var(--card);
  }

  .task-row-todo:hover .task-copy,
  .task-row-todo:focus-within .task-copy {
    padding-right: 32px;
  }

  .task-date {
    display: block;
    margin-top: 4px;
    color: var(--muted-foreground);
    font-size: 11px;
  }

  .task-date.overdue {
    color: var(--destructive);
    font-weight: 580;
  }

  .task-done {
    color: var(--muted-foreground);
  }

  .task-done .task-title-line h3 {
    font-weight: 500;
  }

  .task-done .row-actions {
    color: var(--muted-foreground);
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

  @media (max-width: 767px), (hover: none) {
    .row-actions {
      opacity: 1;
    }
  }

  .task-edit-form {
    grid-column: 1 / -1;
    padding: 7px 0 2px;
    animation: editor-enter 180ms ease both;
  }

  .task-edit-form-drawer {
    grid-column: auto;
    padding: 0 20px;
    animation: none;
  }

  .task-edit-form-drawer :global(.task-edit-title) {
    height: 44px;
    padding-inline: 12px;
    font-size: 16px;
  }

  .task-edit-form-drawer :global(.task-edit-details) {
    min-height: 112px;
    max-height: min(320px, 40dvh);
    padding: 8px 12px;
    font-size: 16px;
    line-height: 24px;
  }

  .task-edit-form-drawer :global(.task-edit-toolbar) {
    gap: 8px;
    padding: 8px 10px 10px;
  }

  .task-edit-form-drawer .task-edit-actions {
    width: 100%;
    justify-content: flex-end;
    padding-top: 2px;
    margin-left: 0;
  }

  .task-edit-form-drawer :global(.task-edit-action) {
    min-height: 44px;
    padding-inline: 14px;
  }

  :global(.task-edit-details) {
    field-sizing: content;
  }

  .task-editing {
    padding-block: 9px 17px;
    border-bottom-color: transparent;
  }

  .task-row-full {
    grid-template-columns: 32px minmax(0, 1fr) 76px;
    padding: 16px 4px;
  }

  .task-row-full .task-title-line h3 {
    font-size: 14px;
  }

  .task-row-full.task-done {
    align-items: center;
    padding-block: 9px;
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

    .task-row-todo .task-copy {
      padding-right: 32px;
    }
  }

  @media (max-width: 767px) {
    .task-row,
    .task-row-full {
      grid-template-columns: 44px minmax(0, 1fr) 44px;
      align-items: start;
    }

    .task-row-compact {
      align-items: center;
      padding-block: 2px;
    }

    .task-row-todo {
      grid-template-columns: 44px minmax(0, 1fr);
      gap: 0;
    }

    .task-row-todo .task-copy {
      padding-right: 44px;
    }

    .task-row-todo .row-actions {
      top: 2px;
    }

    .row-actions {
      display: grid;
    }
  }

  @media (hover: none) {
    .row-actions {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .task-edit-form {
      animation-duration: 0.01ms;
    }

    :global(.task-editor-sheet) {
      transition-duration: 0.01ms;
    }

    .row-actions {
      transition-duration: 0.01ms;
    }
  }
</style>
