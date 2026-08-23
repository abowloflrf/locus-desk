<script lang="ts">
  import { errorMessage } from '../api/client';
  import type { Note } from '../api/types';

  let { onCreate }: { onCreate: (content: string) => Promise<Note> } = $props();

  let content = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let textarea = $state<HTMLTextAreaElement>();

  async function submit(): Promise<void> {
    if (busy) return;
    const submittedContent = content;
    const trimmed = content.trim();
    if (!trimmed) {
      error = 'Write something before posting.';
      textarea?.focus();
      return;
    }

    busy = true;
    error = null;
    try {
      await onCreate(submittedContent);
      if (content === submittedContent) content = '';
      resizeTextarea();
      textarea?.focus();
    } catch (cause) {
      error = errorMessage(cause, 'Unable to create the note.');
    } finally {
      busy = false;
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      void submit();
    }
  }

  function resizeTextarea(): void {
    if (!textarea) return;
    textarea.style.height = 'auto';
    textarea.style.height = `${Math.min(Math.max(textarea.scrollHeight, 112), 360)}px`;
  }
</script>

<section class="note-composer" aria-labelledby="composer-title">
  <h2 class="sr-only" id="composer-title">Create a note</h2>
  <label class="sr-only" for="note-composer-input">Markdown note</label>
  <textarea
    aria-describedby={error ? 'composer-error composer-hint' : 'composer-hint'}
    bind:this={textarea}
    bind:value={content}
    disabled={busy}
    id="note-composer-input"
    oninput={resizeTextarea}
    onkeydown={handleKeydown}
    placeholder="Write a quick note… Markdown is supported."
    rows="5"></textarea>
  <div class="composer-footer">
    <div class="composer-submit">
      <span id="composer-hint"><kbd>⌘</kbd><kbd>↵</kbd></span>
      <button
        class="button primary"
        disabled={busy || !content.trim()}
        onclick={() => void submit()}
        type="button"
      >
        {busy ? 'Posting…' : 'Post note'}
      </button>
    </div>
  </div>
  {#if error}<p aria-live="assertive" class="form-error" id="composer-error">{error}</p>{/if}
</section>

<style>
  .note-composer {
    padding: 18px 18px 14px;
    margin-bottom: 22px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 12px;
    transition:
      border-color 160ms ease,
      box-shadow 160ms ease,
      transform 160ms ease;
  }

  .note-composer:focus-within {
    border-color: color-mix(in oklch, var(--color-accent), var(--color-border) 40%);
    box-shadow: 0 12px 32px color-mix(in oklch, var(--color-text), transparent 93%);
    transform: translateY(-1px);
  }

  textarea {
    min-height: 112px;
    max-height: 360px;
    padding: 5px 6px 12px;
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    font-size: 15px;
    line-height: 25px;
    resize: none;
  }

  textarea:focus {
    box-shadow: none;
  }

  .composer-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-top: 10px;
    border-top: 1px solid var(--color-border);
  }

  .composer-submit {
    display: flex;
    flex: none;
    gap: 10px;
    align-items: center;
  }

  .composer-submit > span {
    display: flex;
    gap: 2px;
  }

  @media (max-width: 767px) {
    .note-composer {
      padding: 13px 12px 11px;
    }

    textarea {
      min-height: 132px;
    }

    .composer-footer {
      padding-top: 9px;
    }

    .composer-submit {
      justify-content: flex-end;
    }
  }
</style>
