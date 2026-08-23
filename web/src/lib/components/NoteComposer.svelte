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
      error = 'Write something before saving.';
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
      error = errorMessage(cause, 'Unable to save the note.');
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
    textarea.style.height = `${Math.min(Math.max(textarea.scrollHeight, 88), 280)}px`;
  }
</script>

<section class="note-composer" aria-labelledby="composer-title">
  <h2 class="sr-only" id="composer-title">Create a note</h2>
  <label class="sr-only" for="note-composer-input">Note content</label>
  <textarea
    aria-describedby={error ? 'composer-error' : undefined}
    bind:this={textarea}
    bind:value={content}
    disabled={busy}
    id="note-composer-input"
    oninput={resizeTextarea}
    onkeydown={handleKeydown}
    placeholder="Write a quick note…"
    rows="3"></textarea>
  <div class="composer-footer">
    <div class="composer-submit">
      <button
        class="button primary"
        disabled={busy || !content.trim()}
        onclick={() => void submit()}
        type="button"
      >
        {busy ? 'Saving…' : 'Save'}
      </button>
    </div>
  </div>
  {#if error}<p aria-live="assertive" class="form-error" id="composer-error">{error}</p>{/if}
</section>

<style>
  .note-composer {
    position: relative;
    padding: 16px 20px 14px;
    margin-bottom: 10px;
    background: var(--color-surface);
    border-radius: 18px;
    box-shadow:
      0 1px 2px color-mix(in oklch, var(--color-text), transparent 94%),
      0 16px 42px color-mix(in oklch, var(--color-text), transparent 95%);
    transition:
      box-shadow 160ms ease;
  }

  .note-composer:focus-within {
    box-shadow:
      0 0 0 3px color-mix(in oklch, var(--color-accent), transparent 88%),
      0 20px 52px color-mix(in oklch, var(--color-text), transparent 92%);
  }

  textarea {
    min-height: 88px;
    max-height: 280px;
    padding: 5px 2px 9px;
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    font-family: var(--font-ui);
    font-size: 15px;
    line-height: 24px;
    resize: none;
  }

  textarea:focus {
    box-shadow: none;
  }

  .composer-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-top: 0;
  }

  .composer-submit {
    display: flex;
    flex: none;
    align-items: center;
  }

  .composer-submit .button {
    min-height: 38px;
    padding-inline: 18px;
    border-radius: 10px;
  }

  @media (max-width: 767px) {
    .note-composer {
      padding: 14px 15px 12px;
      border-radius: 15px;
    }

    textarea {
      min-height: 96px;
      padding-top: 4px;
      font-size: 15px;
    }

    .composer-footer {
      padding-top: 2px;
    }

    .composer-submit {
      justify-content: flex-end;
    }
  }
</style>
