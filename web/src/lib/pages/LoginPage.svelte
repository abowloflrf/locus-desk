<script lang="ts">
  import { onMount } from 'svelte';

  import { Button } from '$lib/components/ui/button';
  import * as Field from '$lib/components/ui/field';
  import { Input } from '$lib/components/ui/input';
  import { Spinner } from '$lib/components/ui/spinner';

  import { errorMessage } from '../api/client';
  import type { SessionInfo } from '../api/types';

  let {
    onLogin,
  }: {
    onLogin: (username: string, password: string) => Promise<SessionInfo>;
  } = $props();

  let username = $state('');
  let password = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let usernameInput = $state<HTMLInputElement | null>(null);

  onMount(() => {
    if (window.matchMedia('(min-width: 768px) and (pointer: fine)').matches) usernameInput?.focus();
  });

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!username.trim() || !password) {
      error = 'Enter your username and password.';
      return;
    }

    busy = true;
    error = null;
    try {
      await onLogin(username.trim(), password);
    } catch (cause) {
      error = errorMessage(cause, 'Unable to sign in.');
    } finally {
      busy = false;
    }
  }
</script>

<main class="login-page h-full overflow-y-auto bg-background px-6 py-12">
  <div class="mx-auto flex min-h-full w-full max-w-[320px] flex-col justify-center gap-8">
    <a class="flex items-center gap-2 text-sm font-semibold" href="/" aria-label="Locus Desk home">
      <span
        aria-hidden="true"
        class="grid size-7 place-items-center rounded-md bg-primary text-primary-foreground">L</span
      >
      <span>Locus Desk</span>
    </a>
    <section aria-labelledby="login-title">
      <form class="login-form flex w-full flex-col gap-4" onsubmit={submit}>
        <div>
          <h1 class="mb-2 text-xl font-semibold tracking-tight" id="login-title">Sign in</h1>
          <p class="mb-0 text-sm text-muted-foreground">Access your personal workspace.</p>
        </div>

        <Field.Group>
          <Field.Field data-invalid={Boolean(error && !username.trim())}>
            <Field.Label for="login-username">Username</Field.Label>
            <Input
              aria-invalid={error && !username.trim() ? 'true' : undefined}
              autocomplete="username"
              bind:ref={usernameInput}
              disabled={busy}
              id="login-username"
              name="username"
              spellcheck={false}
              autocapitalize="none"
              bind:value={username}
            />
          </Field.Field>
          <Field.Field data-invalid={Boolean(error && !password)}>
            <Field.Label for="login-password">Password</Field.Label>
            <Input
              aria-invalid={error && !password ? 'true' : undefined}
              autocomplete="current-password"
              disabled={busy}
              id="login-password"
              name="password"
              type="password"
              bind:value={password}
            />
          </Field.Field>
        </Field.Group>

        {#if error}<Field.Error class="login-error">{error}</Field.Error>{/if}
        <Button class="login-button h-11 w-full" disabled={busy} type="submit">
          {#if busy}<Spinner data-icon="inline-start" />{/if}
          {busy ? 'Signing in…' : 'Sign in'}
        </Button>
      </form>
    </section>
  </div>
</main>
