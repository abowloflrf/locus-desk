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
        <img src="/media/hero.jpg" alt="Article illustration" width="1200" height="800" onerror="alert('image')">
        <img src="data:image/png;base64,tracking" alt="Embedded tracker">
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
    expect(template.content.querySelector('script, svg, form, input')).toBeNull();
    expect(template.content.querySelector('p')?.attributes).toHaveLength(0);
    const image = template.content.querySelector('img');
    expect(image?.src).toBe('https://source.example/media/hero.jpg');
    expect(image?.alt).toBe('Article illustration');
    expect(image?.getAttribute('loading')).toBe('lazy');
    expect(image?.getAttribute('decoding')).toBe('async');
    expect(image?.getAttribute('referrerpolicy')).toBe('no-referrer');
    expect(image?.hasAttribute('onerror')).toBe(false);
    expect(template.content.querySelectorAll('img')).toHaveLength(1);
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

  it('preserves safe article semantics and layout attributes', () => {
    const html = sanitizeLibraryHtml(
      `
        <article lang="zh-CN" dir="ltr">
          <header><address>By Ada</address></header>
          <section>
            <aside><abbr title="Application programming interface">API</abbr></aside>
            <dl><dt>Term</dt><dd>Definition</dd></dl>
            <details open><summary>Notes</summary><mark>Important</mark></details>
            <ol start="3"><li><kbd>Ctrl</kbd> <samp>Output</samp> <var>value</var></li></ol>
            <blockquote cite="https://source.example/original"><q cite="https://source.example/quote">Quoted</q></blockquote>
            <table>
              <caption>Results</caption>
              <colgroup><col></colgroup>
              <tbody><tr><th colspan="2">Heading</th><td rowspan="2">Value</td></tr></tbody>
            </table>
            <p><small>Small</small> <s>Old</s> <u>Underlined</u> <ins>New</ins><wbr></p>
          </section>
        </article>
      `,
      'https://source.example/articles/one',
    );
    const template = document.createElement('template');
    template.innerHTML = html;
    const article = template.content.querySelector('article')!;

    expect(article.lang).toBe('zh-CN');
    expect(article.dir).toBe('ltr');
    for (const selector of [
      'header address',
      'section aside abbr',
      'dl > dt',
      'dl > dd',
      'details[open] > summary',
      'mark',
      'kbd',
      'samp',
      'var',
      'blockquote > q',
      'caption',
      'colgroup > col',
      'small',
      's',
      'u',
      'ins',
      'wbr',
    ]) {
      expect(article.querySelector(selector), selector).not.toBeNull();
    }
    expect(article.querySelector('ol')?.start).toBe(3);
    expect(article.querySelector('th')?.colSpan).toBe(2);
    expect(article.querySelector('td')?.rowSpan).toBe(2);
    expect(article.querySelector('blockquote')?.cite).toBe('https://source.example/original');
    expect(article.querySelector('q')?.cite).toBe('https://source.example/quote');
  });

  it('rejects non-web protocols and deceptive user-info URLs', () => {
    expect(safeLibrarySourceUrl('file:///tmp/article.html')).toBeNull();
    expect(safeLibrarySourceUrl('https://trusted.example@evil.example/read')).toBeNull();
    expect(safeLibrarySourceUrl('https://reader:secret@example.com/read')).toBeNull();
    expect(safeLibrarySourceUrl('https://example.com/read')).toBe('https://example.com/read');
  });
});
