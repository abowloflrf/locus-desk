import { describe, expect, it } from 'vitest';

import { renderMarkdown, renderHighlightedMarkdown } from './markdown';

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
    expect(html).toContain('<pre><span>ts</span><code>');
  });

  it('highlights language aliases without changing code text or allowing executable HTML', async () => {
    const code = 'const html = "<img src=x onerror=alert(1)>";';
    const template = document.createElement('template');
    template.innerHTML = await renderHighlightedMarkdown('```ts\n' + code + '\n```');
    expect(template.content.querySelector('.shiki-fg-ab5959')?.textContent?.trim()).toBe('const');
    expect(template.content.querySelector('.shiki-fg-b56959')).not.toBeNull();
    expect(template.content.querySelector('code')?.textContent).toBe(code + '\n');
    expect(template.content.querySelector('img, script, [style]')).toBeNull();
  });

  it('loads the supported languages and preserves multiline code', async () => {
    const examples = {
      bash: 'echo "hello"',
      css: '.a { color: red; }',
      c: 'int main(void) { return 0; }',
      'c++': 'int main() { return 0; }',
      cs: 'public class Example { }',
      diff: '-old line\n+new line',
      docker: 'FROM alpine:latest',
      gql: 'query { user(id: 1) { name } }',
      html: '<script>const n = 1;</script>',
      ini: '[section]\nvalue=42',
      java: 'public class Example { }',
      jsx: 'const el = <div title="hello">Hello</div>;',
      md: '# Heading\n**bold**',
      nginx: 'server { listen 80; }',
      proto: 'syntax = "proto3";',
      svelte: '<script lang="ts">let n: number = 1;</script>\n<p>{n}</p>',
      toml: '[package]\nname = "example"',
      tsx: 'const el: JSX.Element = <div>Hello</div>;',
      vue: '<script setup lang="ts">const n: number = 1;</script>\n<template><p>{{ n }}</p></template>',
      go: 'package main',
      js: 'const n = 1;',
      json: '{"n": 1}',
      py: 'return True',
      rust: 'let n = 1;',
      sql: 'SELECT 1;',
      ts: 'const n: number = 1;',
      xml: '<root>hello</root>',
      yml: 'n: true',
    };
    await Promise.all(
      Object.entries(examples).map(async ([language, sample]) => {
        const code = sample + '\n\n' + sample;
        const template = document.createElement('template');
        template.innerHTML = await renderHighlightedMarkdown(
          '```' + language + '\n' + code + '\n```',
        );
        expect(template.content.querySelector('code')?.textContent).toBe(code + '\n');
        expect(template.content.querySelector('code span'), language).not.toBeNull();
      }),
    );
  });

  it('keeps unlabelled, unsupported, and oversized code blocks readable as plain text', async () => {
    for (const [language, code] of [
      ['', '<div>hello</div>'],
      ['unknown-language', 'x < 3'],
      ['js', 'x'.repeat(20_001)],
    ]) {
      const template = document.createElement('template');
      template.innerHTML = await renderHighlightedMarkdown(
        '```' + language + '\n' + code + '\n```',
      );
      expect(template.content.querySelector('code')?.textContent).toBe(code + '\n');
      expect(template.content.querySelector('code span')).toBeNull();
    }
  });

  it('only retains scoped highlighter classes in code blocks', () => {
    const template = document.createElement('template');
    template.innerHTML = renderMarkdown(
      '<span class="shiki-fg-ab5959 sidebar">Outside</span><pre><code><span class="shiki-fg-b56959 sidebar fixed">Inside</span></code></pre>',
    );
    expect(template.content.querySelector('span')?.hasAttribute('class')).toBe(false);
    expect(template.content.querySelector('code span')?.className).toBe('shiki-fg-b56959');
  });

  it('labels annotated code blocks safely without including fence metadata or changing code', () => {
    for (const language of ['js', 'unknown-language', '<img/src=x/onerror=alert(1)>']) {
      const template = document.createElement('template');
      template.innerHTML = renderMarkdown('```' + language + ' title=example\nhello\n```');
      expect(template.content.querySelector('pre > span')?.textContent).toBe(language);
      expect(template.content.querySelector('code')?.textContent).toBe('hello\n');
      expect(template.content.querySelector('img')).toBeNull();
    }
    const template = document.createElement('template');
    template.innerHTML = renderMarkdown('```\nhello\n```');
    expect(template.content.querySelector('pre > span')).toBeNull();
  });

  it('preserves ordered list start numbers and nested lists', () => {
    const template = document.createElement('template');
    template.innerHTML = renderMarkdown(
      '3. Third\n4. Fourth\n   - Nested bullet\n   - Another bullet',
    );
    expect(template.content.querySelector('ol')?.getAttribute('start')).toBe('3');
    expect(template.content.querySelectorAll('ol > li')).toHaveLength(2);
    expect(template.content.querySelectorAll('ol > li > ul > li')).toHaveLength(2);
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
