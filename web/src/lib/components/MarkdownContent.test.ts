import { mount, tick, unmount } from 'svelte';
import { fromStore, writable } from 'svelte/store';
import { afterEach, expect, it, vi } from 'vitest';
import { renderHighlightedMarkdown } from '../markdown';
import MarkdownContent from './MarkdownContent.svelte';

vi.mock('../markdown', async (original) => ({
  ...(await original<typeof import('../markdown')>()),
  renderHighlightedMarkdown: vi.fn(),
}));

let component: ReturnType<typeof mount> | undefined;
afterEach(async () => {
  if (component) await unmount(component);
  component = undefined;
  document.body.replaceChildren();
  vi.mocked(renderHighlightedMarkdown).mockReset();
});

it('keeps edited content visible when an older highlight completes later', async () => {
  const pending = new Map<string, (html: string) => void>();
  vi.mocked(renderHighlightedMarkdown).mockImplementation(
    (source) =>
      new Promise((resolve) => {
        pending.set(source, resolve);
      }),
  );
  const content = writable('Old memo');
  const reactive = fromStore(content);
  component = mount(MarkdownContent, {
    target: document.body,
    props: {
      get content() {
        return reactive.current;
      },
    },
  });
  await tick();
  expect(document.body.textContent).toBe('Old memo\n');
  content.set('New memo');
  await tick();
  expect(document.body.textContent).toBe('New memo\n');
  pending.get('New memo')!('<p>New highlighted memo</p>');
  await tick();
  pending.get('Old memo')!('<p>Old highlighted memo</p>');
  await tick();
  expect(document.body.textContent).toBe('New highlighted memo');
});

it('leaves readable content when asynchronous highlighting fails', async () => {
  vi.mocked(renderHighlightedMarkdown).mockRejectedValue(new Error('Language download failed'));
  component = mount(MarkdownContent, {
    target: document.body,
    props: { content: '```ts\nconst n = 1;\n```' },
  });
  await tick();
  await tick();
  expect(document.querySelector('code')?.textContent).toBe('const n = 1;\n');
});
