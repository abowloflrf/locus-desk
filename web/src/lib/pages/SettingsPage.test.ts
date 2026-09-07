import { mount, unmount, tick } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { getAiSettings, saveAiSettings, testAiSettings } from '../api/settings';
import SettingsPage from './SettingsPage.svelte';

vi.mock('../api/settings', () => ({
  getAiSettings: vi.fn(),
  saveAiSettings: vi.fn(),
  testAiSettings: vi.fn(),
}));
afterEach(() => {
  vi.resetAllMocks();
  document.body.replaceChildren();
});

it('preserves the saved key, retains failed edits, and clears the new key after saving', async () => {
  vi.mocked(getAiSettings).mockResolvedValue({
    baseUrl: 'https://models.example/v1',
    model: 'old-model',
    hasApiKey: true,
  });
  vi.mocked(saveAiSettings)
    .mockRejectedValueOnce(new Error('Unable to save settings.'))
    .mockResolvedValue({
      baseUrl: 'https://models.example/v1',
      model: 'new-model',
      hasApiKey: true,
    });
  const component = mount(SettingsPage, { target: document.body });
  try {
    await vi.waitFor(() => expect(document.querySelector('#ai-model')).not.toBeNull());
    const model = document.querySelector<HTMLInputElement>('#ai-model')!;
    const key = document.querySelector<HTMLInputElement>('#ai-api-key')!;
    expect(key.type).toBe('password');
    expect(key.value).toBe('');
    model.value = 'new-model';
    model.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    document
      .querySelector('form')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await vi.waitFor(() =>
      expect(document.querySelector('[role="alert"]')?.textContent).toContain('Unable to save'),
    );
    expect(vi.mocked(saveAiSettings).mock.calls[0][0]).toEqual({
      baseUrl: 'https://models.example/v1',
      model: 'new-model',
    });
    expect(model.value).toBe('new-model');
    key.value = 'replacement-key';
    key.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true }));
    await vi.waitFor(() => expect(document.body.textContent).toContain('Settings saved.'));
    expect(vi.mocked(saveAiSettings).mock.calls[1][0].apiKey).toBe('replacement-key');
    expect(key.value).toBe('');
  } finally {
    await unmount(component);
  }
});

it('blocks invalid URLs and focuses the field', async () => {
  vi.mocked(getAiSettings).mockResolvedValue({ baseUrl: '', model: '', hasApiKey: false });
  const component = mount(SettingsPage, { target: document.body });
  try {
    await vi.waitFor(() => expect(document.querySelector('form')).not.toBeNull());
    const input = document.querySelector<HTMLInputElement>('#ai-base-url')!;
    input.value = 'file:///tmp/model';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    document
      .querySelector('form')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(input.getAttribute('aria-invalid')).toBe('true'));
    expect(document.activeElement).toBe(input);
    expect(saveAiSettings).not.toHaveBeenCalled();
  } finally {
    await unmount(component);
  }
});

it('tests unsaved values, allows retry, and clears results when edited', async () => {
  vi.mocked(getAiSettings).mockResolvedValue({
    baseUrl: 'https://models.example/v1',
    model: 'saved',
    hasApiKey: true,
  });
  vi.mocked(testAiSettings)
    .mockRejectedValueOnce(new Error('The provider rejected authentication.'))
    .mockResolvedValueOnce({ elapsedMs: 250 });
  const component = mount(SettingsPage, { target: document.body });
  try {
    await vi.waitFor(() => expect(document.querySelector('#ai-model')).not.toBeNull());
    const model = document.querySelector<HTMLInputElement>('#ai-model')!;
    model.value = 'draft';
    model.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    const button = [...document.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('Test connection'),
    )!;
    button.click();
    await vi.waitFor(() => expect(document.body.textContent).toContain('rejected authentication'));
    expect(testAiSettings).toHaveBeenCalledWith(
      { baseUrl: 'https://models.example/v1', model: 'draft' },
      expect.any(AbortSignal),
    );
    button.click();
    await vi.waitFor(() =>
      expect(document.body.textContent).toContain(
        'Connection successful. Model responded in 0.25 s.',
      ),
    );
    expect(saveAiSettings).not.toHaveBeenCalled();
    expect(model.value).toBe('draft');
    model.value = 'another';
    model.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    expect(document.body.textContent).not.toContain('Connection successful');
  } finally {
    await unmount(component);
  }
});
