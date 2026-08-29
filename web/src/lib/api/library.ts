import { request } from './client';
import type {
  CreateLibraryItemRequest,
  LibraryContentResponse,
  LibraryItem,
  ListLibraryItemsParams,
  ListLibraryItemsResponse,
  UpdateLibraryItemRequest,
} from './types';

export function listLibraryItems(
  params: ListLibraryItemsParams = {},
  signal?: AbortSignal,
): Promise<ListLibraryItemsResponse> {
  const query = new URLSearchParams();

  if (params.status) query.set('status', params.status);
  if (params.q?.trim()) query.set('q', params.q.trim());
  if (params.tag?.trim()) query.set('tag', params.tag.trim());
  if (params.read !== undefined) query.set('read', String(params.read));
  if (params.starred !== undefined) query.set('starred', String(params.starred));
  if (params.page !== undefined) query.set('page', String(params.page));
  if (params.pageSize !== undefined) query.set('page_size', String(params.pageSize));

  const suffix = query.size ? `?${query.toString()}` : '';
  return request<ListLibraryItemsResponse>(`/library${suffix}`, { method: 'GET', signal });
}

export function createLibraryItem(payload: CreateLibraryItemRequest): Promise<LibraryItem> {
  return request<LibraryItem>('/library', { body: payload, method: 'POST' });
}

export function getLibraryItem(uid: string, signal?: AbortSignal): Promise<LibraryItem> {
  return request<LibraryItem>(`/library/${encodeURIComponent(uid)}`, { method: 'GET', signal });
}

export function getLibraryContent(
  uid: string,
  signal?: AbortSignal,
): Promise<LibraryContentResponse> {
  return request<LibraryContentResponse>(`/library/${encodeURIComponent(uid)}/content`, {
    method: 'GET',
    signal,
  });
}

export function retryLibraryItem(uid: string, signal?: AbortSignal): Promise<LibraryItem> {
  return request<LibraryItem>(`/library/${encodeURIComponent(uid)}/retry`, {
    method: 'POST',
    signal,
  });
}

export function updateLibraryItem(
  uid: string,
  payload: UpdateLibraryItemRequest,
): Promise<LibraryItem> {
  return request<LibraryItem>(`/library/${encodeURIComponent(uid)}`, {
    body: payload,
    method: 'PATCH',
  });
}

export function deleteLibraryItem(uid: string): Promise<void> {
  return request<void>(`/library/${encodeURIComponent(uid)}`, { method: 'DELETE' });
}
