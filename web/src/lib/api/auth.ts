import { request } from './client';
import type { LoginRequest, SessionInfo } from './types';

export function getSession(signal?: AbortSignal): Promise<SessionInfo> {
  return request<SessionInfo>('/auth/me', { method: 'GET', signal });
}

export function login(credentials: LoginRequest): Promise<SessionInfo> {
  return request<SessionInfo>('/auth/login', {
    body: credentials,
    method: 'POST',
    skipUnauthorizedHandler: true,
  });
}

export function logout(): Promise<void> {
  return request<void>('/auth/logout', { method: 'POST' });
}
