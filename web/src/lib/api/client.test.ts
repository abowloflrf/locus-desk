import { afterEach, describe, expect, it, vi } from 'vitest';

import { advanceAuthGeneration, ApiClientError, request, setUnauthorizedHandler } from './client';

afterEach(() => {
  setUnauthorizedHandler(null);
  vi.unstubAllGlobals();
});

describe('API client', () => {
  it('parses the stable API error envelope', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({ error: { code: 'invalid_note', message: 'Note is empty.' } }),
          {
            headers: { 'content-type': 'application/json' },
            status: 422,
          },
        ),
      ),
    );

    await expect(request('/notes')).rejects.toEqual(
      expect.objectContaining({
        code: 'invalid_note',
        message: 'Note is empty.',
        status: 422,
      }),
    );
  });

  it('uses a safe message for malformed server errors', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response('<h1>Internal details</h1>', {
          headers: { 'content-type': 'text/html' },
          status: 500,
        }),
      ),
    );

    await expect(request('/notes')).rejects.toMatchObject({
      code: 'request_failed',
      message: 'The server could not complete the request.',
      status: 500,
    });
  });

  it('handles no-content responses without parsing JSON', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

    await expect(request<void>('/auth/logout', { method: 'POST' })).resolves.toBeUndefined();
  });

  it('notifies the authentication gate on unauthorized responses', async () => {
    const onUnauthorized = vi.fn();
    setUnauthorizedHandler(onUnauthorized);
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({ error: { code: 'unauthorized', message: 'Sign in required.' } }),
          {
            headers: { 'content-type': 'application/json' },
            status: 401,
          },
        ),
      ),
    );

    await expect(request('/notes')).rejects.toBeInstanceOf(ApiClientError);
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });

  it('ignores an unauthorized response from an older authentication generation', async () => {
    const onUnauthorized = vi.fn();
    setUnauthorizedHandler(onUnauthorized);
    let resolveResponse: ((response: Response) => void) | undefined;
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(
        () =>
          new Promise<Response>((resolve) => {
            resolveResponse = resolve;
          }),
      ),
    );

    const pending = request('/notes');
    advanceAuthGeneration();
    resolveResponse?.(
      new Response(JSON.stringify({ error: { code: 'unauthorized', message: 'Expired.' } }), {
        headers: { 'content-type': 'application/json' },
        status: 401,
      }),
    );

    await expect(pending).rejects.toBeInstanceOf(ApiClientError);
    expect(onUnauthorized).not.toHaveBeenCalled();
  });
});
