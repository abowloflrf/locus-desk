<script lang="ts">
  import { onMount } from 'svelte';

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
  let usernameInput = $state<HTMLInputElement>();

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

<main class="login-page">
  <section class="login-intro" aria-labelledby="login-brand">
    <div class="login-brand">
      <span aria-hidden="true">L</span>
      <strong id="login-brand">Locus Desk</strong>
    </div>
    <div>
      <p class="eyebrow">Personal workspace</p>
      <h1>Keep thoughts close.<br />Keep today clear.</h1>
      <p>Notes and next actions, in one quiet place.</p>
    </div>
  </section>

  <section class="login-form-region" aria-labelledby="login-title">
    <form class="login-form" onsubmit={submit}>
      <div>
        <p class="eyebrow">Welcome back</p>
        <h2 id="login-title">Sign in</h2>
      </div>
      <label class="field">
        <span>Username</span>
        <input
          autocomplete="username"
          bind:this={usernameInput}
          disabled={busy}
          bind:value={username}
        />
      </label>
      <label class="field">
        <span>Password</span>
        <input
          autocomplete="current-password"
          disabled={busy}
          type="password"
          bind:value={password}
        />
      </label>
      {#if error}<p aria-live="assertive" class="form-error login-error">{error}</p>{/if}
      <button class="button primary login-button" disabled={busy} type="submit">
        {busy ? 'Signing in…' : 'Sign in'}
      </button>
    </form>
  </section>
</main>

<style>
  .login-page {
    display: grid;
    height: 100%;
    min-height: 100vh;
    grid-template-columns: minmax(360px, 0.92fr) minmax(430px, 1.08fr);
    overflow-y: auto;
    scrollbar-width: none;
  }

  .login-page::-webkit-scrollbar {
    display: none;
  }

  .login-intro {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 42px clamp(36px, 6vw, 88px) 68px;
    background: var(--color-surface-muted);
    border-right: 1px solid var(--color-border);
  }

  .login-brand {
    display: flex;
    gap: 11px;
    align-items: center;
    font-size: 16px;
  }

  .login-brand > span {
    display: grid;
    width: 32px;
    height: 32px;
    color: white;
    background: var(--color-accent);
    border-radius: 8px;
    font-weight: 700;
    place-items: center;
  }

  .login-intro h1 {
    max-width: 540px;
    margin-bottom: 18px;
    font-family: var(--font-reading);
    font-size: clamp(38px, 5vw, 68px);
    font-weight: 520;
    line-height: 1.08;
    letter-spacing: -0.045em;
  }

  .login-intro > div:last-child > p:last-child {
    margin-bottom: 0;
    color: var(--color-text-muted);
    font-size: 15px;
  }

  .login-form-region {
    display: grid;
    padding: 40px;
    background: var(--color-canvas);
    place-items: center;
  }

  .login-form {
    display: grid;
    width: min(100%, 360px);
    gap: 18px;
  }

  .login-form h2 {
    margin-bottom: 0;
    font-size: 26px;
  }

  .login-form input {
    min-height: 44px;
  }

  .login-error {
    margin: -5px 0 0;
  }

  .login-button {
    min-height: 44px;
    margin-top: 3px;
  }

  @media (max-width: 767px) {
    .login-page {
      display: block;
    }

    .login-intro {
      min-height: 250px;
      padding: 26px 24px 34px;
      border-right: 0;
      border-bottom: 1px solid var(--color-border);
    }

    .login-intro h1 {
      margin: 36px 0 10px;
      font-size: 38px;
    }

    .login-form-region {
      padding: 36px 24px 48px;
    }
  }
</style>
