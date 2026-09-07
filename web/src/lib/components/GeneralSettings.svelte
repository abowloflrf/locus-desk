<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { getGeneralSettings, saveGeneralSettings, type Language } from '../api/settings';
  import { errorMessage } from '../api/client';
  import * as Field from './ui/field';
  import * as ToggleGroup from './ui/toggle-group';
  import { Button } from './ui/button';
  import { Spinner } from './ui/spinner';
  import StatusMessage from './StatusMessage.svelte';

  let language = $state<Language>('en');
  let selectedLanguage = $state('en');
  let loading = $state(true);
  let loaded = $state(false);
  let saving = $state(false);
  let error = $state('');
  let success = $state('');
  const controller = new AbortController();

  onMount(() => {
    void load();
    return () => controller.abort();
  });

  async function load() {
    loading = true;
    error = '';
    try {
      language = (await getGeneralSettings(controller.signal)).language;
      selectedLanguage = language;
      loaded = true;
    } catch (cause) {
      if (!controller.signal.aborted)
        error = errorMessage(cause, 'Unable to load language settings.');
    } finally {
      loading = false;
    }
  }

  async function changeLanguage(value: string) {
    if (saving || value === language || (value !== 'en' && value !== 'zh-CN')) {
      await tick();
      selectedLanguage = language;
      return;
    }
    saving = true;
    error = '';
    success = '';
    try {
      language = (await saveGeneralSettings({ language: value }, controller.signal)).language;
      success = 'Language saved.';
    } catch (cause) {
      if (!controller.signal.aborted) error = errorMessage(cause, 'Unable to save language.');
    } finally {
      selectedLanguage = language;
      saving = false;
    }
  }
</script>

<section class="mb-12 max-w-[560px]" aria-labelledby="general-settings-title">
  <h2 id="general-settings-title" class="mb-6 text-base font-semibold">General</h2>
  {#if loading}
    <p role="status" class="flex items-center gap-2 text-sm text-muted-foreground">
      <Spinner /> Loading language…
    </p>
  {:else if loaded}
    <Field.Group>
      <Field.Field data-invalid={Boolean(error)}>
        <Field.Title id="language-label">Language</Field.Title>
        <ToggleGroup.Root
          type="single"
          bind:value={selectedLanguage}
          onValueChange={(value) => void changeLanguage(value)}
          disabled={saving}
          variant="outline"
          spacing={2}
          aria-labelledby="language-label"
          aria-describedby={error ? 'language-description language-error' : 'language-description'}
          aria-invalid={Boolean(error)}
        >
          <ToggleGroup.Item value="zh-CN" class="min-h-11" aria-label="Chinese"
            ><span lang="zh-CN">中文</span></ToggleGroup.Item
          >
          <ToggleGroup.Item value="en" class="min-h-11">English</ToggleGroup.Item>
        </ToggleGroup.Root>
        <Field.Description id="language-description"
          >Used for AI article summaries. Interface translations will follow. Existing summaries
          keep their language until you regenerate them.</Field.Description
        >
        {#if error}<Field.Error id="language-error">{error}</Field.Error>{/if}
      </Field.Field>
    </Field.Group>
  {/if}
  {#if error && !loaded}<StatusMessage tone="error">{error}</StatusMessage><Button
      variant="outline"
      onclick={() => void load()}>Retry language settings</Button
    >{/if}
  {#if saving}<p role="status" class="mt-3 text-sm text-muted-foreground">Saving language…</p>{/if}
  {#if success}<StatusMessage tone="success">{success}</StatusMessage>{/if}
</section>
