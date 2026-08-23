import type { ApiErrorPayload } from './types';

const API_PREFIX = '/api/v1';

let unauthorizedHandler: (() => void) | null = null;
let authGeneration = 0;

export class ApiClientError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = 'ApiClientError';
  }
}

export function setUnauthorizedHandler(handler: (() => void) | null): void {
  unauthorizedHandler = handler;
}

export function advanceAuthGeneration(): void {
  authGeneration += 1;
}

export interface RequestOptions extends Omit<RequestInit, 'body'> {
  body?: unknown;
  skipUnauthorizedHandler?: boolean;
}

export async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const requestAuthGeneration = authGeneration;
  const { body, headers: customHeaders, skipUnauthorizedHandler, ...init } = options;
  const headers = new Headers(customHeaders);
  headers.set('Accept', 'application/json');

  if (body !== undefined) {
    headers.set('Content-Type', 'application/json');
  }

  const response = await fetch(`${API_PREFIX}${path}`, {
    ...init,
    body: body === undefined ? undefined : JSON.stringify(body),
    credentials: 'same-origin',
    headers,
  });

  if (
    response.status === 401 &&
    !skipUnauthorizedHandler &&
    requestAuthGeneration === authGeneration
  ) {
    unauthorizedHandler?.();
  }

  if (!response.ok) {
    throw await parseApiError(response);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const contentType = response.headers.get('content-type') ?? '';
  if (!contentType.toLowerCase().includes('application/json')) {
    throw new ApiClientError(
      response.status,
      'invalid_response',
      'The server returned an invalid response.',
    );
  }

  return (await response.json()) as T;
}

async function parseApiError(response: Response): Promise<ApiClientError> {
  const fallback =
    response.status >= 500 ? 'The server could not complete the request.' : 'Request failed.';
  const contentType = response.headers.get('content-type') ?? '';

  if (contentType.toLowerCase().includes('application/json')) {
    try {
      const payload = (await response.json()) as Partial<ApiErrorPayload>;
      if (payload.error?.code && payload.error.message) {
        return new ApiClientError(response.status, payload.error.code, payload.error.message);
      }
    } catch {
      // Fall through to a stable client-facing error.
    }
  }

  return new ApiClientError(response.status, 'request_failed', fallback);
}

export function errorMessage(cause: unknown, fallback = 'Something went wrong.'): string {
  return cause instanceof Error && cause.message ? cause.message : fallback;
}
