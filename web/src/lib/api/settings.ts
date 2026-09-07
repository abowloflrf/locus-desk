import { request } from './client';

export interface AiSettings {
  baseUrl: string;
  model: string;
  hasApiKey: boolean;
}

export function getAiSettings(signal?: AbortSignal): Promise<AiSettings> {
  return request('/settings/ai', { signal });
}

export function saveAiSettings(
  settings: { baseUrl: string; model: string; apiKey?: string },
  signal?: AbortSignal,
): Promise<AiSettings> {
  return request('/settings/ai', { method: 'PUT', body: settings, signal });
}

export function testAiSettings(
  settings: { baseUrl: string; model: string; apiKey?: string },
  signal?: AbortSignal,
): Promise<{ elapsedMs: number }> {
  return request('/settings/ai/test', { method: 'POST', body: settings, signal });
}
