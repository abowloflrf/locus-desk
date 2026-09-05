import DOMPurify from 'dompurify';
import { highlightCode } from './code-highlight';

const ALLOWED_TAGS = [
  'a',
  'abbr',
  'address',
  'article',
  'aside',
  'blockquote',
  'br',
  'caption',
  'cite',
  'code',
  'col',
  'colgroup',
  'dd',
  'del',
  'details',
  'div',
  'dl',
  'dt',
  'em',
  'figcaption',
  'figure',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'header',
  'hr',
  'img',
  'ins',
  'kbd',
  'li',
  'main',
  'mark',
  'ol',
  'p',
  'pre',
  'q',
  's',
  'samp',
  'section',
  'small',
  'strong',
  'sub',
  'summary',
  'sup',
  'table',
  'tbody',
  'td',
  'tfoot',
  'th',
  'thead',
  'time',
  'tr',
  'u',
  'ul',
  'var',
  'wbr',
] as const;

const ALLOWED_ATTRIBUTES = [
  'alt',
  'cite',
  'colspan',
  'datetime',
  'data-language',
  'dir',
  'height',
  'href',
  'lang',
  'open',
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

  for (const heading of template.content.querySelectorAll('h1, h2, h3, h4, h5, h6')) {
    const anchor = heading.parentElement;
    if (
      heading.textContent?.trim() !== '#' ||
      heading.closest('pre, code') ||
      anchor?.tagName !== 'A'
    )
      continue;
    const href = normalizeHttpUrl(anchor.getAttribute('href'), sourceUrl);
    if (!href) continue;
    const target = new URL(href);
    if (!target.hash) continue;
    target.hash = '';
    const source = new URL(sourceUrl);
    source.hash = '';
    if (target.href !== source.href) continue;
    const hasTitle = [...anchor.children].some(
      (sibling) =>
        sibling !== heading &&
        sibling.tagName === heading.tagName &&
        sibling.textContent?.trim() &&
        sibling.textContent.trim() !== '#',
    );
    if (hasTitle) heading.remove();
  }

  for (const element of template.content.querySelectorAll('[data-language]')) {
    const language = element.getAttribute('data-language')?.trim().toLowerCase() ?? '';
    if (element.matches('pre, code') && /^[a-z0-9][a-z0-9_+.#-]{0,31}$/.test(language)) {
      element.setAttribute('data-language', language);
    } else {
      element.removeAttribute('data-language');
    }
  }

  for (const pre of template.content.querySelectorAll('pre')) {
    const code = pre.querySelector('code') ?? pre;
    const language = code.getAttribute('data-language');
    if (language) pre.setAttribute('data-language', language);
    code.textContent = codeBlockText(code);
  }

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

function codeBlockText(code: Element): string {
  const fragment = code.cloneNode(true) as Element;
  // textContent omits visual line breaks used by Prism/Docusaurus code blocks.
  for (const br of fragment.querySelectorAll('br')) {
    br.replaceWith(document.createTextNode('\n'));
  }
  const lines = [...fragment.querySelectorAll('div')].map((line) => ({
    line,
    followedByNewline:
      line.nextSibling?.nodeType === Node.TEXT_NODE &&
      line.nextSibling.textContent?.startsWith('\n'),
  }));
  for (const { line, followedByNewline } of lines.reverse()) {
    let text = line.textContent ?? '';
    if (!text.endsWith('\n') && !followedByNewline) text += '\n';
    const previous = line.previousSibling;
    const previousIsLine = previous instanceof Element && previous.tagName === 'DIV';
    if (!previousIsLine && previous?.textContent && !previous.textContent.endsWith('\n')) {
      text = `\n${text}`;
    }
    line.replaceWith(document.createTextNode(text));
  }
  return fragment.textContent ?? '';
}

export async function highlightLibraryHtml(source: string, sourceUrl: string): Promise<string> {
  const template = document.createElement('template');
  template.innerHTML = sanitizeLibraryHtml(source, sourceUrl);
  // Work on a detached fragment so late highlights cannot change a different article.
  let remaining = 100_000;
  for (const pre of template.content.querySelectorAll('pre')) {
    const code = pre.querySelector('code') ?? pre;
    const language = code.getAttribute('data-language') ?? pre.getAttribute('data-language');
    if (!language) continue;
    const text = code.textContent ?? '';
    if (text.length > 20_000 || text.length > remaining) continue;
    remaining -= text.length;
    const html = await highlightCode(text, language);
    if (html !== null) code.innerHTML = html;
  }
  return template.innerHTML;
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
