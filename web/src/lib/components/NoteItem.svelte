<script lang="ts">
  import { tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import type { Note } from '../api/types';
  import { formatNoteTimestamp } from '../utils/date';
  import Icon from './Icon.svelte';
  import MarkdownContent from './MarkdownContent.svelte';

  let {
    note,
    timeZone,
    mode = 'active',
    busy,
    onSave,
    onPin,
    onArchive,
    onRestore,
    onDelete,
    onTag,
  }: {
    note: Note;
    timeZone: string;
    mode?: 'active' | 'archive';
    busy: boolean;
    onSave: (note: Note, content: string) => Promise<void>;
    onPin?: (note: Note) => Promise<void>;
    onArchive?: (note: Note) => Promise<void>;
    onRestore?: (note: Note) => Promise<void>;
    onDelete: (note: Note) => void;
    onTag?: (tag: string) => void;
  } = $props();

  let editing = $state(false);
  let draft = $state('');
  let editError = $state<string | null>(null);
  let actionsOpen = $state(false);
  let actionsElement = $state<HTMLDivElement>();
  let editor = $state<HTMLTextAreaElement>();
  let editButton = $state<HTMLButtonElement>();
  let moreButton = $state<HTMLButtonElement>();

  async function beginEdit(): Promise<void> {
    draft = note.content;
    editError = null;
    editing = true;
    await tick();
    resizeEditor();
    editor?.focus();
    editor?.setSelectionRange(draft.length, draft.length);
  }

  async function closeEditor(): Promise<void> {
    editing = false;
    editError = null;
    await tick();
    editButton?.focus();
  }

  async function save(): Promise<void> {
    const submittedDraft = draft;
    if (!submittedDraft.trim()) {
      editError = 'A memo cannot be empty.';
      return;
    }

    editError = null;
    try {
      await onSave(note, submittedDraft);
      await closeEditor();
    } catch (cause) {
      editError = errorMessage(cause, 'Unable to save the memo.');
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

  function resizeEditor(): void {
    if (!editor) return;
    editor.style.height = 'auto';
    editor.style.height = `${Math.min(Math.max(editor.scrollHeight, 48), 280)}px`;
    editor.style.overflowY = editor.scrollHeight > 280 ? 'auto' : 'hidden';
  }

  function closeActions(restoreFocus = false): void {
    actionsOpen = false;
    if (restoreFocus) requestAnimationFrame(() => moreButton?.focus());
  }

  function handleActionBlur(event: FocusEvent): void {
    if (event.relatedTarget instanceof Node && actionsElement?.contains(event.relatedTarget)) {
      return;
    }
    closeActions();
  }

  function handleActionsKeydown(event: KeyboardEvent): void {
    if (actionsOpen && event.key === 'Escape') {
      event.preventDefault();
      closeActions(true);
    }
  }

  async function handlePin(): Promise<void> {
    const restoreMenuFocus = actionsOpen;
    await onPin?.(note);
    closeActions(restoreMenuFocus);
  }

  async function handleArchive(): Promise<void> {
    closeActions();
    await onArchive?.(note);
  }

  async function handleRestore(): Promise<void> {
    closeActions();
    await onRestore?.(note);
  }

  function handleDelete(): void {
    closeActions();
    onDelete(note);
  }
</script>

<article
  class:editing
  class:note-pinned={note.pinned}
  class="note-item"
  data-focus-uid={note.uid}
  tabindex="-1"
>
  <time datetime={note.createdAt}>{formatNoteTimestamp(note.createdAt, timeZone)}</time>
  <div class="note-body">
    {#if editing}
      <div class="note-edit-form">
        <label class="sr-only" for={`edit-note-${note.uid}`}>Edit memo</label>
        <textarea
          bind:this={editor}
          disabled={busy}
          id={`edit-note-${note.uid}`}
          oninput={resizeEditor}
          onkeydown={handleEditorKeydown}
          rows="1"
          bind:value={draft}></textarea>
        {#if editError}<p aria-live="assertive" class="form-error">{editError}</p>{/if}
        <div class="inline-form-actions">
          <button
            class="button secondary"
            disabled={busy}
            onclick={() => void closeEditor()}
            type="button">Cancel</button
          >
          <button
            class="button primary"
            disabled={busy || !draft.trim()}
            onclick={() => void save()}
            type="button"
          >
            {busy ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    {:else}
      <MarkdownContent content={note.content} />
      {#if note.tags.length > 0}
        <div aria-label="Tags" class="note-tags">
          {#each note.tags as tag}
            <button class="tag-chip" disabled={!onTag} onclick={() => onTag?.(tag)} type="button"
              >#{tag}</button
            >
          {/each}
        </div>
      {/if}
    {/if}
  </div>
  {#if !editing}
    <div
      aria-label="Memo actions"
      bind:this={actionsElement}
      class:menu-open={actionsOpen}
      class="note-actions"
      role="group"
    >
      <button
        aria-expanded={actionsOpen}
        aria-label="More memo actions"
        bind:this={moreButton}
        class="icon-button note-more"
        disabled={busy}
        onblur={handleActionBlur}
        onclick={() => (actionsOpen = !actionsOpen)}
        onkeydown={handleActionsKeydown}
        type="button"><Icon name="more" size={18} /></button
      >
      <div class="note-action-buttons">
        {#if mode === 'active'}
          <button
            aria-label={note.pinned ? 'Unpin memo' : 'Pin memo'}
            aria-pressed={note.pinned}
            class:active={note.pinned}
            class="icon-button"
            disabled={busy}
            onblur={handleActionBlur}
            onclick={() => void handlePin()}
            onkeydown={handleActionsKeydown}
            type="button"
          >
            <Icon name="pin" size={16} />
            <span class="action-label">{note.pinned ? 'Unpin' : 'Pin'}</span>
          </button>
        {/if}
        <button
          aria-label="Edit memo"
          bind:this={editButton}
          class="icon-button"
          disabled={busy}
          onblur={handleActionBlur}
          onclick={() => void beginEdit()}
          onkeydown={handleActionsKeydown}
          type="button"
        >
          <Icon name="edit" size={16} />
          <span class="action-label">Edit</span>
        </button>
        {#if mode === 'active'}
          <button
            aria-label="Archive memo"
            class="icon-button"
            disabled={busy}
            onblur={handleActionBlur}
            onclick={() => void handleArchive()}
            onkeydown={handleActionsKeydown}
            type="button"
          >
            <Icon name="archive" size={16} />
            <span class="action-label">Archive</span>
          </button>
        {:else}
          <button
            aria-label="Restore memo"
            class="icon-button"
            disabled={busy}
            onblur={handleActionBlur}
            onclick={() => void handleRestore()}
            onkeydown={handleActionsKeydown}
            type="button"
          >
            <Icon name="restore" size={16} />
            <span class="action-label">Restore</span>
          </button>
        {/if}
        <button
          aria-label="Delete memo"
          class="icon-button danger-quiet"
          disabled={busy}
          onblur={handleActionBlur}
          onclick={handleDelete}
          onkeydown={handleActionsKeydown}
          type="button"
        >
          <Icon name="delete" size={16} />
          <span class="action-label">Delete</span>
        </button>
      </div>
    </div>
  {/if}
</article>

<style>
  .note-item {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
    align-items: start;
    padding: 14px 14px 15px;
    overflow: visible;
    background: color-mix(in oklch, var(--color-surface), var(--color-canvas) 16%);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-surface);
    transition:
      opacity 150ms ease,
      border-color 150ms ease,
      box-shadow 150ms ease;
  }

  .note-item:hover,
  .note-item:focus-visible,
  .note-item:has(:focus-visible) {
    border-color: color-mix(in oklch, var(--color-accent), var(--color-border) 64%);
    box-shadow: var(--shadow-soft);
  }

  .note-item.editing,
  .note-item.editing:hover,
  .note-item.editing:focus-visible,
  .note-item.editing:has(:focus-visible) {
    border-color: color-mix(in oklch, var(--color-accent), var(--color-border) 48%);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-accent), transparent 88%);
  }

  .note-item.note-pinned::before {
    position: absolute;
    top: 14px;
    left: 0;
    width: 3px;
    height: 24px;
    background: var(--color-accent);
    border-radius: 0 2px 2px 0;
    content: '';
  }

  .note-item > time {
    display: inline-flex;
    width: max-content;
    grid-column: 1;
    grid-row: 1;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 20px;
    opacity: 0.76;
  }

  .note-body {
    min-width: 0;
    grid-column: 1 / -1;
    grid-row: 2;
  }

  .note-actions {
    position: absolute;
    top: 7px;
    right: 8px;
    display: flex;
    gap: 1px;
    opacity: 0;
    pointer-events: none;
    transform: translateY(-4px);
    transition:
      opacity 140ms ease,
      transform 140ms ease;
  }

  .note-action-buttons {
    display: flex;
    gap: 1px;
  }

  .note-more,
  .action-label {
    display: none;
  }

  .note-item:hover .note-actions,
  .note-item:focus-within .note-actions {
    opacity: 1;
    pointer-events: auto;
    transform: translateY(0);
  }

  .note-actions .icon-button {
    width: 28px;
    height: 28px;
    border-radius: 6px;
  }

  .note-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 12px;
  }

  .note-tags .tag-chip {
    min-height: 25px;
    padding-block: 2px;
  }

  .note-edit-form textarea {
    min-height: 48px;
    max-height: 280px;
    padding: 2px 0 6px;
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    font-family: var(--font-ui);
    font-size: 15px;
    line-height: 24px;
    resize: none;
  }

  .note-edit-form textarea:focus {
    border: 0;
    box-shadow: none;
  }

  .note-edit-form .inline-form-actions {
    margin-top: 8px;
  }

  @media (max-width: 767px) {
    .note-item {
      gap: 3px;
      padding: 12px 13px 14px;
    }

    .note-item > time {
      line-height: 20px;
    }

    .note-tags .tag-chip {
      min-height: 44px;
    }

    .note-edit-form textarea {
      font-size: 16px;
    }
  }

  @media (max-width: 767px), (hover: none) {
    .note-actions {
      opacity: 1;
      pointer-events: auto;
      transform: none;
    }

    .note-more {
      display: inline-grid;
    }

    .note-action-buttons {
      position: absolute;
      top: calc(100% + 6px);
      right: 0;
      z-index: 30;
      display: grid;
      width: 148px;
      gap: 2px;
      padding: 6px;
      background: var(--color-surface);
      border: 1px solid var(--color-border-soft);
      border-radius: var(--radius-input);
      box-shadow: var(--shadow-floating);
      opacity: 0;
      pointer-events: none;
      transform: translateY(-4px);
      visibility: hidden;
      transition:
        opacity 120ms ease,
        transform 140ms ease,
        visibility 120ms step-end;
    }

    .note-actions.menu-open .note-action-buttons {
      opacity: 1;
      pointer-events: auto;
      transform: translateY(0);
      visibility: visible;
      transition:
        opacity 120ms ease,
        transform 140ms ease,
        visibility 0ms step-start;
    }

    .note-action-buttons .icon-button {
      display: flex;
      width: 100%;
      height: 44px;
      min-width: 0;
      min-height: 44px;
      gap: 9px;
      justify-content: flex-start;
      padding: 0 9px;
    }

    .action-label {
      display: inline;
      font-size: 12px;
      font-weight: 560;
    }
  }
</style>
