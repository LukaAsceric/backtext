import React, { useEffect, useRef, useState } from 'react';
import { ImagePlus, X } from 'lucide-react';
import type { Language } from '../types';

interface RichNotepadProps {
  id?: string;
  meetingId: string;
  value: string;
  lang: Language;
  placeholder: string;
  onChange: (markdown: string) => void;
  onBlur?: (markdown: string) => void;
}

function escapeHtml(raw: string): string {
  return raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function inlineMarkdownToHtml(raw: string): string {
  let s = escapeHtml(raw);
  s = s.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/\*([^*\n]+)\*/g, '<em>$1</em>');
  s = s.replace(/`([^`\n]+)`/g, '<code>$1</code>');
  return s;
}

function imageFigureHtml(alt: string, src: string): string {
  const safeAlt = escapeHtml(alt || 'Notizbild');
  const safeSrc = escapeHtml(src);
  return `<figure class="note-img-figure" contenteditable="false" data-note-img="1" data-alt="${safeAlt}"><img src="${safeSrc}" alt="${safeAlt}" class="note-inline-img" /><button type="button" class="note-img-remove" data-remove-img="1" title="Bild entfernen">×</button></figure>`;
}

/**
 * Converts clean Markdown (`raw_user_notes`) into rich semantic HTML for the zero-toolbar
 * contentEditable notepad (bullets, numbered lists, interactive checkboxes, headings, quotes, images).
 */
export function markdownToNoteHtml(markdown: string): string {
  if (!markdown || !markdown.trim()) {
    return '<p><br></p>';
  }
  const lines = markdown.split('\n');
  const out: string[] = [];
  let listType: 'ul' | 'ol' | null = null;

  const closeList = () => {
    if (listType) {
      out.push(`</${listType}>`);
      listType = null;
    }
  };

  for (const rawLine of lines) {
    const trimmed = rawLine.trim();

    if (!trimmed) {
      closeList();
      out.push('<p><br></p>');
      continue;
    }

    // Standalone Markdown image: ![alt](data:image/... or https://...)
    const imgMatch = trimmed.match(/^!\[([^\]]*)\]\(((?:data:image\/|https?:\/\/)[^)]+)\)$/);
    if (imgMatch) {
      closeList();
      out.push(imageFigureHtml(imgMatch[1], imgMatch[2]));
      continue;
    }

    // Interactive checklist item: - [ ] ... or - [x] ...
    const taskMatch = trimmed.match(/^(?:[-*•])\s+\[([ xX✓])\]\s*(.*)$/);
    if (taskMatch) {
      closeList();
      const checked = taskMatch[1] !== ' ';
      const body = inlineMarkdownToHtml(taskMatch[2]);
      out.push(
        `<div class="note-check-row ${checked ? 'checked' : ''}" data-note-check="1"><input type="checkbox" contenteditable="false" ${checked ? 'checked' : ''} /><span class="note-check-text">${body || '<br>'}</span></div>`,
      );
      continue;
    }

    // Bullet item: - ..., * ..., • ...
    const bulletMatch = trimmed.match(/^(?:[-*•])\s+(.*)$/);
    if (bulletMatch) {
      if (listType !== 'ul') {
        closeList();
        out.push('<ul>');
        listType = 'ul';
      }
      out.push(`<li>${inlineMarkdownToHtml(bulletMatch[1]) || '<br>'}</li>`);
      continue;
    }

    // Numbered list item: 1. ...
    const numMatch = trimmed.match(/^\d+\.\s+(.*)$/);
    if (numMatch) {
      if (listType !== 'ol') {
        closeList();
        out.push('<ol>');
        listType = 'ol';
      }
      out.push(`<li>${inlineMarkdownToHtml(numMatch[1]) || '<br>'}</li>`);
      continue;
    }

    closeList();

    // Headings
    if (trimmed.startsWith('### ')) {
      out.push(`<h3>${inlineMarkdownToHtml(trimmed.slice(4))}</h3>`);
      continue;
    }
    if (trimmed.startsWith('## ')) {
      out.push(`<h2>${inlineMarkdownToHtml(trimmed.slice(3))}</h2>`);
      continue;
    }
    if (trimmed.startsWith('# ')) {
      out.push(`<h2>${inlineMarkdownToHtml(trimmed.slice(2))}</h2>`);
      continue;
    }

    // Blockquote
    if (trimmed.startsWith('> ')) {
      out.push(`<blockquote>${inlineMarkdownToHtml(trimmed.slice(2))}</blockquote>`);
      continue;
    }

    out.push(`<p>${inlineMarkdownToHtml(rawLine)}</p>`);
  }

  closeList();
  return out.join('') || '<p><br></p>';
}

function serializeInlineNodes(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return (node.textContent ?? '').replace(/\u00A0/g, ' ');
  }
  if (node.nodeType !== Node.ELEMENT_NODE) {
    return '';
  }
  const el = node as HTMLElement;
  const tag = el.tagName.toLowerCase();
  if (tag === 'br') return '';
  if (tag === 'img') {
    const src = el.getAttribute('src') ?? '';
    const alt = el.getAttribute('alt') ?? 'Notizbild';
    return src ? `![${alt}](${src})` : '';
  }
  if (tag === 'button' || tag === 'input') return '';
  const inner = Array.from(el.childNodes).map(serializeInlineNodes).join('');
  if (tag === 'strong' || tag === 'b') {
    return inner.trim() ? `**${inner}**` : inner;
  }
  if (tag === 'em' || tag === 'i') {
    return inner.trim() ? `*${inner}*` : inner;
  }
  if (tag === 'code') {
    return inner.trim() ? `\`${inner}\`` : inner;
  }
  return inner;
}

