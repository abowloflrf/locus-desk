export const READER_PREFERENCES_STORAGE_KEY = 'locus.reader.preferences.v1';

export const READER_FONT_PRESETS = [
  { label: 'Plex', value: 'plex' },
  { label: 'Atkinson', value: 'atkinson' },
  { label: 'System', value: 'system' },
] as const;

export const READER_FONT_SIZES = [
  { label: '16', value: 'small' },
  { label: '18', value: 'medium' },
  { label: '20', value: 'large' },
] as const;

export const READER_LINE_HEIGHTS = [
  { label: 'Tight', value: 'compact' },
  { label: 'Comfort', value: 'comfortable' },
  { label: 'Airy', value: 'spacious' },
] as const;

export const READER_WIDTHS = [
  { label: 'Narrow', value: 'narrow' },
  { label: 'Balanced', value: 'balanced' },
  { label: 'Wide', value: 'wide' },
] as const;

export type ReaderFontPreset = (typeof READER_FONT_PRESETS)[number]['value'];
export type ReaderFontSize = (typeof READER_FONT_SIZES)[number]['value'];
export type ReaderLineHeight = (typeof READER_LINE_HEIGHTS)[number]['value'];
export type ReaderWidth = (typeof READER_WIDTHS)[number]['value'];

export interface ReaderPreferences {
  fontPreset: ReaderFontPreset;
  fontSize: ReaderFontSize;
  lineHeight: ReaderLineHeight;
  width: ReaderWidth;
}

export const DEFAULT_READER_PREFERENCES: ReaderPreferences = {
  fontPreset: 'plex',
  fontSize: 'medium',
  lineHeight: 'comfortable',
  width: 'balanced',
};

export function loadReaderPreferences(storage: Pick<Storage, 'getItem'>): ReaderPreferences {
  try {
    const stored = storage.getItem(READER_PREFERENCES_STORAGE_KEY);
    if (!stored) return { ...DEFAULT_READER_PREFERENCES };
    const value = JSON.parse(stored) as Record<string, unknown>;
    return {
      fontPreset: isOption(READER_FONT_PRESETS, value.fontPreset)
        ? value.fontPreset
        : DEFAULT_READER_PREFERENCES.fontPreset,
      fontSize: isOption(READER_FONT_SIZES, value.fontSize)
        ? value.fontSize
        : DEFAULT_READER_PREFERENCES.fontSize,
      lineHeight: isOption(READER_LINE_HEIGHTS, value.lineHeight)
        ? value.lineHeight
        : DEFAULT_READER_PREFERENCES.lineHeight,
      width: isOption(READER_WIDTHS, value.width) ? value.width : DEFAULT_READER_PREFERENCES.width,
    };
  } catch {
    return { ...DEFAULT_READER_PREFERENCES };
  }
}

export function saveReaderPreferences(
  storage: Pick<Storage, 'setItem'>,
  preferences: ReaderPreferences,
): boolean {
  try {
    storage.setItem(READER_PREFERENCES_STORAGE_KEY, JSON.stringify(preferences));
    return true;
  } catch {
    return false;
  }
}

function isOption<T extends string>(options: readonly { value: T }[], value: unknown): value is T {
  return typeof value === 'string' && options.some((option) => option.value === value);
}
