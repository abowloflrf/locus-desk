<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import { tick } from 'svelte';

  import type { CreateLibraryItemRequest, LibraryItem } from '../api/types';
  import { Button, buttonVariants } from './ui/button';
  import * as Field from './ui/field';
  import { Input } from './ui/input';
  import * as Dialog from './ui/dialog';
  import { Spinner } from './ui/spinner';

  let {
    onCreate,
  }: {
    onCreate: (payload: CreateLibraryItemRequest) => Promise<LibraryItem>;
  } = $props();

  let open = $state(false);
  let url = $state('');
  let submitting = $state(false);
  let error = $state<string | null>(null);
  let form = $state<HTMLFormElement | null>(null);
  let urlInput = $state<HTMLInputElement | null>(null);
  let triggerButton = $state<HTMLButtonElement | null>(null);

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (submitting) return;

    const submittedUrl = url.trim();
    if (!isHttpUrl(submittedUrl)) {
      error = 'Enter a complete http or https URL.';
      await tick();
      urlInput?.focus();
      return;
    }

    error = null;
    submitting = true;
    try {
      await onCreate({ url: submittedUrl });
      url = '';
      open = false;
    } catch (cause) {
      error = cause instanceof Error && cause.message ? cause.message : 'Unable to save this link.';
    } finally {
      submitting = false;
    }
  }

  function handleOpenChange(nextOpen: boolean): void {
    if (submitting) return;
    open = nextOpen;
    if (open) {
      error = null;
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!event.isComposing && (event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      form?.requestSubmit();
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
</script>

<Dialog.Root {open} onOpenChange={handleOpenChange}>
  <Dialog.Trigger
    bind:ref={triggerButton}
    aria-label="Add a link"
    class={buttonVariants({ size: 'icon', variant: 'default' })}
    title="Add link"
  >
    <Plus />
  </Dialog.Trigger>
  <Dialog.Content
    class="max-h-[calc(100dvh-2rem)] gap-4 overflow-y-auto overscroll-contain sm:max-w-xl"
    aria-describedby={undefined}
    showCloseButton={!submitting}
    onOpenAutoFocus={(event) => {
      event.preventDefault();
      urlInput?.focus();
    }}
    onCloseAutoFocus={(event) => {
      event.preventDefault();
      triggerButton?.focus();
    }}
  >
    <Dialog.Header class="pr-8">
      <Dialog.Title>Save a link</Dialog.Title>
    </Dialog.Header>
    <form aria-busy={submitting} bind:this={form} onsubmit={submit}>
      <Field.Group class="gap-4">
        <Field.Field data-invalid={Boolean(error)}>
          <Field.Label class="sr-only" for="library-url">URL</Field.Label>
          <Input
            aria-invalid={Boolean(error)}
            aria-describedby={error ? 'library-url-error' : undefined}
            autocomplete="url"
            bind:ref={urlInput}
            bind:value={url}
            disabled={submitting}
            id="library-url"
            inputmode="url"
            onkeydown={handleKeydown}
            placeholder="https://example.com"
            required
            type="url"
          />
          {#if error}<Field.Error id="library-url-error">{error}</Field.Error>{/if}
        </Field.Field>
        <Field.Field class="justify-end" orientation="horizontal">
          <Button disabled={submitting || !url.trim()} type="submit">
            {#if submitting}<Spinner data-icon="inline-start" />{/if}
            {submitting ? 'Saving…' : 'Save'}
          </Button>
        </Field.Field>
      </Field.Group>
    </form>
  </Dialog.Content>
</Dialog.Root>
