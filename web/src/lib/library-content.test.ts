import { afterEach, describe, expect, it } from 'vitest';

import { highlightLibraryHtml, safeLibrarySourceUrl, sanitizeLibraryHtml } from './library-content';

afterEach(() => {
  document.body.replaceChildren();
});

describe('Library content sanitization', () => {
  it('preserves meaningful hash headings, external links, and code when removing anchor decorations', () => {
    const source =
      '<h2>#</h2><h3>C#</h3><h2><a href="#symbol">#</a></h2><a href="https://other.example/article#symbol"><h2>Symbol</h2><h2>#</h2></a><pre><code><a href="#code"><h2>Code</h2><h2>#</h2></a></code></pre>';
    const template = document.createElement('template');
    template.innerHTML = sanitizeLibraryHtml(source, 'https://source.example/article');
    expect(
      [...template.content.querySelectorAll('h2, h3')].map((heading) => heading.textContent),
    ).toEqual(['#', 'C#', '#', 'Symbol', '#']);
    expect(template.content.querySelector('code')?.textContent).toBe('Code#');
  });

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

it('preserves validated language labels and highlights aliases without changing code text', async () => {
  const source =
    '<p data-language="js">Body</p><pre><code data-language="JS" class="sidebar">const x = &quot;&lt;script&gt;&quot;;\n  x;</code></pre><pre data-language="bad language">plain</pre><pre data-language="unknown">untouched</pre><pre>unlabelled</pre>';
  const template = document.createElement('template');
  template.innerHTML = await highlightLibraryHtml(source, 'https://example.com');
  const blocks = template.content.querySelectorAll('pre');
  expect(blocks[0]?.getAttribute('data-language')).toBe('js');
  expect(blocks[0]?.querySelector('span[class^="shiki-"]')).not.toBeNull();
  expect(blocks[0]?.textContent).toBe('const x = "<script>";\n  x;');
  expect(template.content.querySelector('script, .sidebar, p[data-language]')).toBeNull();
  expect(blocks[1]?.hasAttribute('data-language')).toBe(false);
  expect(blocks[2]?.innerHTML).toBe('untouched');
  expect(blocks[3]?.innerHTML).toBe('unlabelled');
});

it('preserves structural code lines, blank lines, indentation and escaping through highlighting', async () => {
  // Mirrors Mastra's Docusaurus output: token-line divs ending in br, without text newlines.
  const source =
    '<pre data-language="typescript"><code><div><span>const answer = 42;</span><br></div><div><span></span><br></div><div><span>  console.log(&quot;&lt;script&gt;&quot;, answer);</span><br></div></code></pre>';
  const expected = 'const answer = 42;\n\n  console.log("<script>", answer);\n';
  const sanitized = sanitizeLibraryHtml(source, 'https://example.com');
  const template = document.createElement('template');
  template.innerHTML = sanitized;
  expect(template.content.querySelector('code')?.textContent).toBe(expected);
  expect(sanitizeLibraryHtml(sanitized, 'https://example.com')).toBe(sanitized);
  template.innerHTML = await highlightLibraryHtml(sanitized, 'https://example.com');
  expect(template.content.querySelector('code')?.textContent).toBe(expected);
  expect(template.content.querySelector('code span[class^="shiki-"]')).not.toBeNull();
  expect(template.content.querySelector('script')).toBeNull();
});

it('preserves br-only and block-only lines without doubling existing newlines', async () => {
  const cases = [
    ['first<br><br>  last', 'first\n\n  last'],
    ['<div>first</div><div></div><div>  last</div>', 'first\n\n  last\n'],
    ['<div>first\n</div><div>  last\n</div>', 'first\n  last\n'],
    ['<div>first</div>\n<div>  last</div>', 'first\n  last\n'],
    ['first\n\n  last\n', 'first\n\n  last\n'],
  ];
  for (const [source, expected] of cases) {
    const template = document.createElement('template');
    template.innerHTML = await highlightLibraryHtml(
      `<pre data-language="unknown"><code>${source}</code></pre>`,
      'https://example.com',
    );
    expect(template.content.querySelector('code')?.textContent).toBe(expected);
  }
});
