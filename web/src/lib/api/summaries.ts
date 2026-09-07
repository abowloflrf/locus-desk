import { request } from './client';
import type { Language } from './settings';

export interface ArticleSummary {
  content: string;
  language: Language;
  contentVersion: number;
}

export function getArticleSummary(
  uid: string,
  signal?: AbortSignal,
): Promise<ArticleSummary | null> {
  return request(`/library/${encodeURIComponent(uid)}/summary`, { signal });
}

export function generateArticleSummary(uid: string, signal?: AbortSignal): Promise<ArticleSummary> {
  return request(`/library/${encodeURIComponent(uid)}/summary`, { method: 'POST', signal });
}
