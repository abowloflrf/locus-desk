import { request } from './client';
import type {
  CreateNoteRequest,
  ListNotesParams,
  ListNotesResponse,
  ListTagsResponse,
  Note,
  UpdateNoteRequest,
} from './types';

export function listNotes(
  params: ListNotesParams = {},
  signal?: AbortSignal,
): Promise<ListNotesResponse> {
  const query = new URLSearchParams();

  if (params.status) query.set('status', params.status);
  if (params.q?.trim()) query.set('q', params.q.trim());
  if (params.tag?.trim()) query.set('tag', params.tag.trim());
  if (params.page !== undefined) query.set('page', String(params.page));
  if (params.pageSize !== undefined) query.set('page_size', String(params.pageSize));

  const suffix = query.size ? `?${query.toString()}` : '';
  return request<ListNotesResponse>(`/notes${suffix}`, { method: 'GET', signal });
}

export function createNote(payload: CreateNoteRequest): Promise<Note> {
  return request<Note>('/notes', { body: payload, method: 'POST' });
}

export function updateNote(uid: string, payload: UpdateNoteRequest): Promise<Note> {
  return request<Note>(`/notes/${encodeURIComponent(uid)}`, { body: payload, method: 'PATCH' });
}

export function deleteNote(uid: string): Promise<void> {
  return request<void>(`/notes/${encodeURIComponent(uid)}`, { method: 'DELETE' });
}

export function listTags(signal?: AbortSignal): Promise<ListTagsResponse> {
  return request<ListTagsResponse>('/tags', { method: 'GET', signal });
}
