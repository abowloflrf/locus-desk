<script lang="ts">
  type Health = {
    status: 'ok';
    service: string;
    version: string;
    schemaVersion: number;
  };

  let health = $state<Health | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const controller = new AbortController();

    fetch('/api/v1/health', { signal: controller.signal })
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Health check failed with status ${response.status}`);
        }
        return response.json() as Promise<Health>;
      })
      .then((payload) => {
        health = payload;
      })
      .catch((cause: unknown) => {
        if (cause instanceof DOMException && cause.name === 'AbortError') {
          return;
        }
        error = cause instanceof Error ? cause.message : 'Unable to reach the API';
      });

    return () => controller.abort();
  });
</script>

<svelte:head>
  <title>Locus Desk</title>
</svelte:head>

<div class="app-shell">
  <aside class="navigation" aria-label="Main navigation">
    <a class="brand" href="/" aria-label="Locus Desk home">
      <span class="brand-mark" aria-hidden="true">L</span>
      <span>Locus Desk</span>
    </a>

    <nav>
      <a class="nav-item active" href="/" aria-current="page">Notes</a>
      <span class="nav-item disabled">Tasks</span>
      <span class="nav-item disabled">Library</span>
      <span class="nav-item disabled">Reader</span>
      <span class="nav-item disabled">Chat</span>
    </nav>
  </aside>

  <main>
    <div class="eyebrow">Phase 0A · A0</div>
    <h1>Project foundation is ready</h1>
    <p class="summary">
      The Rust, Axum, Svelte 5, and Vite development and production build pipelines are connected.
      The first vertical slice will start with the database, migrations, and owner bootstrap.
    </p>

    <section class="status-panel" aria-labelledby="api-status-title" aria-live="polite">
      <div>
        <h2 id="api-status-title">API status</h2>
        <p>GET /api/v1/health</p>
      </div>

      {#if health}
        <span class="status success">Online · v{health.version}</span>
      {:else if error}
        <span class="status error">Disconnected</span>
      {:else}
        <span class="status pending">Checking</span>
      {/if}
    </section>

    {#if error}
      <p class="inline-error">{error}. Confirm that the Axum server is running on port 7310.</p>
    {/if}
  </main>

  <aside class="context-rail" aria-label="Initialization scope">
    <h2>Initialization scope</h2>
    <ul>
      <li>Rust 2024 project</li>
      <li>Svelte 5 SPA</li>
      <li>Vite API proxy</li>
      <li>Embedded static assets</li>
      <li>Unified quality commands</li>
    </ul>
  </aside>
</div>
