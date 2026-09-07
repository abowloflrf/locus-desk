import { mount, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { getGeneralSettings, saveGeneralSettings } from '../api/settings';
import GeneralSettings from './GeneralSettings.svelte';
vi.mock('../api/settings', () => ({ getGeneralSettings: vi.fn(), saveGeneralSettings: vi.fn() }));
afterEach(() => {
  vi.resetAllMocks();
  document.body.replaceChildren();
});
it('saves language independently and preserves the saved value on failure', async () => {
  vi.mocked(getGeneralSettings).mockResolvedValue({ language: 'en' });
  vi.mocked(saveGeneralSettings)
    .mockRejectedValueOnce(new Error('Save failed.'))
    .mockResolvedValueOnce({ language: 'zh-CN' });
  const component = mount(GeneralSettings, { target: document.body });
  try {
    await vi.waitFor(() => expect(document.querySelector('[aria-label="Chinese"]')).not.toBeNull());
    const chinese = document.querySelector<HTMLButtonElement>('[aria-label="Chinese"]')!;
    chinese.click();
    await vi.waitFor(() => expect(document.body.textContent).toContain('Save failed.'));
    expect(chinese.getAttribute('data-state')).toBe('off');
    chinese.click();
    await vi.waitFor(() => expect(document.body.textContent).toContain('Language saved.'));
    expect(chinese.getAttribute('data-state')).toBe('on');
    expect(vi.mocked(saveGeneralSettings).mock.calls[1][0]).toEqual({ language: 'zh-CN' });
  } finally {
    await unmount(component);
  }
});
