import { describe, expect, it } from 'vitest';
import { markdownToNoteHtml, noteDomToMarkdown } from '../components/RichNotepad';

describe('RichNotepad Markdown <-> DOM roundtrip', () => {
  it('roundtrips bullet lists, numbered lists, interactive checkboxes, headings, and inline images', () => {
    const sample = [
      '## Architektur-Review',
      '- Erster **wichtiger** Punkt',
      '- Zweiter Punkt mit `Code`',
      '1. Schritt eins',
      '2. Schritt zwei',
      '- [ ] Offene Aufgabe',
      '- [x] Erledigte Aufgabe',
      '![Whiteboard](data:image/jpeg;base64,/9j/4AAQSkZJRgABAQ==)',
    ].join('\n');

    const html = markdownToNoteHtml(sample);
    expect(html).toContain('<h2>Architektur-Review</h2>');
    expect(html).toContain('<ul><li>Erster <strong>wichtiger</strong> Punkt</li>');
    expect(html).toContain('<ol><li>Schritt eins</li><li>Schritt zwei</li></ol>');
    expect(html).toContain('data-note-check="1"');
    expect(html).toContain('data-note-img="1"');

    const host = document.createElement('div');
    host.innerHTML = html;
    const roundtripped = noteDomToMarkdown(host);

    expect(roundtripped).toContain('## Architektur-Review');
    expect(roundtripped).toContain('- Erster **wichtiger** Punkt');
    expect(roundtripped).toContain('- Zweiter Punkt mit `Code`');
    expect(roundtripped).toContain('1. Schritt eins');
    expect(roundtripped).toContain('2. Schritt zwei');
    expect(roundtripped).toContain('- [ ] Offene Aufgabe');
    expect(roundtripped).toContain('- [x] Erledigte Aufgabe');
    expect(roundtripped).toContain('![Whiteboard](data:image/jpeg;base64,/9j/4AAQSkZJRgABAQ==)');
  });
});
