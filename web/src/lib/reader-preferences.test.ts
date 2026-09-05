import { describe, expect, it } from 'vitest';

import {
  DEFAULT_READER_PREFERENCES,
  loadReaderPreferences,
  READER_PREFERENCES_STORAGE_KEY,
  saveReaderPreferences,
} from './reader-preferences';

describe('reader preferences', () => {
  it('loads valid preferences and repairs invalid values', () => {
    const storage = {
      getItem: () =>
        JSON.stringify({ fontPreset: 'sans', fontSize: 'huge', lineHeight: 'spacious' }),
    };

    expect(loadReaderPreferences(storage)).toEqual({
      fontPreset: 'sans',
      fontSize: DEFAULT_READER_PREFERENCES.fontSize,
      lineHeight: 'spacious',
      width: DEFAULT_READER_PREFERENCES.width,
    });
  });

  it('falls back when stored data is missing or malformed', () => {
    expect(loadReaderPreferences({ getItem: () => null })).toEqual(DEFAULT_READER_PREFERENCES);
    expect(loadReaderPreferences({ getItem: () => '{' })).toEqual(DEFAULT_READER_PREFERENCES);
  });

  it('persists preferences under the versioned browser key', () => {
    const values = new Map<string, string>();
    const preferences = {
      fontPreset: 'sans',
      fontSize: 'large',
      lineHeight: 'compact',
      width: 'wide',
    } as const;

    expect(
      saveReaderPreferences({ setItem: (key, value) => values.set(key, value) }, preferences),
    ).toBe(true);
    expect(JSON.parse(values.get(READER_PREFERENCES_STORAGE_KEY)!)).toEqual(preferences);
    expect(loadReaderPreferences({ getItem: (key) => values.get(key) ?? null })).toEqual(
      preferences,
    );
  });
});
