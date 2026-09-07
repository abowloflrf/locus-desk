import { describe, expect, it } from 'vitest';
import { createSummaryReveal, renderSummaryMarkdown } from './summary-markdown';

function rendered(source: string) {
  const element = document.createElement('div');
  element.innerHTML = renderSummaryMarkdown(source);
  return element;
}

describe('summary Markdown', () => {
  it('renders lists, bold, and inline code, including nested and ordered lists', () => {
    const element = rendered(
      'Main **finding** uses `cache`.\n\n- First **point**\n  - Nested `value`\n- Second point\n\n3. Third\n4. Fourth',
    );
    expect(element.querySelectorAll('strong')).toHaveLength(2);
    expect(element.querySelectorAll('code')).toHaveLength(2);
    expect(element.querySelector('ul ul li')?.textContent).toBe('Nested value');
    expect(element.querySelector('ol')?.getAttribute('start')).toBe('3');
    expect(element.querySelectorAll('ol li')).toHaveLength(2);
  });

  it('keeps unsupported syntax literal and prevents HTML, links, and embedded resources', () => {
    const source =
      '# Heading\n\n> Quote\n\n*italic* ~~deleted~~ [link](javascript:alert(1)) ![image](https://example.com/image)\n\n```js\nalert(1)\n```\n\n<img src=x onerror=alert(1)><strong>raw HTML</strong><script>alert(1)</script>\n\n| A | B |\n| - | - |\n| 1 | 2 |';
    const element = rendered(source);
    expect(
      element.querySelector('h1, blockquote, em, del, a, img, pre, table, script, strong'),
    ).toBeNull();
    expect(element.textContent).toContain('# Heading');
    expect(element.textContent).toContain('*italic*');
    expect(element.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(element.textContent).toContain('```js');
    expect(element.querySelector('[onerror], [href], [src]')).toBeNull();
  });

  it('reveals formatted text without incomplete Markdown or broken graphemes', () => {
    const reveal = createSummaryReveal('**你好👩‍💻** and `cache`\n\n- One\n- Two');
    expect(reveal.frame(0)).toBe('');
    expect(reveal.frame(1)).toBe(reveal.html);
    const element = document.createElement('div');
    let previous = '';
    for (const progress of [0.1, 0.25, 0.5, 0.75, 1]) {
      element.innerHTML = reveal.frame(progress);
      const text = element.textContent ?? '';
      expect(text.startsWith(previous)).toBe(true);
      expect(text).not.toMatch(/\*\*|`/);
      if (text.includes('👩')) expect(text).toContain('👩‍💻');
      expect([...element.querySelectorAll('li')].every((li) => li.textContent?.trim())).toBe(true);
      previous = text;
    }
    expect(element.querySelector('strong')?.textContent).toBe('你好👩‍💻');
    expect(element.querySelector('code')?.textContent).toBe('cache');
  });
});
