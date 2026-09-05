import DOMPurify from 'dompurify';
import { Marked, Renderer } from 'marked';
import { highlightCode, isSyntaxClass } from './code-highlight';

export async function renderHighlightedMarkdown(source: string): Promise<string> {
  const parser = new Marked();
  const highlights = new Map<string, string>();
  const jobs: Promise<void>[] = [];
  parser.walkTokens(parser.lexer(source), (token) => {
    if (token.type === 'code') {
      jobs.push(
        highlightCode(token.text, token.lang ?? '').then((html) => {
          if (html !== null) highlights.set(`${token.lang ?? ''}\0${token.text}`, html);
        }),
      );
    }
  });
  await Promise.all(jobs);
  return renderMarkdown(source, highlights);
}

const SAFE_LINK_PROTOCOLS = new Set(['http:', 'https:', 'mailto:']);

export function renderMarkdown(source: string, highlights = new Map<string, string>()): string {
  const markdown = new Marked({
    renderer: {
      code(token) {
        const highlighted = highlights.get(`${token.lang ?? ''}\0${token.text}`) ?? null;
        const block =
          highlighted === null
            ? new Renderer().code(token)
            : `<pre><code>${highlighted}\n</code></pre>`;
        const language = token.lang?.trim().split(/\s+/, 1)[0];
        if (!language) return block;
        const label = document.createElement('span');
        label.textContent = language;
        return block.replace('<pre>', `<pre>${label.outerHTML}`);
      },
    },
  });

  const rendered = markdown.parse(source, {
    async: false,
    breaks: true,
    gfm: true,
  });
  const clean = DOMPurify.sanitize(rendered, {
    ALLOWED_ATTR: ['class', 'checked', 'disabled', 'href', 'start', 'title', 'type'],
    ALLOWED_TAGS: [
      'a',
      'blockquote',
      'br',
      'code',
      'del',
      'em',
      'h1',
      'h2',
      'h3',
      'h4',
      'h5',
      'h6',
      'hr',
      'input',
      'li',
      'ol',
      'p',
      'pre',
      'strong',
      'span',
      'table',
      'tbody',
      'td',
      'th',
      'thead',
      'tr',
      'ul',
    ],
    ALLOW_ARIA_ATTR: false,
    ALLOW_DATA_ATTR: false,
    FORBID_ATTR: ['style', 'srcset'],
  });

  const template = document.createElement('template');
  template.innerHTML = clean;

  // Keep only syntax token classes inside code, never user-supplied application classes.
  for (const element of template.content.querySelectorAll('[class]')) {
    const classes = element.matches('pre code span')
      ? [...element.classList].filter(isSyntaxClass)
      : [];
    if (classes.length) element.setAttribute('class', classes.join(' '));
    else element.removeAttribute('class');
  }

  for (const anchor of template.content.querySelectorAll('a')) {
    const href = anchor.getAttribute('href');
    if (!href || !isSafeLink(href)) {
      anchor.removeAttribute('href');
      anchor.removeAttribute('target');
      continue;
    }

    anchor.setAttribute('rel', 'noopener noreferrer');
    if (isExternalLink(href)) {
      anchor.setAttribute('target', '_blank');
    } else {
      anchor.removeAttribute('target');
    }
  }

  for (const input of template.content.querySelectorAll('input')) {
    if (input.getAttribute('type') !== 'checkbox') {
      input.remove();
      continue;
    }
    input.setAttribute('disabled', '');
    input.removeAttribute('form');
    input.removeAttribute('name');
  }

  return template.innerHTML;
}

function isSafeLink(href: string): boolean {
  const value = href.trim();
  if (!value) return false;
  if (value.startsWith('#') || value.startsWith('?')) return true;

  try {
    const url = new URL(value, browserBaseUrl());
    return SAFE_LINK_PROTOCOLS.has(url.protocol);
  } catch {
    return false;
  }
}

function isExternalLink(href: string): boolean {
  try {
    const base = browserBaseUrl();
    const url = new URL(href, base);
    return (
      (url.protocol === 'http:' || url.protocol === 'https:') && url.origin !== new URL(base).origin
    );
  } catch {
    return false;
  }
}

function browserBaseUrl(): string {
  const origin = window.location.origin;
  return origin && origin !== 'null' ? `${origin}/` : 'https://locus.local/';
}
