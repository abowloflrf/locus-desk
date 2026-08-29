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

  onMount(() => usernameInput?.focus());

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

<main
  class="login-page grid min-h-dvh overflow-y-auto md:grid-cols-[minmax(360px,0.92fr)_minmax(430px,1.08fr)]"
>
  <section
    class="flex min-h-[250px] flex-col justify-between border-b bg-muted px-6 pb-8 pt-[calc(1.5rem+env(safe-area-inset-top))] md:min-h-full md:border-r md:border-b-0 md:px-[clamp(2.25rem,6vw,5.5rem)] md:pt-10 md:pb-16"
    aria-labelledby="login-brand"
  >
    <div class="flex items-center gap-3 text-base">
      <span
        aria-hidden="true"
        class="grid size-8 place-items-center rounded-md bg-primary font-bold text-primary-foreground"
        >L</span
      >
      <strong id="login-brand">Locus Desk</strong>
    </div>
    <div>
      <p class="text-xs font-semibold tracking-[0.12em] text-muted-foreground uppercase">
        Personal workspace
      </p>
      <h1
        class="mt-9 mb-3 max-w-xl text-4xl leading-[1.08] font-semibold tracking-[-0.04em] md:mt-0 md:mb-4 md:text-[clamp(2.5rem,5vw,4.25rem)]"
      >
        Keep thoughts close.<br />Keep today clear.
      </h1>
      <p class="text-sm text-muted-foreground md:text-base">
        Memos and next actions, in one quiet place.
      </p>
    </div>
  </section>

  <section
    class="grid place-items-center bg-background px-6 py-9 pb-[calc(3rem+env(safe-area-inset-bottom))] md:p-10"
    aria-labelledby="login-title"
  >
    <form class="login-form flex w-full max-w-[360px] flex-col gap-5" onsubmit={submit}>
      <div>
        <p class="text-xs font-semibold tracking-[0.12em] text-muted-foreground uppercase">
          Welcome back
        </p>
        <h2 class="mt-1 text-2xl font-semibold tracking-tight" id="login-title">Sign in</h2>
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
</main>
