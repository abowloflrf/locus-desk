import DOMPurify from 'dompurify';

const ALLOWED_TAGS = [
  'a',
  'blockquote',
  'br',
  'code',
  'del',
  'em',
  'figcaption',
  'figure',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'hr',
  'img',
  'li',
  'ol',
  'p',
  'pre',
  'strong',
  'sub',
  'sup',
  'table',
  'tbody',
  'td',
  'tfoot',
  'th',
  'thead',
  'time',
  'tr',
  'ul',
] as const;

const ALLOWED_ATTRIBUTES = [
  'alt',
  'colspan',
  'datetime',
  'height',
  'href',
  'rowspan',
  'src',
  'start',
  'title',
  'width',
] as const;

export function sanitizeLibraryHtml(source: string, sourceUrl: string): string {
  const clean = DOMPurify.sanitize(source, {
    ALLOWED_ATTR: [...ALLOWED_ATTRIBUTES],
    ALLOWED_TAGS: [...ALLOWED_TAGS],
    ALLOW_ARIA_ATTR: false,
    ALLOW_DATA_ATTR: false,
    FORBID_ATTR: ['class', 'id', 'name', 'srcset', 'style'],
    FORBID_TAGS: ['form', 'iframe', 'math', 'script', 'style', 'svg', 'template'],
  });

  const template = document.createElement('template');
  template.innerHTML = clean;

  for (const anchor of template.content.querySelectorAll('a')) {
    const href = normalizeHttpUrl(anchor.getAttribute('href'), sourceUrl);
    if (!href) {
      anchor.removeAttribute('href');
      anchor.removeAttribute('rel');
      anchor.removeAttribute('target');
      continue;
    }

    anchor.setAttribute('href', href);
    anchor.setAttribute('rel', 'noopener noreferrer');
    anchor.setAttribute('target', '_blank');
  }

  for (const image of template.content.querySelectorAll('img')) {
    const src = normalizeHttpUrl(image.getAttribute('src'), sourceUrl);
    if (!src) {
      image.remove();
      continue;
    }

    image.setAttribute('src', src);
    image.setAttribute('loading', 'lazy');
    image.setAttribute('decoding', 'async');
    image.setAttribute('referrerpolicy', 'no-referrer');
  }

  return template.innerHTML.trim();
}

export function safeLibrarySourceUrl(value: string): string | null {
  return normalizeHttpUrl(value, null);
}

function normalizeHttpUrl(value: string | null, sourceUrl: string | null): string | null {
  const candidate = value?.trim();
  if (!candidate) return null;

  try {
    const base = sourceUrl ? new URL(sourceUrl) : undefined;
    if (base && base.protocol !== 'http:' && base.protocol !== 'https:') return null;
    if (base && (base.username || base.password)) return null;
    const url = new URL(candidate, base);
    if (url.username || url.password) return null;
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.toString() : null;
  } catch {
    return null;
  }
}
