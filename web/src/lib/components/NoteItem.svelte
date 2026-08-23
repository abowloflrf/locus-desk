<script lang="ts">
  import { tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import type { Note } from '../api/types';
  import { formatNoteTime } from '../utils/date';
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
  let editor = $state<HTMLTextAreaElement>();
  let editButton = $state<HTMLButtonElement>();

  async function beginEdit(): Promise<void> {
    draft = note.content;
    editError = null;
    editing = true;
    await tick();
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
      editError = 'A note cannot be empty.';
      return;
    }

    editError = null;
    try {
      await onSave(note, submittedDraft);
      await closeEditor();
    } catch (cause) {
      editError = errorMessage(cause, 'Unable to save the note.');
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

<article class:note-pinned={note.pinned} class="note-item" data-focus-uid={note.uid} tabindex="-1">
  <time datetime={note.createdAt}>{formatNoteTime(note.createdAt, timeZone)}</time>
  <div class="note-body">
    {#if editing}
      <div class="note-edit-form">
        <label class="sr-only" for={`edit-note-${note.uid}`}>Edit note</label>
        <textarea
          bind:this={editor}
          disabled={busy}
          id={`edit-note-${note.uid}`}
          onkeydown={handleEditorKeydown}
          rows="7"
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
  <div class="note-actions">
    {#if mode === 'active'}
      <button
        aria-label={note.pinned ? 'Unpin note' : 'Pin note'}
        aria-pressed={note.pinned}
        class:active={note.pinned}
        class="icon-button"
        disabled={busy}
        onclick={() => void onPin?.(note)}
        type="button"><Icon name="pin" size={16} /></button
      >
    {/if}
    <button
      aria-label="Edit note"
      bind:this={editButton}
      class="icon-button"
      disabled={busy}
      onclick={() => void beginEdit()}
      type="button"
    >
      <Icon name="edit" size={16} />
    </button>
    {#if mode === 'active'}
      <button
        aria-label="Archive note"
        class="icon-button"
        disabled={busy}
        onclick={() => void onArchive?.(note)}
        type="button"><Icon name="archive" size={16} /></button
      >
    {:else}
      <button
        aria-label="Restore note"
        class="icon-button"
        disabled={busy}
        onclick={() => void onRestore?.(note)}
        type="button"><Icon name="restore" size={16} /></button
      >
    {/if}
    <button
      aria-label="Delete note"
      class="icon-button danger-quiet"
      disabled={busy}
      onclick={() => onDelete(note)}
      type="button"><Icon name="delete" size={16} /></button
    >
  </div>
</article>

<style>
  .note-item {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px 16px;
    align-items: start;
    padding: 15px 16px 17px;
    overflow: hidden;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 11px;
    box-shadow: 0 1px 2px color-mix(in oklch, var(--color-text), transparent 96%);
    transition:
      opacity 150ms ease,
      border-color 150ms ease,
      box-shadow 150ms ease,
      transform 150ms ease;
  }

  .note-item:hover,
  .note-item:focus-within {
    border-color: color-mix(in oklch, var(--color-accent), var(--color-border) 54%);
    box-shadow: 0 8px 24px color-mix(in oklch, var(--color-text), transparent 94%);
    transform: translateY(-1px);
  }

  .note-item.note-pinned::before {
    position: absolute;
    top: 15px;
    left: 0;
    width: 3px;
    height: 24px;
    background: var(--color-accent);
    border-radius: 2px;
    content: '';
  }

  .note-item > time {
    grid-column: 1;
    grid-row: 1;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 28px;
  }

  .note-body {
    min-width: 0;
    grid-column: 1 / -1;
    grid-row: 2;
  }

  .note-actions {
    display: flex;
    grid-column: 2;
    grid-row: 1;
    gap: 1px;
    margin: -3px -5px 0 0;
    opacity: 0;
    transform: translateY(-4px);
    transition:
      opacity 140ms ease,
      transform 140ms ease;
  }

  .note-item:hover .note-actions,
  .note-item:focus-within .note-actions {
    opacity: 1;
    transform: translateY(0);
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
    min-height: 150px;
    font-family: var(--font-mono);
    font-size: 13px;
  }

  @media (max-width: 1199px) {
    .note-actions {
      opacity: 1;
      transform: none;
    }
  }

  @media (max-width: 767px) {
    .note-item {
      gap: 8px;
      padding: 13px 13px 15px;
    }

    .note-item > time {
      line-height: 34px;
    }

    .note-actions {
      flex-direction: column;
    }
  }

  @media (hover: none) {
    .note-actions {
      opacity: 1;
      transform: none;
    }
  }
</style>
