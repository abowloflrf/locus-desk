<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Pin from '@lucide/svelte/icons/pin';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import { tick } from 'svelte';

  import { errorMessage } from '../api/client';
  import type { Note } from '../api/types';
  import { formatNoteTimestamp } from '../utils/date';
  import MarkdownEditor from './MarkdownEditor.svelte';
  import MarkdownContent from './MarkdownContent.svelte';
  import { Button } from './ui/button';
  import * as DropdownMenu from './ui/dropdown-menu';
  import { Spinner } from './ui/spinner';

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
  let actionsOpen = $state(false);
  let draft = $state('');
  let editError = $state<string | null>(null);
  let moreButton = $state<HTMLButtonElement | null>(null);

  function beginEdit(): void {
    draft = note.content;
    editError = null;
    editing = true;
  }

  async function closeEditor(): Promise<void> {
    editing = false;
    editError = null;
    await tick();
    moreButton?.focus();
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

  async function handlePin(): Promise<void> {
    await onPin?.(note);
  }

  async function handleArchive(): Promise<void> {
    await onArchive?.(note);
  }

  async function handleRestore(): Promise<void> {
    await onRestore?.(note);
  }

  function handleDelete(): void {
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
        <MarkdownEditor
          disabled={busy}
          id={`edit-note-${note.uid}`}
          label="Edit memo"
          onCancel={() => void closeEditor()}
          onSave={() => void save()}
          bind:value={draft}
        />
        {#if editError}
          <p aria-live="assertive" class="form-error" id={`edit-note-error-${note.uid}`}>
            {editError}
          </p>
        {/if}
        <div class="inline-form-actions">
          <Button disabled={busy} onclick={() => void closeEditor()} variant="secondary"
            >Cancel</Button
          >
          <Button disabled={busy || !draft.trim()} onclick={() => void save()}>
            {#if busy}<Spinner data-icon="inline-start" />{/if}
            {busy ? 'Saving…' : 'Save'}
          </Button>
        </div>
      </div>
    {:else}
      <MarkdownContent content={note.content} />
      {#if note.tags.length > 0}
        <div aria-label="Tags" class="note-tags">
          {#each note.tags as tag}
            <Button
              class="tag-chip"
              disabled={!onTag}
              onclick={() => onTag?.(tag)}
              size="xs"
              variant="secondary">#{tag}</Button
            >
          {/each}
        </div>
      {/if}
    {/if}
  </div>
  {#if !editing}
    <div aria-label="Memo actions" class="note-actions">
      <DropdownMenu.Root onOpenChange={(open) => (actionsOpen = open)} open={actionsOpen}>
        <DropdownMenu.Trigger disabled={busy}>
          {#snippet child({ props })}
            <Button
              {...props}
              aria-label="More memo actions"
              bind:ref={moreButton}
              size="icon-sm"
              variant="ghost"
            >
              <Ellipsis />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        {#if actionsOpen}
          <DropdownMenu.Content align="end" class="w-40" forceMount>
            <DropdownMenu.Group>
              {#if mode === 'active'}
                <DropdownMenu.Item
                  aria-label={note.pinned ? 'Unpin memo' : 'Pin memo'}
                  disabled={busy}
                  onclick={() => void handlePin()}
                >
                  <Pin />
                  {note.pinned ? 'Unpin' : 'Pin'}
                </DropdownMenu.Item>
              {/if}
              <DropdownMenu.Item
                aria-label="Edit memo"
                disabled={busy}
                onclick={() => void beginEdit()}
              >
                <Pencil />
                Edit
              </DropdownMenu.Item>
              {#if mode === 'active'}
                <DropdownMenu.Item
                  aria-label="Archive memo"
                  disabled={busy}
                  onclick={() => void handleArchive()}
                >
                  <Archive />
                  Archive
                </DropdownMenu.Item>
              {:else}
                <DropdownMenu.Item
                  aria-label="Restore memo"
                  disabled={busy}
                  onclick={() => void handleRestore()}
                >
                  <RotateCcw />
                  Restore
                </DropdownMenu.Item>
              {/if}
            </DropdownMenu.Group>
            <DropdownMenu.Separator />
            <DropdownMenu.Item
              aria-label="Delete memo"
              disabled={busy}
              onclick={handleDelete}
              variant="destructive"
            >
              <Trash2 />
              Delete
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        {/if}
      </DropdownMenu.Root>
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
    background: color-mix(in oklch, var(--card), var(--background) 16%);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    transition:
      opacity 150ms ease,
      border-color 150ms ease,
      box-shadow 150ms ease;
  }

  .note-item:hover,
  .note-item:focus-visible,
  .note-item:has(:focus-visible) {
    border-color: color-mix(in oklch, var(--primary), var(--border) 64%);
    box-shadow: var(--shadow-sm);
  }

  .note-item.editing,
  .note-item.editing:hover,
  .note-item.editing:focus-visible,
  .note-item.editing:has(:focus-visible) {
    border-color: color-mix(in oklch, var(--primary), var(--border) 48%);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--primary), transparent 88%);
  }

  .note-item.note-pinned::before {
    position: absolute;
    top: 14px;
    left: 0;
    width: 3px;
    height: 24px;
    background: var(--primary);
    border-radius: 0 2px 2px 0;
    content: '';
  }

  .note-item > time {
    display: inline-flex;
    width: max-content;
    grid-column: 1;
    grid-row: 1;
    color: var(--muted-foreground);
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

  .note-item:hover .note-actions,
  .note-item:focus-within .note-actions {
    opacity: 1;
    pointer-events: auto;
    transform: translateY(0);
  }

  .note-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 12px;
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
  }

  @media (max-width: 767px), (hover: none) {
    .note-actions {
      opacity: 1;
      pointer-events: auto;
      transform: none;
    }
  }
</style>
