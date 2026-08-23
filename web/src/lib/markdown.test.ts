import { describe, expect, it } from 'vitest';

import { renderMarkdown } from './markdown';

describe('Markdown rendering', () => {
  it('renders the required GFM structures', () => {
    const html = renderMarkdown(`# Heading

> Quote

- item
- [x] finished

\`\`\`ts
const answer = 42;
\`\`\``);

    expect(html).toContain('<h1>Heading</h1>');
    expect(html).toContain('<blockquote>');
    expect(html).toContain('type="checkbox"');
    expect(html).toContain('disabled');
    expect(html).toContain('<pre><code>');
  });

  it('removes stored-XSS vectors from raw HTML and links', () => {
    const html = renderMarkdown(`<script>alert(1)</script>
<img src=x onerror="alert(2)">
<svg onload="alert(3)"></svg>
<button autofocus>Fake action</button>
[unsafe](javascript:alert)`);

    const template = document.createElement('template');
    template.innerHTML = html;

    expect(html).not.toMatch(/<script|<img|<svg|<button|autofocus|onerror|onload/i);
    expect(template.content.querySelector('a[href^="javascript:"]')).toBeNull();
  });

  it('removes application classes that could cover the workspace', () => {
    const html = renderMarkdown(
      '<p class="drawer-backdrop visible sidebar today-rail open">Blocked</p>',
    );
    const template = document.createElement('template');
    template.innerHTML = html;

    expect(template.content.querySelector('p')?.getAttribute('class')).toBeNull();
  });

  it('hardens safe links and opens only external web links in a new tab', () => {
    const html = renderMarkdown(
      '[external](https://example.com) [internal](/archive) [mail](mailto:user@example.com)',
    );
    const template = document.createElement('template');
    template.innerHTML = html;
    const links = [...template.content.querySelectorAll('a')];

    expect(links[0]?.getAttribute('rel')).toBe('noopener noreferrer');
    expect(links[0]?.getAttribute('target')).toBe('_blank');
    expect(links[1]?.getAttribute('target')).toBeNull();
    expect(links[1]?.getAttribute('href')).toBe('/archive');
    expect(links[2]?.getAttribute('target')).toBeNull();
  });
});
