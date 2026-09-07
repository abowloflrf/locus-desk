import DOMPurify from 'dompurify';
import { Marked } from 'marked';

function literal({ raw }: { raw: string }): string {
  return raw.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function literalBlock(token: { raw: string }): string {
  return `<p>${literal(token)}</p>`;
}

const parser = new Marked({
  breaks: true,
  gfm: true,
  renderer: {
    heading: literalBlock,
    blockquote: literalBlock,
    code: literalBlock,
    table: literalBlock,
    hr: literalBlock,
    html: literal,
    link: literal,
    image: literal,
    em: literal,
    del: literal,
    checkbox: ({ checked }) => (checked ? '[x] ' : '[ ] '),
  },
});

export function renderSummaryMarkdown(source: string): string {
  return DOMPurify.sanitize(parser.parse(source, { async: false }), {
    ALLOWED_TAGS: ['p', 'br', 'ul', 'ol', 'li', 'strong', 'code'],
    ALLOWED_ATTR: ['start'],
    ALLOW_ARIA_ATTR: false,
    ALLOW_DATA_ATTR: false,
  });
}

// Reveal text from the complete parsed tree so incomplete Markdown never flashes on screen.
export function createSummaryReveal(source: string) {
  const html = renderSummaryMarkdown(source);
  const template = document.createElement('template');
  template.innerHTML = html;
  const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
  const segments = (text: string) => Array.from(segmenter.segment(text), (part) => part.segment);
  const textLength = segments(template.content.textContent ?? '').length;

  return {
    html,
    textLength,
    frame(progress: number): string {
      if (progress >= 1) return html;
      if (progress <= 0) return '';
      const copy = template.cloneNode(true) as HTMLTemplateElement;
      let remaining = Math.ceil(textLength * progress);
      function trim(parent: Node) {
        for (const node of [...parent.childNodes]) {
          if (remaining <= 0) {
            node.remove();
          } else if (node.nodeType === Node.TEXT_NODE) {
            const characters = segments(node.textContent ?? '');
            node.textContent = characters.slice(0, remaining).join('');
            remaining -= characters.length;
          } else {
            trim(node);
          }
        }
      }
      trim(copy.content);
      return copy.innerHTML;
    },
  };
}
