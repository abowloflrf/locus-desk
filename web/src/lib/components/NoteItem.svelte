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
  import { Badge } from './ui/badge';
  import { Button } from './ui/button';
  import * as Field from './ui/field';
  import * as Dialog from './ui/dialog';
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

  const previewHeight = 240;
  let readerOpen = $state(false);
  let readerContent = $state<HTMLDivElement | null>(null);
  let readerButton = $state<HTMLButtonElement | null>(null);
  let overflowing = $state(false);

  function measurePreview(node: HTMLElement) {
    const measure = () => {
      overflowing = node.scrollHeight > previewHeight + 1;
    };
    const frame = requestAnimationFrame(measure);
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    observer?.observe(node);
    window.addEventListener('resize', measure);
    return {
      destroy() {
        cancelAnimationFrame(frame);
        observer?.disconnect();
        window.removeEventListener('resize', measure);
      },
    };
  }

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
  class="note-item list-action-row"
  data-actions-open={actionsOpen}
  data-focus-uid={note.uid}
  tabindex="-1"
>
  <div class="note-meta">
    <time datetime={note.createdAt}>{formatNoteTimestamp(note.createdAt, timeZone)}</time>
  </div>
  <div class="note-body">
    {#if editing}
      <div class="note-edit-form">
        <MarkdownEditor
          disabled={busy}
          id={`edit-note-${note.uid}`}
          label="Edit memo"
          invalid={Boolean(editError)}
          describedBy={editError ? `edit-note-error-${note.uid}` : undefined}
          onCancel={() => void closeEditor()}
          onSave={() => void save()}
          bind:value={draft}
        />
        {#if editError}
          <Field.Error class="mt-2" id={`edit-note-error-${note.uid}`}>{editError}</Field.Error>
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
      <div
        class="note-preview"
        class:collapsed={overflowing}
        id={`memo-content-${note.uid}`}
        style:--preview-height={`${previewHeight}px`}
        onfocusin={() => {
          if (overflowing) readerOpen = true;
        }}
      >
        <div use:measurePreview>
          <MarkdownContent content={note.content} />
        </div>
      </div>
      {#if overflowing}
        <div class="memo-more">
          <Button
            class="memo-expand"
            bind:ref={readerButton}
            variant="ghost"
            size="sm"
            aria-haspopup="dialog"
            onclick={() => (readerOpen = true)}>More</Button
          >
        </div>
      {/if}
      {#if note.tags.length > 0}
        <div aria-label="Tags" class="note-tags">
          {#each note.tags as tag}
            <Button
              aria-label={`Filter by tag ${tag}`}
              class="tag-chip pointer-coarse:min-w-11 px-0"
              disabled={!onTag}
              onclick={() => onTag?.(tag)}
              size="xs"
              variant="ghost"
            >
              <Badge variant="tag">#{tag}</Badge>
            </Button>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
  {#if !editing}
    <div aria-label="Memo actions" class="note-actions list-action-group">
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

<Dialog.Root bind:open={readerOpen}>
  <Dialog.Content
    class="memo-dialog flex max-h-[calc(100dvh-2rem)] flex-col gap-4 overflow-hidden sm:max-w-[min(56rem,calc(100%-2rem))]"
    onOpenAutoFocus={(event) => {
      event.preventDefault();
      readerContent?.focus();
    }}
    onCloseAutoFocus={(event) => {
      event.preventDefault();
      readerButton?.focus();
    }}
  >
    <Dialog.Header class="shrink-0 pr-8 text-left">
      <Dialog.Title class="sr-only">Memo</Dialog.Title>
      <Dialog.Description>{formatNoteTimestamp(note.createdAt, timeZone)}</Dialog.Description>
    </Dialog.Header>
    <div
      class="memo-reader min-h-0 overflow-y-auto overscroll-contain pr-2"
      bind:this={readerContent}
      tabindex="-1"
      role="region"
      aria-label="Full memo content"
    >
      <MarkdownContent content={note.content} />
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  .note-item {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
    align-items: start;
    padding: 12px;
    overflow: visible;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    transition:
      opacity 150ms ease,
      border-color 150ms ease;
  }

  .note-item:focus-visible,
  .note-item:has(:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: 4px;
  }

  .note-item.editing,
  .note-item.editing:hover,
  .note-item.editing:focus-visible,
  .note-item.editing:has(:focus-visible) {
    background: var(--background);
    border-color: var(--ring);
  }

  .note-pinned::before {
    position: absolute;
    top: 14px;
    left: 5px;
    width: 3px;
    height: 16px;
    background: var(--foreground);
    border-radius: 2px;
    content: '';
    pointer-events: none;
  }

  .note-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    min-height: 20px;
    padding-right: 36px;
    margin-bottom: 4px;
  }

  .memo-more {
    display: flex;
    justify-content: center;
    margin-top: 8px;
  }

  .note-preview.collapsed {
    max-height: var(--preview-height);
    overflow: hidden;
    mask-image: linear-gradient(to bottom, black calc(100% - 48px), transparent);
  }

  .note-meta time {
    display: inline-flex;
    width: max-content;
    grid-column: 1;
    grid-row: 1;
    color: var(--muted-foreground);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 20px;
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
  }

  .note-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 8px;
  }

  .note-edit-form .inline-form-actions {
    margin-top: 8px;
  }

  @media (max-width: 767px) {
    .note-item {
      gap: 3px;
      padding: 12px;
    }

    .note-meta time {
      line-height: 20px;
    }
  }

  @media (max-width: 767px), (pointer: coarse) {
    .note-body :global(.memo-expand) {
      min-height: 44px;
      min-width: 44px;
    }
  }
</style>
