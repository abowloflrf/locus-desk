<script lang="ts">
  import { errorMessage } from '../api/client';
  import type { Note } from '../api/types';

  let { onCreate }: { onCreate: (content: string) => Promise<Note> } = $props();

  let content = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let focused = $state(false);
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
    const minimum = focused || content ? 88 : 60;
    textarea.style.height = 'auto';
    textarea.style.height = `${Math.min(Math.max(textarea.scrollHeight, minimum), 280)}px`;
  }

  function handleFocus(): void {
    focused = true;
    resizeTextarea();
  }

  function handleBlur(): void {
    focused = false;
    if (!content) resizeTextarea();
  }
</script>

<section class="note-composer" aria-labelledby="composer-title">
  <h2 class="sr-only" id="composer-title">Create a note</h2>
  <label class="sr-only" for="note-composer-input">Note content</label>
  <textarea
    aria-describedby={error ? 'composer-hint composer-error' : 'composer-hint'}
    bind:this={textarea}
    bind:value={content}
    disabled={busy}
    id="note-composer-input"
    onblur={handleBlur}
    onfocus={handleFocus}
    oninput={resizeTextarea}
    onkeydown={handleKeydown}
    placeholder="Write a quick note…"
    rows="3"></textarea>
  <div class="composer-footer">
    <span class="composer-hint" id="composer-hint">Markdown <kbd>⌘↵</kbd></span>
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
    padding: 14px 16px 12px;
    margin-bottom: 10px;
    background: var(--color-surface);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-feature);
    box-shadow: var(--shadow-soft);
    transition:
      border-color 160ms ease,
      box-shadow 160ms ease;
  }

  .note-composer:focus-within {
    border-color: color-mix(in oklch, var(--color-accent), var(--color-border) 68%);
    box-shadow:
      0 0 0 3px color-mix(in oklch, var(--color-accent), transparent 88%),
      var(--shadow-soft);
  }

  textarea {
    min-height: 60px;
    max-height: 280px;
    padding: 4px 2px 8px;
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    font-family: var(--font-ui);
    font-size: 15px;
    line-height: 24px;
    resize: none;
    transition: height 160ms ease;
  }

  textarea:focus {
    box-shadow: none;
  }

  .composer-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding-top: 0;
  }

  .composer-hint {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .composer-hint kbd {
    background: transparent;
    border-color: var(--color-border-soft);
  }

  .composer-submit {
    display: flex;
    flex: none;
    align-items: center;
  }

  .composer-submit .button {
    min-height: 38px;
    padding-inline: 18px;
    border-radius: var(--radius-control);
  }

  @media (max-width: 767px) {
    .note-composer {
      padding: 12px 14px 11px;
    }

    textarea {
      min-height: 60px;
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