/**
 * Serializes the contentEditable DOM back to clean, standard Markdown (preserving bullets,
 * numbered lists, checkboxes, headings, quotes, inline formatting, and embedded data-URI images).
 */
export function noteDomToMarkdown(root: HTMLElement): string {
  const lines: string[] = [];

  const walkBlock = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const txt = (node.textContent ?? '').replace(/\u00A0/g, ' ').trimEnd();
      if (txt.trim()) lines.push(txt);
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const el = node as HTMLElement;
    const tag = el.tagName.toLowerCase();

    if (el.dataset.noteImg === '1' || tag === 'figure') {
      const img = el.querySelector('img');
      const src = img?.getAttribute('src') ?? '';
      const alt = el.dataset.alt || img?.getAttribute('alt') || 'Notizbild';
      if (src) {
        lines.push(`![${alt}](${src})`);
      }
      return;
    }

    if (el.dataset.noteCheck === '1' || el.classList.contains('note-check-row')) {
      const cb = el.querySelector('input[type="checkbox"]') as HTMLInputElement | null;
      const textEl = el.querySelector('.note-check-text') ?? el;
      const body = serializeInlineNodes(textEl).trim();
      lines.push(`- [${cb?.checked ? 'x' : ' '}] ${body}`);
      return;
    }

    if (tag === 'ul') {
      Array.from(el.children).forEach((child) => {
        if (child.tagName.toLowerCase() === 'li') {
          const text = serializeInlineNodes(child).trim();
          lines.push(`- ${text}`);
        } else {
          walkBlock(child);
        }
      });
      return;
    }

    if (tag === 'ol') {
      let idx = 1;
      Array.from(el.children).forEach((child) => {
        if (child.tagName.toLowerCase() === 'li') {
          const text = serializeInlineNodes(child).trim();
          lines.push(`${idx}. ${text}`);
          idx += 1;
        } else {
          walkBlock(child);
        }
      });
      return;
    }

    if (tag === 'h1' || tag === 'h2') {
      const text = serializeInlineNodes(el).trim();
      if (text) lines.push(`## ${text}`);
      return;
    }
    if (tag === 'h3') {
      const text = serializeInlineNodes(el).trim();
      if (text) lines.push(`### ${text}`);
      return;
    }
    if (tag === 'blockquote') {
      const text = serializeInlineNodes(el).trim();
      if (text) lines.push(`> ${text}`);
      return;
    }
    if (tag === 'img') {
      const src = el.getAttribute('src') ?? '';
      const alt = el.getAttribute('alt') ?? 'Notizbild';
      if (src) lines.push(`![${alt}](${src})`);
      return;
    }

    if (tag === 'p' || tag === 'div') {
      // Check if div wraps nested blocks (e.g. paste) vs inline text
      const hasBlockChild = Array.from(el.children).some((c) =>
        ['p', 'div', 'ul', 'ol', 'h1', 'h2', 'h3', 'blockquote', 'figure'].includes(
          c.tagName.toLowerCase(),
        ),
      );
      if (hasBlockChild) {
        Array.from(el.childNodes).forEach(walkBlock);
      } else {
        const text = serializeInlineNodes(el).trimEnd();
        lines.push(text);
      }
      return;
    }

    const fallback = serializeInlineNodes(el).trimEnd();
    if (fallback) lines.push(fallback);
  };

  Array.from(root.childNodes).forEach(walkBlock);

  // Trim trailing empty lines so empty notepad serializes to ''
  while (lines.length > 0 && lines[lines.length - 1].trim() === '') {
    lines.pop();
  }
  return lines.join('\n');
}

