import { mount, unmount, tick } from 'svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { getArticleSummary, generateArticleSummary } from '../api/summaries';
import ArticleSummary from './ArticleSummary.svelte';
vi.mock('../api/summaries', () => ({
  getArticleSummary: vi.fn(),
  generateArticleSummary: vi.fn(),
}));
beforeEach(() => {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: true })),
  );
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetAllMocks();
  document.body.replaceChildren();
});
function button(label: string) {
  return [...document.querySelectorAll('button')].find(
    (node) => node.textContent?.trim() === label,
  )!;
}
it('generates only on click, collapses, and preserves a summary on regeneration failure', async () => {
  vi.mocked(getArticleSummary).mockResolvedValue(null);
  vi.mocked(generateArticleSummary)
    .mockResolvedValueOnce({
      content: 'Summary <script>unsafe</script>',
      language: 'zh-CN',
      contentVersion: 1,
    })
    .mockRejectedValueOnce(new Error('Provider unavailable.'));
  const component = mount(ArticleSummary, {
    target: document.body,
    props: { uid: 'article-a', contentVersion: 1 },
  });
  try {
    await vi.waitFor(() => expect(button('Generate summary')).toBeTruthy());
    expect(generateArticleSummary).not.toHaveBeenCalled();
    button('Generate summary').click();
    await vi.waitFor(() =>
      expect(document.body.textContent).toContain('Summary <script>unsafe</script>'),
    );
    expect(document.querySelector('script')).toBeNull();
    expect(document.querySelector('.summary-text')?.getAttribute('lang')).toBe('zh-CN');
    const collapse = document.querySelector<HTMLButtonElement>(
      '[aria-label="Collapse AI summary"]',
    )!;
    collapse.click();
    await tick();
    expect(collapse.getAttribute('aria-expanded')).toBe('false');
    expect(document.querySelector('.summary-text')?.closest('[hidden]')).not.toBeNull();
    collapse.click();
    await tick();
    await vi.waitFor(() => expect(button('Regenerate').disabled).toBe(false));
    button('Regenerate').click();
    await vi.waitFor(() =>
      expect(document.querySelector('[role="alert"]')?.textContent).toContain(
        'Provider unavailable.',
      ),
    );
    expect(document.querySelector('.summary-text')?.textContent).toContain('Summary');
    expect(generateArticleSummary).toHaveBeenCalledTimes(2);
  } finally {
    await unmount(component);
  }
});
it('loads saved summaries without generating and warns about a changed article', async () => {
  vi.mocked(getArticleSummary).mockResolvedValue({
    content: 'Saved summary',
    language: 'en',
    contentVersion: 1,
  });
  const component = mount(ArticleSummary, {
    target: document.body,
    props: { uid: 'article-a', contentVersion: 2 },
  });
  try {
    await vi.waitFor(() => expect(document.body.textContent).toContain('The article has changed.'));
    expect(generateArticleSummary).not.toHaveBeenCalled();
  } finally {
    await unmount(component);
  }
});
it('prevents duplicate requests and aborts the client request when closed', async () => {
  vi.mocked(getArticleSummary).mockResolvedValue(null);
  let resolve!: (value: { content: string; language: 'en'; contentVersion: number }) => void;
  vi.mocked(generateArticleSummary).mockReturnValue(new Promise((done) => (resolve = done)));
  const component = mount(ArticleSummary, {
    target: document.body,
    props: { uid: 'article-a', contentVersion: 1 },
  });
  await vi.waitFor(() => expect(button('Generate summary')).toBeTruthy());
  button('Generate summary').click();
  await tick();
  expect(button('Generating…').disabled).toBe(true);
  button('Generating…').click();
  expect(generateArticleSummary).toHaveBeenCalledTimes(1);
  const signal = vi.mocked(generateArticleSummary).mock.calls[0][1]!;
  await unmount(component);
  expect(signal.aborted).toBe(true);
  resolve({ content: 'Late summary', language: 'en', contentVersion: 1 });
  await tick();
  expect(document.body.textContent).not.toContain('Late summary');
});

it.each([false, true])(
  'reveals new text progressively while respecting reduced motion (%s)',
  async (reducedMotion) => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: reducedMotion })),
    );
    let frame: FrameRequestCallback | undefined;
    vi.stubGlobal(
      'requestAnimationFrame',
      vi.fn((callback: FrameRequestCallback) => {
        frame = callback;
        return 1;
      }),
    );
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    vi.mocked(getArticleSummary).mockResolvedValue(null);
    const content = 'Summary with 中文 and emoji 👩‍💻. '.repeat(20);
    vi.mocked(generateArticleSummary).mockResolvedValue({
      content,
      language: 'en',
      contentVersion: 1,
    });
    const component = mount(ArticleSummary, {
      target: document.body,
      props: { uid: 'article-a', contentVersion: 1 },
    });
    try {
      await vi.waitFor(() => expect(button('Generate summary')).toBeTruthy());
      button('Generate summary').click();
      await vi.waitFor(() => expect(document.querySelector('.summary-text')).not.toBeNull());
      const visible = document.querySelector('.summary-markdown')!;
      if (reducedMotion) {
        expect(visible.textContent?.trim()).toBe(content.trim());
        expect(requestAnimationFrame).not.toHaveBeenCalled();
      } else {
        expect(visible.textContent).toBe('');
        frame!(performance.now() + 200);
        await tick();
        expect(visible.textContent!.length).toBeGreaterThan(0);
        expect(visible.textContent!.length).toBeLessThan(content.length);
        frame!(performance.now() + 2400);
        await tick();
        expect(visible.textContent?.trim()).toBe(content.trim());
        expect(button('Regenerate').disabled).toBe(false);
      }
    } finally {
      await unmount(component);
      expect(cancelAnimationFrame).toHaveBeenCalled();
      vi.unstubAllGlobals();
    }
  },
);
