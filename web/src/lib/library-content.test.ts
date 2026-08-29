import { afterEach, describe, expect, it } from 'vitest';

import { safeLibrarySourceUrl, sanitizeLibraryHtml } from './library-content';

afterEach(() => {
  document.body.replaceChildren();
});

describe('Library content sanitization', () => {
  it('applies an explicit HTML allowlist and removes executable content', () => {
    const html = sanitizeLibraryHtml(
      `
        <script>alert('xss')</script>
        <svg><script>alert('svg')</script></svg>
        <img src="https://tracker.example/pixel" onerror="alert('image')">
        <p class="remote" id="override" style="position:fixed" onclick="alert('event')">
          Safe <strong>reading</strong>
        </p>
        <form action="https://evil.example"><input name="token"></form>
      `,
      'https://source.example/articles/one',
    );
    const template = document.createElement('template');
    template.innerHTML = html;

    expect(template.content.textContent).toContain('Safe reading');
    expect(template.content.querySelector('strong')).not.toBeNull();
    expect(template.content.querySelector('script, svg, img, form, input')).toBeNull();
    expect(template.content.querySelector('p')?.attributes).toHaveLength(0);
  });

  it('keeps only resolved http(s) links and isolates every outgoing navigation', () => {
    const html = sanitizeLibraryHtml(
      `
        <a data-kind="safe" href="/related">Relative</a>
        <a href="https://outside.example/read" target="self">External</a>
        <a href="javascript:alert(1)">Script</a>
        <a href="data:text/html,boom">Data</a>
        <a href="mailto:reader@example.com">Mail</a>
      `,
      'https://source.example/articles/one',
    );
    const template = document.createElement('template');
    template.innerHTML = html;
    const anchors = [...template.content.querySelectorAll('a')];

    expect(anchors[0]?.outerHTML).toBe(
      '<a href="https://source.example/related" rel="noopener noreferrer" target="_blank">Relative</a>',
    );
    expect(anchors[1]?.getAttribute('href')).toBe('https://outside.example/read');
    expect(anchors[1]?.getAttribute('rel')).toBe('noopener noreferrer');
    expect(anchors[1]?.getAttribute('target')).toBe('_blank');
    for (const anchor of anchors.slice(2)) {
      expect(anchor.hasAttribute('href')).toBe(false);
      expect(anchor.hasAttribute('target')).toBe(false);
      expect(anchor.hasAttribute('rel')).toBe(false);
    }
  });

  it('rejects non-web protocols and deceptive user-info URLs', () => {
    expect(safeLibrarySourceUrl('file:///tmp/article.html')).toBeNull();
    expect(safeLibrarySourceUrl('https://trusted.example@evil.example/read')).toBeNull();
    expect(safeLibrarySourceUrl('https://reader:secret@example.com/read')).toBeNull();
    expect(safeLibrarySourceUrl('https://example.com/read')).toBe('https://example.com/read');
  });
});
