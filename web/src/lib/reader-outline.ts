export type ReaderHeading = {
  id: string;
  depth: number;
  text: string;
};

// Only pass sanitized article HTML or its trusted syntax-highlighted equivalent.
export function createReaderOutline(html: string, prefix: string) {
  const template = document.createElement('template');
  template.innerHTML = html;
  const headings: ReaderHeading[] = [];

  for (const heading of template.content.querySelectorAll('h2, h3')) {
    const text = heading.textContent?.replace(/\s+/g, ' ').trim();
    if (!text) continue;
    const id = `${prefix}-section-${headings.length + 1}`;
    heading.id = id;
    heading.setAttribute('tabindex', '-1');
    headings.push({ id, depth: Number(heading.tagName.slice(1)), text });
  }

  return { html: template.innerHTML, headings };
}
