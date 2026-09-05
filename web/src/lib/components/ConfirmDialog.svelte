<script lang="ts">
  import * as AlertDialog from '$lib/components/ui/alert-dialog';
  import { Spinner } from '$lib/components/ui/spinner';
  import * as Field from '$lib/components/ui/field';

  let {
    open,
    title,
    message,
    confirmLabel = 'Confirm',
    busy = false,
    error = null,
    onConfirm,
    onCancel,
    onFallbackFocus,
  }: {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    busy?: boolean;
    error?: string | null;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
    onFallbackFocus?: () => void | Promise<void>;
  } = $props();

  let wasOpen = false;
  let cancelled = false;

  $effect(() => {
    if (open) cancelled = false;
    if (wasOpen && !open && !cancelled && onFallbackFocus) {
      requestAnimationFrame(() => void onFallbackFocus());
    }
    wasOpen = open;
  });

  function handleOpenChange(nextOpen: boolean): void {
    if (nextOpen || !open || busy) return;
    cancelled = true;
    onCancel();
  }

  async function handleConfirm(event: MouseEvent): Promise<void> {
    event.preventDefault();
    await onConfirm();
  }
</script>

<AlertDialog.Root {open} onOpenChange={handleOpenChange}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>{title}</AlertDialog.Title>
      <AlertDialog.Description>{message}</AlertDialog.Description>
      {#if error}
        <Field.Error aria-live="assertive">{error}</Field.Error>
      {/if}
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={busy}>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action disabled={busy} onclick={handleConfirm} variant="destructive">
        {#if busy}<Spinner data-icon="inline-start" />{/if}
        {busy ? 'Working…' : confirmLabel}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
