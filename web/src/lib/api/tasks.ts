import { request } from './client';
import type {
  CreateTaskRequest,
  ListTasksParams,
  ListTasksResponse,
  Task,
  UpdateTaskRequest,
} from './types';

export function listTasks(
  params: ListTasksParams = {},
  signal?: AbortSignal,
): Promise<ListTasksResponse> {
  const query = new URLSearchParams();
  if (params.scope) query.set('scope', params.scope);
  if (params.status) query.set('status', params.status);

  const suffix = query.size ? `?${query.toString()}` : '';
  return request<ListTasksResponse>(`/tasks${suffix}`, { method: 'GET', signal });
}

export function createTask(payload: CreateTaskRequest): Promise<Task> {
  return request<Task>('/tasks', { body: payload, method: 'POST' });
}

export function updateTask(uid: string, payload: UpdateTaskRequest): Promise<Task> {
  return request<Task>(`/tasks/${encodeURIComponent(uid)}`, { body: payload, method: 'PATCH' });
}

export function deleteTask(uid: string): Promise<void> {
  return request<void>(`/tasks/${encodeURIComponent(uid)}`, { method: 'DELETE' });
}
