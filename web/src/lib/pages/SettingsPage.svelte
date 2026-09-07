<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Field from '$lib/components/ui/field';
  import { Input } from '$lib/components/ui/input';
  import { Spinner } from '$lib/components/ui/spinner';
  import StatusMessage from '../components/StatusMessage.svelte';
  import { errorMessage } from '../api/client';
  import { getAiSettings, saveAiSettings, testAiSettings, type AiSettings } from '../api/settings';

  let saved = $state<AiSettings | null>(null);
  let baseUrl = $state('');
  let model = $state('');
  let apiKey = $state('');
  let loading = $state(true);
  let busy = $state(false);
  let testing = $state(false);
  let testResult = $state('');
  let error = $state('');
  let success = $state('');
  let errors = $state<Record<string, string>>({});
  let form = $state<HTMLFormElement>();
  const controller = new AbortController();
  let dirty = $derived(
    saved !== null && (baseUrl !== saved.baseUrl || model !== saved.model || apiKey !== ''),
  );

  onMount(() => {
    void load();
    return () => controller.abort();
  });

  async function load() {
    loading = true;
    error = '';
    try {
      saved = await getAiSettings(controller.signal);
      baseUrl = saved.baseUrl;
      model = saved.model;
    } catch (cause) {
      if (!controller.signal.aborted) error = errorMessage(cause, 'Unable to load settings.');
    } finally {
      loading = false;
    }
  }

  async function validate(): Promise<boolean> {
    errors = {};
    error = '';
    success = '';
    try {
      const url = new URL(baseUrl.trim());
      if (
        !['http:', 'https:'].includes(url.protocol) ||
        !url.hostname ||
        url.username ||
        url.password ||
        url.search ||
        url.hash ||
        baseUrl.trim().length > 2048
      )
        throw new Error();
    } catch {
      errors.baseUrl =
        'Enter an HTTP or HTTPS URL without credentials, query parameters, or fragments.';
    }
    if (
      !model.trim() ||
      new TextEncoder().encode(model.trim()).length > 200 ||
      /[\x00-\x1f\x7f-\x9f]/.test(model)
    )
      errors.model = 'Enter a model name of up to 200 bytes without control characters.';
    if (
      new TextEncoder().encode(apiKey.trim()).length > 8192 ||
      /[\x00-\x1f\x7f-\x9f]/.test(apiKey.trim())
    )
      errors.apiKey = 'Enter a valid API key without control characters.';
    if (Object.keys(errors).length) {
      await tick();
      form?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
      return false;
    }
    return true;
  }

  function payload() {
    return {
      baseUrl: baseUrl.trim(),
      model: model.trim(),
      ...(apiKey.trim() ? { apiKey: apiKey.trim() } : {}),
    };
  }

  async function testConnection() {
    if (busy || testing || !saved || !(await validate())) return;
    testResult = '';
    testing = true;
    try {
      const result = await testAiSettings(payload(), controller.signal);
      testResult = `Connection successful. Model responded in ${(result.elapsedMs / 1000).toFixed(2)} s.`;
    } catch (cause) {
      if (!controller.signal.aborted) error = errorMessage(cause, 'Unable to test the model.');
    } finally {
      testing = false;
    }
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (busy || testing || !saved || !(await validate())) return;
    busy = true;
    try {
      saved = await saveAiSettings(payload(), controller.signal);
      baseUrl = saved.baseUrl;
      model = saved.model;
      apiKey = '';
      success = 'Settings saved.';
    } catch (cause) {
      if (!controller.signal.aborted) error = errorMessage(cause, 'Unable to save settings.');
    } finally {
      busy = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter' && dirty && !busy && !testing) {
      event.preventDefault();
      form?.requestSubmit();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="page settings-page">
  <header class="page-header">
    <h1 class="max-md:sr-only">Settings</h1>
  </header>
  <section class="max-w-[560px]" aria-labelledby="ai-settings-title">
    <div class="mb-8">
      <h2 id="ai-settings-title" class="mb-2 text-base font-semibold">AI model</h2>
      <p class="text-sm leading-6 text-muted-foreground">
        Configure an OpenAI-compatible provider for article summaries in Library.
      </p>
      <p class="mt-2 text-xs leading-5 text-muted-foreground">
        Summaries are not available yet. You can save your configuration now.
      </p>
    </div>
    {#if loading}
      <p role="status" class="flex items-center gap-2 text-sm text-muted-foreground">
        <Spinner /> Loading settings…
      </p>
    {:else if saved}
      <form
        bind:this={form}
        onsubmit={save}
        oninput={() => {
          success = '';
          testResult = '';
          error = '';
        }}
        novalidate
        aria-busy={busy || testing}
      >
        <Field.Group>
          <Field.Field data-invalid={Boolean(errors.baseUrl)}>
            <Field.Label for="ai-base-url">Base URL</Field.Label>
            <Input
              class="max-md:min-h-11"
              id="ai-base-url"
              type="url"
              bind:value={baseUrl}
              disabled={busy || testing}
              placeholder="https://api.example.com/v1"
              autocomplete="off"
              spellcheck={false}
              aria-invalid={Boolean(errors.baseUrl)}
              aria-describedby={errors.baseUrl ? 'ai-url-error' : 'ai-url-hint'}
            />
            <Field.Description id="ai-url-hint"
              >Use your provider's API base URL, including its version path, such as /v1.</Field.Description
            >
            {#if errors.baseUrl}<Field.Error id="ai-url-error">{errors.baseUrl}</Field.Error>{/if}
          </Field.Field>
          <Field.Field data-invalid={Boolean(errors.apiKey)}>
            <Field.Label for="ai-api-key">API key</Field.Label>
            <Input
              class="max-md:min-h-11"
              id="ai-api-key"
              type="password"
              bind:value={apiKey}
              disabled={busy || testing}
              placeholder={saved.hasApiKey
                ? 'Key saved · enter a new key to replace it'
                : 'Enter your API key'}
              autocomplete="new-password"
              spellcheck={false}
              aria-invalid={Boolean(errors.apiKey)}
              aria-describedby={errors.apiKey ? 'ai-key-error' : 'ai-key-hint'}
            />
            <Field.Description id="ai-key-hint"
              >{saved.hasApiKey
                ? 'Leave blank to keep your saved key.'
                : 'Optional for providers that do not require authentication.'} Stored on the server for
              your account in this workspace.</Field.Description
            >
            {#if errors.apiKey}<Field.Error id="ai-key-error">{errors.apiKey}</Field.Error>{/if}
          </Field.Field>
          <Field.Field data-invalid={Boolean(errors.model)}>
            <Field.Label for="ai-model">Model</Field.Label>
            <Input
              class="max-md:min-h-11"
              id="ai-model"
              bind:value={model}
              disabled={busy || testing}
              placeholder="Enter a model ID"
              autocomplete="off"
              spellcheck={false}
              aria-invalid={Boolean(errors.model)}
              aria-describedby={errors.model ? 'ai-model-error' : 'ai-model-hint'}
            />
            <Field.Description id="ai-model-hint"
              >Enter the exact model ID provided by your service.</Field.Description
            >
            {#if errors.model}<Field.Error id="ai-model-error">{errors.model}</Field.Error>{/if}
          </Field.Field>
        </Field.Group>
        <div class="mt-8 flex flex-wrap items-center gap-3">
          <Button class="max-md:min-h-11" type="submit" disabled={busy || testing || !dirty}
            >{#if busy}<Spinner data-icon="inline-start" />{/if}{busy
              ? 'Saving…'
              : 'Save settings'}</Button
          >
          <Button
            class="max-md:min-h-11"
            type="button"
            variant="outline"
            disabled={busy || testing}
            onclick={() => void testConnection()}
          >
            {#if testing}<Spinner data-icon="inline-start" />{/if}{testing
              ? 'Testing…'
              : 'Test connection'}
          </Button>
          <span class="text-xs text-muted-foreground" aria-live="polite"
            >{dirty ? 'Unsaved changes' : ''}</span
          >
        </div>
        <p class="mt-3 text-xs leading-5 text-muted-foreground">
          Testing sends a short request to your provider and may use a small amount of API credit.
          It does not save changes.
        </p>
      </form>
    {/if}
    {#if error}<StatusMessage tone="error">{error}</StatusMessage>{/if}
    {#if !loading && !saved}<Button variant="outline" onclick={() => void load()}>Try again</Button
      >{/if}
    {#if testing}<p class="mt-3 text-sm text-muted-foreground" role="status">
        Waiting for the model…
      </p>{/if}
    {#if testResult}<StatusMessage tone="success">{testResult}</StatusMessage>{/if}
    {#if success}<StatusMessage tone="success">{success}</StatusMessage>{/if}
  </section>
</div>
