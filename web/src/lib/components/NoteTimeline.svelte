<script lang="ts">
  import type { Note } from '../api/types';
  import { groupNotesByMonth, sortNotesByCreatedAt } from '../utils/date';
  import NoteItem from './NoteItem.svelte';

  let {
    notes,
    timeZone,
    mode = 'active',
    busyUids,
    onSave,
    onPin,
    onArchive,
    onRestore,
    onDelete,
    onTag,
  }: {
    notes: Note[];
    timeZone: string;
    mode?: 'active' | 'archive';
    busyUids: Set<string>;
    onSave: (note: Note, content: string) => Promise<void>;
    onPin?: (note: Note) => Promise<void>;
    onArchive?: (note: Note) => Promise<void>;
    onRestore?: (note: Note) => Promise<void>;
    onDelete: (note: Note) => void;
    onTag?: (tag: string) => void;
  } = $props();

  let pinnedNotes = $derived(sortNotesByCreatedAt(notes.filter((note) => note.pinned)));
  let groups = $derived(
    groupNotesByMonth(
      notes.filter((note) => !note.pinned),
      timeZone,
    ),
  );
</script>

<div class="note-timeline">
  {#if pinnedNotes.length > 0}
    <section class="note-day pinned-notes" aria-labelledby={`pinned-${mode}`}>
      <h2 id={`pinned-${mode}`}>Pinned</h2>
      <div class="note-day-list">
        {#each pinnedNotes as note (note.uid)}
          <NoteItem
            busy={busyUids.has(note.uid)}
            {mode}
            {note}
            {onArchive}
            {onDelete}
            {onPin}
            {onRestore}
            {onSave}
            {onTag}
            {timeZone}
          />
        {/each}
      </div>
    </section>
  {/if}

  {#each groups as group (group.key)}
    <section class="note-day" aria-labelledby={`day-${mode}-${group.key}`}>
      <h2 id={`day-${mode}-${group.key}`}>{group.label}</h2>
      <div class="note-day-list">
        {#each group.items as note (note.uid)}
          <NoteItem
            busy={busyUids.has(note.uid)}
            {mode}
            {note}
            {onArchive}
            {onDelete}
            {onPin}
            {onRestore}
            {onSave}
            {onTag}
            {timeZone}
          />
        {/each}
      </div>
    </section>
  {/each}
</div>

<style>
  .note-timeline {
    animation: content-enter 180ms ease both;
  }

  .note-day > h2 {
    padding: 18px 2px 10px;
    margin: 0;
    color: var(--muted-foreground);
    font-size: 12px;
    font-weight: 650;
    letter-spacing: 0.015em;
  }

  .note-day-list {
    display: grid;
    gap: 9px;
    padding-bottom: 12px;
  }

  @keyframes content-enter {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