/**
 * Compresses pasted/dropped images cleanly via HTML5 Canvas (max width 1360px, JPEG 0.82)
 * so Retina screenshots stay crisp yet compact in SQLCipher.
 */
async function compressImageFileToDataUrl(file: File): Promise<string> {
  const rawUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ''));
    reader.onerror = () => reject(new Error('Bild konnte nicht gelesen werden'));
    reader.readAsDataURL(file);
  });

  if (typeof Image === 'undefined') return rawUrl;

  return await new Promise<string>((resolve) => {
    const img = new Image();
    img.onload = () => {
      const maxW = 1360;
      const scale = img.width > maxW ? maxW / img.width : 1;
      const targetW = Math.max(1, Math.round(img.width * scale));
      const targetH = Math.max(1, Math.round(img.height * scale));
      const canvas = document.createElement('canvas');
      canvas.width = targetW;
      canvas.height = targetH;
      const ctx = canvas.getContext('2d');
      if (!ctx) {
        resolve(rawUrl);
        return;
      }
      ctx.drawImage(img, 0, 0, targetW, targetH);
      try {
        const mime = file.type === 'image/png' && file.size < 180_000 ? 'image/png' : 'image/jpeg';
        const dataUrl = canvas.toDataURL(mime, 0.82);
        resolve(dataUrl.length < rawUrl.length ? dataUrl : rawUrl);
      } catch {
        resolve(rawUrl);
      }
    };
    img.onerror = () => resolve(rawUrl);
    img.src = rawUrl;
  });
}

function placeCaretAtStart(el: Node) {
  const sel = window.getSelection();
  if (!sel) return;
  const range = document.createRange();
  range.selectNodeContents(el);
  range.collapse(true);
  sel.removeAllRanges();
  sel.addRange(range);
}

function getClosestBlock(node: Node | null, root: HTMLElement): HTMLElement | null {
  let cur: Node | null = node;
  while (cur && cur !== root) {
    if (cur.nodeType === Node.ELEMENT_NODE) {
      const el = cur as HTMLElement;
      const tag = el.tagName.toLowerCase();
      if (
        ['p', 'div', 'li', 'h2', 'h3', 'blockquote'].includes(tag) &&
        !el.classList.contains('note-check-text')
      ) {
        return el;
      }
    }
    cur = cur.parentNode;
  }
  return null;
}

