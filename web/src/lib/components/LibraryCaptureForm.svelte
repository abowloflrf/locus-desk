<script lang="ts">
  import { tick } from 'svelte';

  import type { CreateLibraryItemRequest, LibraryItem } from '../api/types';
  import Icon from './Icon.svelte';
  import StatusMessage from './StatusMessage.svelte';

  let {
    onCreate,
  }: {
    onCreate: (payload: CreateLibraryItemRequest) => Promise<LibraryItem>;
  } = $props();

  let url = $state('');
  let title = $state('');
  let selection = $state('');
  let note = $state('');
  let tags = $state('');
  let expanded = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);
  let urlInput = $state<HTMLInputElement>();
  let contextButton = $state<HTMLButtonElement>();
  let form = $state<HTMLFormElement>();

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (submitting) return;

    const submittedUrl = url.trim();
    if (!isHttpUrl(submittedUrl)) {
      error = 'Enter a complete http or https URL.';
      success = null;
      await tick();
      urlInput?.focus();
      return;
    }

    error = null;
    success = null;
    submitting = true;
    const payload: CreateLibraryItemRequest = { url: submittedUrl };
    if (title.trim()) payload.title = title;
    if (selection.trim()) payload.selection = selection;
    if (note.trim()) payload.note = note;
    const parsedTags = parseTags(tags);
    if (parsedTags.length > 0) payload.tags = parsedTags;

    let saved = false;
    try {
      const item = await onCreate(payload);
      url = '';
      title = '';
      selection = '';
      note = '';
      tags = '';
      expanded = false;
      success = `Saved ${item.title ? `“${item.title}”` : 'link'} to Library.`;
      saved = true;
    } catch (cause) {
      error = cause instanceof Error && cause.message ? cause.message : 'Unable to save this link.';
    } finally {
      submitting = false;
    }
    if (saved) {
      await tick();
      urlInput?.focus();
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!(event.target instanceof Node) || !form?.contains(event.target)) return;
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      form?.requestSubmit();
      return;
    }
    if (event.key === 'Escape' && expanded) {
      event.preventDefault();
      expanded = false;
      requestAnimationFrame(() => contextButton?.focus());
    }
  }

  function isHttpUrl(value: string): boolean {
    try {
      const parsed = new URL(value);
      return parsed.protocol === 'http:' || parsed.protocol === 'https:';
    } catch {
      return false;
    }
  }

  function parseTags(value: string): string[] {
    const seen = new Set<string>();
    return value
      .split(/[\s,#]+/u)
      .map((tag) => tag.trim())
      .filter((tag) => {
        const normalized = tag.toLocaleLowerCase();
        if (!normalized || seen.has(normalized)) return false;
        seen.add(normalized);
        return true;
      });
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<section aria-labelledby="library-title" class="library-capture">
  <header class="capture-heading">
    <div>
      <p class="eyebrow">Knowledge library</p>
      <h1 id="library-title">Save a link</h1>
      <p>Keep the source and the context that made it useful.</p>
    </div>
    <button
      aria-controls="library-capture-context"
      aria-expanded={expanded}
      bind:this={contextButton}
      class="button secondary context-toggle"
      onclick={() => (expanded = !expanded)}
      type="button"
    >
      <Icon name={expanded ? 'close' : 'plus'} size={16} />
      {expanded ? 'Hide context' : 'Add context'}
    </button>
  </header>

  <form aria-busy={submitting} bind:this={form} onsubmit={(event) => void submit(event)}>
    <div class="capture-primary">
      <label class="field url-field" for="library-url">
        <span>URL</span>
        <input
          aria-describedby="library-url-hint"
          autocomplete="url"
          bind:this={urlInput}
          bind:value={url}
          disabled={submitting}
          id="library-url"
          inputmode="url"
          placeholder="https://example.com/article"
          required
          type="url"
        />
      </label>
      <button class="button primary save-link" disabled={submitting || !url.trim()} type="submit">
        {submitting ? 'Saving…' : 'Save link'}
      </button>
    </div>
    <p class="field-hint" id="library-url-hint">Press Ctrl or Command + Enter to save.</p>

    {#if expanded}
      <div class="capture-context" id="library-capture-context">
        <label class="field title-field" for="library-title-input">
          <span>Title <small>Optional</small></span>
          <input
            bind:value={title}
            disabled={submitting}
            id="library-title-input"
            placeholder="Leave blank to show the site name"
          />
        </label>
        <label class="field tags-field" for="library-tags">
          <span>Tags <small>Optional</small></span>
          <input
            autocomplete="off"
            bind:value={tags}
            disabled={submitting}
            id="library-tags"
            placeholder="research, rust"
          />
        </label>
        <label class="field selection-field" for="library-selection">
          <span>Selection <small>Optional</small></span>
          <textarea
            bind:value={selection}
            disabled={submitting}
            id="library-selection"
            placeholder="Paste the passage you want to remember"
            rows="3"></textarea>
        </label>
        <label class="field note-field" for="library-note">
          <span>Note <small>Optional</small></span>
          <textarea
            bind:value={note}
            disabled={submitting}
            id="library-note"
            placeholder="Why are you keeping this?"
            rows="3"></textarea>
        </label>
      </div>
    {/if}
  </form>

  {#if error}<StatusMessage tone="error">{error}</StatusMessage>{/if}
  {#if success}<StatusMessage tone="success">{success}</StatusMessage>{/if}
</section>

<style>
  .library-capture {
    padding-bottom: 28px;
    border-bottom: 1px solid var(--color-border);
  }

  .capture-heading {
    display: flex;
    gap: 24px;
    align-items: flex-end;
    justify-content: space-between;
    margin-bottom: 22px;
  }

  .capture-heading h1 {
    margin-bottom: 2px;
    font-size: 25px;
    font-weight: 680;
    letter-spacing: -0.03em;
    line-height: 1.25;
  }

  .capture-heading p:last-child {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .context-toggle {
    flex: none;
    gap: 7px;
  }

  .capture-primary {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    align-items: end;
  }

  .url-field input {
    min-height: 44px;
    font-family: var(--font-mono);
    font-size: 13px;
  }

  .save-link {
    min-height: 44px;
    padding-inline: 18px;
  }

  .field-hint {
    margin: 6px 0 0;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .capture-context {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
    padding-top: 20px;
    animation: reveal-context 160ms ease-out;
  }

  .field small {
    margin-left: 4px;
    color: var(--color-text-muted);
    font-size: 10px;
    font-weight: 450;
  }

  .selection-field,
  .note-field {
    grid-column: span 1;
  }

  @keyframes reveal-context {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  @media (max-width: 767px) {
    .library-capture {
      padding-bottom: 24px;
    }

    .capture-heading {
      display: grid;
      gap: 16px;
      align-items: start;
      margin-bottom: 18px;
    }

    .capture-heading h1 {
      font-size: 23px;
    }

    .context-toggle {
      width: 100%;
    }

    .capture-primary,
    .capture-context {
      grid-template-columns: minmax(0, 1fr);
    }

    .save-link {
      width: 100%;
    }
  }
</style>
