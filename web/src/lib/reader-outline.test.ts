import { describe, expect, it } from 'vitest';
import { createReaderOutline } from './reader-outline';
import { sanitizeLibraryHtml } from './library-content';

describe('createReaderOutline', () => {
  it.each([
    '<a href="#map"><h2 id="map">Normal map</h2><h2 class="hash" data-pagefind-ignore arialabel="Anchor">#</h2></a><a href="#lookup"><h3 id="lookup">Lookup</h3><h3 class="hash">#</h3></a>',
    '<a href="https://source.example/article#map" target="_blank"><h2>Normal map</h2><h2>#</h2></a><a href="https://source.example/article#lookup"><h3>Lookup</h3><h3>#</h3></a>',
  ])('excludes decorative heading markers from new and previously saved captures', (source) => {
    const html = sanitizeLibraryHtml(source, 'https://source.example/article');
    const outline = createReaderOutline(html, 'reader');
    expect(outline.headings.map((heading) => heading.text)).toEqual(['Normal map', 'Lookup']);
    expect(outline.html).not.toContain('>#<');
    expect(sanitizeLibraryHtml(html, 'https://source.example/article')).toBe(html);
  });

  it('uses readable section labels and unique, stable anchors after sanitization', () => {
    const html = sanitizeLibraryHtml(
      '<h1>Title</h1><h2 id="unsafe">重复 <em>章节</em></h2><h3> Detail\n label </h3><h2>重复 章节</h2><h2> </h2><h4>Footnote</h4><script>alert(1)</script>',
      'https://example.com',
    );
    const outline = createReaderOutline(html, 'reader-1');
    expect(outline.headings).toEqual([
      { id: 'reader-1-section-1', depth: 2, text: '重复 章节' },
      { id: 'reader-1-section-2', depth: 3, text: 'Detail label' },
      { id: 'reader-1-section-3', depth: 2, text: '重复 章节' },
    ]);
    expect(outline.html).toContain('tabindex="-1"');
    expect(outline.html).not.toContain('unsafe');
    expect(outline.html).not.toContain('<script');
    expect(createReaderOutline(html, 'reader-1')).toEqual(outline);
    expect(createReaderOutline(html, 'reader-2').headings[0]?.id).not.toBe(outline.headings[0]?.id);
  });
});
