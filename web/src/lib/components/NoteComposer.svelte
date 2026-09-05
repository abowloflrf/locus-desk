<script lang="ts">
  import { tick } from 'svelte';
  import * as Field from '$lib/components/ui/field';
  import * as InputGroup from '$lib/components/ui/input-group';
  import { Spinner } from '$lib/components/ui/spinner';
  import { cn } from '$lib/utils';

  import { errorMessage } from '../api/client';
  import type { Note } from '../api/types';

  let { onCreate }: { onCreate: (content: string) => Promise<Note> } = $props();

  let content = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let showActions = $derived(Boolean(content) || busy || Boolean(error));
  let textarea = $state<HTMLTextAreaElement | null>(null);

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
    } catch (cause) {
      error = errorMessage(cause, 'Unable to save the memo.');
    } finally {
      busy = false;
      await tick();
      textarea?.focus();
      resizeTextarea();
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
    textarea.style.height = `${Math.min(Math.max(textarea.scrollHeight, 44), 280)}px`;
  }
</script>

<section class="note-composer mb-3" aria-labelledby="composer-title">
  <h2 class="sr-only" id="composer-title">Create a memo</h2>
  <Field.Field data-invalid={Boolean(error)}>
    <Field.Label class="sr-only" for="note-composer-input">Memo content</Field.Label>
    <InputGroup.Root variant="quiet">
      <InputGroup.Textarea
        aria-describedby={error ? 'composer-error' : undefined}
        aria-invalid={error ? 'true' : undefined}
        bind:ref={textarea}
        class="min-h-11 max-h-[280px] transition-[height] duration-150"
        bind:value={content}
        disabled={busy}
        id="note-composer-input"
        oninput={resizeTextarea}
        onkeydown={handleKeydown}
        placeholder="Write a quick memo…"
        rows={1}
      />
      <InputGroup.Addon align="block-end" class={cn(!showActions && 'hidden')}>
        <InputGroup.Button
          class="composer-submit ml-auto"
          disabled={busy || !content.trim()}
          onclick={() => void submit()}
          size="sm"
          variant="default"
        >
          {#if busy}<Spinner data-icon="inline-start" />{/if}
          {busy ? 'Saving…' : 'Save'}
        </InputGroup.Button>
      </InputGroup.Addon>
    </InputGroup.Root>
    {#if error}<Field.Error id="composer-error">{error}</Field.Error>{/if}
  </Field.Field>
</section>
