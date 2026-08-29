<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';
  import { tick } from 'svelte';

  import type { CreateLibraryItemRequest, LibraryItem } from '../api/types';
  import StatusMessage from './StatusMessage.svelte';
  import { Button, buttonVariants } from './ui/button';
  import * as Field from './ui/field';
  import { Input } from './ui/input';
  import * as Popover from './ui/popover';
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
      await tick();
      triggerButton?.focus();
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
      void tick().then(() => urlInput?.focus());
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
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

<Popover.Root bind:open onOpenChange={handleOpenChange}>
  <Popover.Trigger
    bind:ref={triggerButton}
    aria-label="Add a link"
    class={buttonVariants({ size: 'icon', variant: 'default' })}
    title="Add link"
  >
    <Plus />
  </Popover.Trigger>
  <Popover.Content align="end" class="w-[min(24rem,calc(100vw-2rem))] p-4" sideOffset={8}>
    <Popover.Header>
      <Popover.Title>Save a link</Popover.Title>
      <Popover.Description>Paste the article URL you want to keep.</Popover.Description>
    </Popover.Header>
    <form aria-busy={submitting} bind:this={form} class="mt-4" onsubmit={submit}>
      <Field.Group>
        <Field.Field data-invalid={Boolean(error)}>
          <Field.Label for="library-url">URL</Field.Label>
          <Input
            aria-invalid={Boolean(error)}
            autocomplete="url"
            bind:ref={urlInput}
            bind:value={url}
            disabled={submitting}
            id="library-url"
            inputmode="url"
            onkeydown={handleKeydown}
            placeholder="https://example.com/article"
            required
            type="url"
          />
          <Field.Description>Press Ctrl or Command + Enter to save.</Field.Description>
        </Field.Field>
        {#if error}<StatusMessage tone="error">{error}</StatusMessage>{/if}
        <Field.Field class="justify-end" orientation="horizontal">
          <Button disabled={submitting || !url.trim()} type="submit">
            {#if submitting}<Spinner data-icon="inline-start" />{/if}
            {submitting ? 'Saving…' : 'Save'}
          </Button>
        </Field.Field>
      </Field.Group>
    </form>
  </Popover.Content>
</Popover.Root>