export const RichNotepad: React.FC<RichNotepadProps> = ({
  id = 'meeting-rough-notes-editor',
  meetingId,
  value,
  lang,
  placeholder,
  onChange,
  onBlur,
}) => {
  const editorRef = useRef<HTMLDivElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const lastMeetingIdRef = useRef<string | null>(null);
  const lastEmittedMdRef = useRef<string>(value);
  const [isEmpty, setIsEmpty] = useState<boolean>(() => !value.trim());
  const [dragOver, setDragOver] = useState(false);
  const [lightboxSrc, setLightboxSrc] = useState<string | null>(null);

  // Sync incoming Markdown into DOM only when switching meetings or updated externally
  useEffect(() => {
    const el = editorRef.current;
    if (!el) return;
    if (lastMeetingIdRef.current !== meetingId || value !== lastEmittedMdRef.current) {
      lastMeetingIdRef.current = meetingId;
      lastEmittedMdRef.current = value;
      el.innerHTML = markdownToNoteHtml(value);
      setIsEmpty(!value.trim());
    }
  }, [meetingId, value]);

  const emitCurrentMarkdown = () => {
    const el = editorRef.current;
    if (!el) return '';
    const md = noteDomToMarkdown(el);
    lastEmittedMdRef.current = md;
    setIsEmpty(!md.trim());
    onChange(md);
    return md;
  };

  const insertImageFiles = async (files: File[]) => {
    const el = editorRef.current;
    if (!el) return;
    const imageFiles = files.filter((f) => f.type.startsWith('image/'));
    if (imageFiles.length === 0) return;

    for (const file of imageFiles) {
      try {
        const dataUrl = await compressImageFileToDataUrl(file);
        const alt = file.name ? file.name.replace(/\.[^.]+$/, '') : 'Notizbild';
        const wrapper = document.createElement('div');
        wrapper.innerHTML = imageFigureHtml(alt, dataUrl);
        const figure = wrapper.firstElementChild;
        if (!figure) continue;

        const nextPara = document.createElement('p');
        nextPara.innerHTML = '<br>';

        const sel = window.getSelection();
        const targetBlock: HTMLElement | null =
          sel && sel.rangeCount > 0 && el.contains(sel.anchorNode)
            ? getClosestBlock(sel.anchorNode, el)
            : null;

        if (targetBlock && targetBlock.parentElement === el) {
          targetBlock.insertAdjacentElement('afterend', figure);
          figure.insertAdjacentElement('afterend', nextPara);
        } else {
          el.appendChild(figure);
          el.appendChild(nextPara);
        }
        placeCaretAtStart(nextPara);
      } catch {
        // ignore unreadable image file
      }
    }
    emitCurrentMarkdown();
  };

  /**
   * Automatic zero-toolbar markdown shortcuts triggered on Space:
   * `- ` / `* ` -> bullet list, `1. ` -> numbered list, `[] ` / `[ ] ` -> checkbox,
   * `# ` / `## ` / `### ` -> heading, `> ` -> quote.
   */
  const tryTransformBlockPrefix = (): boolean => {
    const el = editorRef.current;
    const sel = window.getSelection();
    if (!el || !sel || !sel.isCollapsed || sel.rangeCount === 0) return false;
    const block = getClosestBlock(sel.anchorNode, el);
    if (!block || block.tagName.toLowerCase() === 'li' || block.dataset.noteCheck === '1') {
      return false;
    }

    const rawText = (block.textContent ?? '').replace(/\u00A0/g, ' ');
    const trimmed = rawText.trim();

    const replaceBlockWith = (newEl: HTMLElement, caretTarget?: HTMLElement) => {
      block.replaceWith(newEl);
      placeCaretAtStart(caretTarget ?? newEl);
      emitCurrentMarkdown();
    };

    // Bullet list: `-` or `*` + Space
    if (trimmed === '-' || trimmed === '*') {
      const ul = document.createElement('ul');
      const li = document.createElement('li');
      li.innerHTML = '<br>';
      ul.appendChild(li);
      replaceBlockWith(ul, li);
      return true;
    }

    // Numbered list: `1.` + Space
    if (/^\d+\.$/.test(trimmed)) {
      const ol = document.createElement('ol');
      const li = document.createElement('li');
      li.innerHTML = '<br>';
      ol.appendChild(li);
      replaceBlockWith(ol, li);
      return true;
    }

    // Interactive checkbox: `[]` or `[ ]` + Space
    if (trimmed === '[]' || trimmed === '[ ]') {
      const row = document.createElement('div');
      row.className = 'note-check-row';
      row.dataset.noteCheck = '1';
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.contentEditable = 'false';
      const span = document.createElement('span');
      span.className = 'note-check-text';
      span.innerHTML = '<br>';
      row.appendChild(cb);
      row.appendChild(span);
      replaceBlockWith(row, span);
      return true;
    }

    // Headings: `#`, `##`, `###` + Space
    if (trimmed === '#' || trimmed === '##' || trimmed === '###') {
      const h = document.createElement(trimmed === '###' ? 'h3' : 'h2');
      h.innerHTML = '<br>';
      replaceBlockWith(h, h);
      return true;
    }

    // Blockquote: `>` + Space
    if (trimmed === '>') {
      const bq = document.createElement('blockquote');
      bq.innerHTML = '<br>';
      replaceBlockWith(bq, bq);
      return true;
    }

    return false;
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const el = editorRef.current;
    if (!el) return;

    // Auto-detect list / checkbox / heading shortcuts on Space
    if (e.key === ' ' && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (tryTransformBlockPrefix()) {
        e.preventDefault();
        return;
      }
    }

    // Enter inside interactive checklist row -> continue checklist or exit if empty
    if (e.key === 'Enter' && !e.shiftKey) {
      const sel = window.getSelection();
      if (sel && sel.rangeCount > 0) {
        const block = getClosestBlock(sel.anchorNode, el);
        if (block?.dataset.noteCheck === '1') {
          e.preventDefault();
          const textSpan = block.querySelector('.note-check-text');
          const txt = (textSpan?.textContent ?? '').trim();
          if (!txt) {
            const p = document.createElement('p');
            p.innerHTML = '<br>';
            block.replaceWith(p);
            placeCaretAtStart(p);
          } else {
            const nextRow = document.createElement('div');
            nextRow.className = 'note-check-row';
            nextRow.dataset.noteCheck = '1';
            const cb = document.createElement('input');
            cb.type = 'checkbox';
            cb.contentEditable = 'false';
            const span = document.createElement('span');
            span.className = 'note-check-text';
            span.innerHTML = '<br>';
            nextRow.appendChild(cb);
            nextRow.appendChild(span);
            block.insertAdjacentElement('afterend', nextRow);
            placeCaretAtStart(span);
          }
          emitCurrentMarkdown();
          return;
        }
      }
    }

    // Backspace on empty checklist row -> convert back to paragraph
    if (e.key === 'Backspace') {
      const sel = window.getSelection();
      if (sel && sel.isCollapsed && sel.rangeCount > 0) {
        const block = getClosestBlock(sel.anchorNode, el);
        if (block?.dataset.noteCheck === '1') {
          const textSpan = block.querySelector('.note-check-text');
          const txt = (textSpan?.textContent ?? '').trim();
          if (!txt) {
            e.preventDefault();
            const p = document.createElement('p');
            p.innerHTML = '<br>';
            block.replaceWith(p);
            placeCaretAtStart(p);
            emitCurrentMarkdown();
            return;
          }
        }
      }
    }

    // Clean inline formatting hotkeys (Cmd/Ctrl + B / I)
    if ((e.metaKey || e.ctrlKey) && !e.altKey) {
      const low = e.key.toLowerCase();
      if (low === 'b') {
        e.preventDefault();
        document.execCommand('bold');
        emitCurrentMarkdown();
      } else if (low === 'i') {
        e.preventDefault();
        document.execCommand('italic');
        emitCurrentMarkdown();
      }
    }
  };

  const handlePaste = async (e: React.ClipboardEvent<HTMLDivElement>) => {
    const items = Array.from(e.clipboardData?.items ?? []);
    const imgFiles: File[] = [];
    for (const item of items) {
      if (item.type.startsWith('image/')) {
        const f = item.getAsFile();
        if (f) imgFiles.push(f);
      }
    }
    if (imgFiles.length > 0) {
      e.preventDefault();
      await insertImageFiles(imgFiles);
      return;
    }

    // Clean plain-text paste (so pasting from websites/PDFs doesn't inject ugly foreign fonts/colors)
    const plain = e.clipboardData?.getData('text/plain');
    if (plain) {
      e.preventDefault();
      // If multi-line markdown list is pasted into an empty editor, parse cleanly
      const hasMarkdownBlocks = /^(\s*[-*•]\s+|\s*\d+\.\s+|#{1,3}\s+)/m.test(plain);
      if (hasMarkdownBlocks && isEmpty && editorRef.current) {
        editorRef.current.innerHTML = markdownToNoteHtml(plain);
        emitCurrentMarkdown();
      } else {
        document.execCommand('insertText', false, plain);
        emitCurrentMarkdown();
      }
    }
  };

  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;

    // Remove embedded image
    if (target.dataset.removeImg === '1') {
      e.preventDefault();
      e.stopPropagation();
      const fig = target.closest('figure');
      if (fig) {
        fig.remove();
        if (editorRef.current && editorRef.current.childNodes.length === 0) {
          editorRef.current.innerHTML = '<p><br></p>';
        }
        emitCurrentMarkdown();
      }
      return;
    }

    // Zoom image in calm lightbox
    if (target.tagName.toLowerCase() === 'img' && target.classList.contains('note-inline-img')) {
      const src = target.getAttribute('src');
      if (src) setLightboxSrc(src);
      return;
    }

    // Toggle interactive checklist checkbox
    if (target.tagName.toLowerCase() === 'input' && (target as HTMLInputElement).type === 'checkbox') {
      const row = target.closest('.note-check-row');
      if (row) {
        row.classList.toggle('checked', (target as HTMLInputElement).checked);
      }
      emitCurrentMarkdown();
    }
  };

  return (
    <div
      className={`rich-notepad-wrap ${dragOver ? 'drag-over' : ''}`}
      onDragOver={(e) => {
        if (Array.from(e.dataTransfer?.types ?? []).includes('Files')) {
          e.preventDefault();
          setDragOver(true);
        }
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={async (e) => {
        setDragOver(false);
        const files = Array.from(e.dataTransfer?.files ?? []);
        if (files.some((f) => f.type.startsWith('image/'))) {
          e.preventDefault();
          await insertImageFiles(files);
        }
      }}
    >
      {isEmpty && <div className="rich-notepad-placeholder">{placeholder}</div>}

      <div
        ref={editorRef}
        id={id}
        className="rich-notepad-surface"
        contentEditable
        suppressContentEditableWarning
        role="textbox"
        aria-multiline="true"
        onInput={() => emitCurrentMarkdown()}
        onKeyDown={handleKeyDown}
        onPaste={handlePaste}
        onClick={handleClick}
        onBlur={() => {
          const md = emitCurrentMarkdown();
          onBlur?.(md);
        }}
      />

      {/* Quiet unobtrusive hint footer — zero bloated text-editor toolbar */}
      <div className="rich-notepad-hint-bar">
        <span>
          {lang === 'de'
            ? 'Auto-Format: „- “ Liste · „1. “ Nummerierung · „[] “ Checkbox · „## “ Titel · ⌘V / Drag & Drop für Bilder'
            : 'Auto-format: "- " bullets · "1. " list · "[] " checklist · "## " heading · ⌘V / drop images'}
        </span>
        <button
          type="button"
          id="note-insert-image-btn"
          className="mini-btn"
          onClick={() => fileInputRef.current?.click()}
          style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
        >
          <ImagePlus size={11} />
          {lang === 'de' ? 'Bild' : 'Image'}
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          multiple
          style={{ display: 'none' }}
          onChange={async (e) => {
            const files = Array.from(e.target.files ?? []);
            e.target.value = '';
            await insertImageFiles(files);
          }}
        />
      </div>

      {/* Calm Full-Size Lightbox for Clicked Note Images */}
      {lightboxSrc && (
        <div
          className="note-lightbox-backdrop"
          onClick={() => setLightboxSrc(null)}
          role="dialog"
          aria-modal="true"
        >
          <button
            type="button"
            className="note-lightbox-close"
            onClick={() => setLightboxSrc(null)}
            title="Schließen"
          >
            <X size={18} />
          </button>
          <img
            src={lightboxSrc}
            alt="Notizbild Vorschau"
            className="note-lightbox-img"
            onClick={(e) => e.stopPropagation()}
          />
        </div>
      )}
    </div>
  );
};
