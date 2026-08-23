import DOMPurify from 'dompurify';
import { marked } from 'marked';

const SAFE_LINK_PROTOCOLS = new Set(['http:', 'https:', 'mailto:']);

export function renderMarkdown(source: string): string {
  const rendered = marked.parse(source, {
    async: false,
    breaks: false,
    gfm: true,
  });
  const clean = DOMPurify.sanitize(rendered, {
    ALLOWED_ATTR: ['checked', 'disabled', 'href', 'start', 'title', 'type'],
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
