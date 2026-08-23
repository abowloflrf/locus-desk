<script lang="ts">
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

  let dialog = $state<HTMLDialogElement>();
  let returnFocus: HTMLElement | null = null;
  const titleId = `confirm-title-${Math.random().toString(36).slice(2)}`;
  const messageId = `confirm-message-${Math.random().toString(36).slice(2)}`;
  const errorId = `confirm-error-${Math.random().toString(36).slice(2)}`;

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
    }
    if (!open && dialog.open) {
      dialog.close();
      requestAnimationFrame(() => {
        if (returnFocus?.isConnected) returnFocus.focus();
        else if (onFallbackFocus) void onFallbackFocus();
        else document.getElementById('main-content')?.focus();
        returnFocus = null;
      });
    }
  });

  function handleCancel(event: Event): void {
    event.preventDefault();
    if (!busy) onCancel();
  }

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === dialog && !busy) onCancel();
  }
</script>

<dialog
  aria-describedby={error ? `${messageId} ${errorId}` : messageId}
  aria-labelledby={titleId}
  bind:this={dialog}
  class="confirm-dialog"
  oncancel={handleCancel}
  onclick={handleBackdrop}
>
  <div class="dialog-content">
    <h2 id={titleId}>{title}</h2>
    <p id={messageId}>{message}</p>
    {#if error}
      <p aria-live="assertive" class="dialog-error" id={errorId} role="alert">{error}</p>
    {/if}
    <div class="dialog-actions">
      <button class="button secondary" disabled={busy} onclick={onCancel} type="button"
        >Cancel</button
      >
      <button class="button danger" disabled={busy} onclick={onConfirm} type="button">
        {busy ? 'Working…' : confirmLabel}
      </button>
    </div>
  </div>
</dialog>

<style>
  .confirm-dialog {
    width: min(420px, calc(100vw - 32px));
    padding: 0;
    color: var(--color-text);
    background: transparent;
    border: 0;
    border-radius: var(--radius-surface);
  }

  .confirm-dialog::backdrop {
    background: color-mix(in oklch, var(--color-text), transparent 72%);
    backdrop-filter: blur(2px);
  }

  .dialog-content {
    padding: 24px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-surface);
    box-shadow: 0 20px 70px color-mix(in oklch, var(--color-text), transparent 78%);
  }

  .dialog-content h2 {
    margin-bottom: 8px;
    font-size: 17px;
  }

  .dialog-content p {
    margin-bottom: 22px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .dialog-content .dialog-error {
    margin: -10px 0 18px;
    color: var(--color-danger);
  }

  .dialog-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
</style>
