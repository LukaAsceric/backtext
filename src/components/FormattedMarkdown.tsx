import React, { useState } from 'react';
import { CheckSquare, Square } from 'lucide-react';
import type { TranscriptSegment } from '../types';
import { formatDurationMs } from '../lib/i18n';

interface FormattedMarkdownProps {
  text: string;
  transcript?: TranscriptSegment[];
  onJumpToTranscript?: (segmentId: string) => void;
}

function unescapeMarkdown(s: string): string {
  return s
    .replace(/\\\*/g, '*')
    .replace(/\\_/g, '_')
    .replace(/\\\[/g, '[')
    .replace(/\\\]/g, ']')
    .replace(/\\`/g, '`');
}

function parseTimestampMs(raw: string): number | null {
  const m = raw.match(/^\[(?:(\d+):)?(\d{1,2}):(\d{2})\]$/);
  if (!m) return null;
  const hours = m[1] ? Number(m[1]) : 0;
  const mins = Number(m[2]);
  const secs = Number(m[3]);
  if (Number.isNaN(hours) || Number.isNaN(mins) || Number.isNaN(secs) || secs >= 60) {
    return null;
  }
  return (hours * 3600 + mins * 60 + secs) * 1000;
}

function findMatchingSegment(
  ms: number,
  transcript?: TranscriptSegment[],
): TranscriptSegment | null {
  if (!transcript || transcript.length === 0) return null;
  // 1. Exact floored-second start_ms match (e.g. 29_200ms displays as 00:29 -> ms === 29_000)
  const exactStart = transcript.find(
    (seg) => Math.floor(seg.start_ms / 1000) * 1000 === ms,
  );
  if (exactStart) return exactStart;
  // 2. Strict inside-interval hit
  const inside = transcript.find((seg) => ms >= seg.start_ms && ms <= seg.end_ms);
  if (inside) return inside;
  // 3. Closest start_ms
  let best = transcript[0];
  let bestDist = Math.abs(transcript[0].start_ms - ms);
  for (const seg of transcript) {
    const d = Math.abs(seg.start_ms - ms);
    if (d < bestDist) {
      best = seg;
      bestDist = d;
    }
  }
  return best;
}

/**
 * Granola-style clickable inline citation badge with viewport-clamped `position: fixed`
 * hover tooltip so it NEVER triggers horizontal scrollbars or clips at window edges.
 */
const CitationPill: React.FC<{
  label: string;
  segment: TranscriptSegment;
  onJump: (segmentId: string) => void;
}> = ({ label, segment, onJump }) => {
  const [tipPos, setTipPos] = useState<{ left: number; top: number } | null>(null);
  const quotePreview =
    segment.text.length > 150 ? `${segment.text.slice(0, 147)}…` : segment.text;

  const showTooltip = (el: HTMLElement) => {
    const rect = el.getBoundingClientRect();
    const tipWidth = Math.min(320, Math.max(220, window.innerWidth - 28));
    const centeredLeft = rect.left + rect.width / 2 - tipWidth / 2;
    const clampedLeft = Math.max(12, Math.min(centeredLeft, window.innerWidth - tipWidth - 12));
    const openAbove = rect.bottom + 110 > window.innerHeight;
    const top = openAbove ? Math.max(12, rect.top - 86) : rect.bottom + 8;
    setTipPos({ left: clampedLeft, top });
  };

  return (
    <span className="transcript-cite-wrap">
      <button
        type="button"
        className="transcript-cite-pill"
        data-cite-ts={label}
        data-segment-id={segment.id}
        onMouseEnter={(e) => showTooltip(e.currentTarget)}
        onFocus={(e) => showTooltip(e.currentTarget)}
        onMouseLeave={() => setTipPos(null)}
        onBlur={() => setTipPos(null)}
        onClick={(e) => {
          e.stopPropagation();
          setTipPos(null);
          onJump(segment.id);
        }}
        title={`${segment.speaker_name} (${formatDurationMs(segment.start_ms)}): "${quotePreview}"`}
      >
        {label}
      </button>
      {tipPos && (
        <span
          className="transcript-cite-tooltip visible"
          role="tooltip"
          style={{ left: tipPos.left, top: tipPos.top }}
        >
          <span className="transcript-cite-tooltip-meta">
            <strong>{segment.speaker_name}</strong>
            <span className="mono-ts">{formatDurationMs(segment.start_ms)}</span>
          </span>
          <span className="transcript-cite-tooltip-quote">„{quotePreview}“</span>
        </span>
      )}
    </span>
  );
};

function renderInline(
  rawLine: string,
  transcript?: TranscriptSegment[],
  onJumpToTranscript?: (segmentId: string) => void,
): React.ReactNode[] {
  const line = unescapeMarkdown(rawLine);
  // Matches: **bold**, `code`, [MM:SS] / [H:MM:SS] citations, or *italic*
  const tokenRegex = /(\*\*[^*]+\*\*|`[^`]+`|\[(?:\d+:)?\d{1,2}:\d{2}\]|\*[^*\s][^*]*\*)/g;
  const parts = line.split(tokenRegex);

  return parts.map((part, idx) => {
    if (!part) return null;

    if (part.startsWith('**') && part.endsWith('**') && part.length > 4) {
      return (
        <strong key={idx}>
          {renderInline(part.slice(2, -2), transcript, onJumpToTranscript)}
        </strong>
      );
    }

    if (part.startsWith('`') && part.endsWith('`') && part.length > 2) {
      return (
        <code
          key={idx}
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: '0.86em',
            background: 'var(--bg-surface-hover)',
            padding: '1px 5px',
            borderRadius: 4,
            wordBreak: 'break-word',
          }}
        >
          {part.slice(1, -1)}
        </code>
      );
    }

    if (part.startsWith('[') && part.endsWith(']')) {
      const tsMs = parseTimestampMs(part);
      if (tsMs !== null) {
        const label = part.slice(1, -1);
        const seg = findMatchingSegment(tsMs, transcript);
        if (seg && onJumpToTranscript) {
          return (
            <CitationPill
              key={idx}
              label={label}
              segment={seg}
              onJump={onJumpToTranscript}
            />
          );
        }
        return (
          <span key={idx} className="transcript-cite-pill static">
            {label}
          </span>
        );
      }
    }

    if (part.startsWith('*') && part.endsWith('*') && part.length > 2) {
      return <em key={idx}>{part.slice(1, -1)}</em>;
    }

    return <React.Fragment key={idx}>{part}</React.Fragment>;
  });
}

export const FormattedMarkdown: React.FC<FormattedMarkdownProps> = ({
  text,
  transcript,
  onJumpToTranscript,
}) => {
  const lines = text.split('\n');
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 5,
        lineHeight: 1.64,
        minWidth: 0,
        maxWidth: '100%',
        overflowWrap: 'anywhere',
      }}
    >
      {lines.map((raw, i) => {
        const line = raw.trim();
        if (!line) return <div key={i} style={{ height: 3 }} />;

        // Embedded Markdown image (data:image/... or https://...)
        const imgMatch = line.match(/^!\[([^\]]*)\]\(((?:data:image\/|https?:\/\/)[^)]+)\)$/);
        if (imgMatch) {
          return (
            <figure key={i} className="note-img-figure" style={{ margin: '6px 0' }}>
              <img
                src={imgMatch[2]}
                alt={imgMatch[1] || 'Notizbild'}
                className="note-inline-img"
              />
            </figure>
          );
        }

        if (line === '---' || line === '***') {
          return (
            <hr
              key={i}
              style={{
                border: 'none',
                borderTop: '1px solid var(--border-subtle)',
                margin: '6px 0',
              }}
            />
          );
        }

        if (line.startsWith('### ')) {
          return (
            <div
              key={i}
              style={{
                fontWeight: 700,
                fontSize: 13.8,
                color: 'var(--text-primary)',
                marginTop: i === 0 ? 0 : 8,
              }}
            >
              {renderInline(line.slice(4), transcript, onJumpToTranscript)}
            </div>
          );
        }

        if (line.startsWith('## ') || line.startsWith('# ')) {
          const clean = line.replace(/^#+\s*/, '');
          return (
            <div
              key={i}
              style={{
                fontWeight: 700,
                fontSize: 15,
                color: 'var(--text-primary)',
                marginTop: i === 0 ? 0 : 8,
              }}
            >
              {renderInline(clean, transcript, onJumpToTranscript)}
            </div>
          );
        }

        // Markdown task checkbox lines: "- [ ] ..." or "- [x] ..."
        const taskMatch = line.match(/^(?:[-*•]|\d+\.)\s+\[([ xX✓])\]\s+(.+)$/);
        if (taskMatch) {
          const checked = taskMatch[1] !== ' ';
          const body = taskMatch[2];
          return (
            <div key={i} className={`md-task-line ${checked ? 'done' : ''}`}>
              <span className="md-task-icon" aria-hidden="true">
                {checked ? <CheckSquare size={14} /> : <Square size={14} />}
              </span>
              <span style={{ minWidth: 0, flex: 1 }}>
                {renderInline(body, transcript, onJumpToTranscript)}
              </span>
            </div>
          );
        }

        // Ordered lists: "1. ..."
        const orderedMatch = line.match(/^(\d+)\.\s+(.+)$/);
        if (orderedMatch) {
          return (
            <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'flex-start', minWidth: 0 }}>
              <span
                className="mono-ts"
                style={{ color: 'var(--accent-sage)', fontWeight: 700, minWidth: 18, flexShrink: 0 }}
              >
                {orderedMatch[1]}.
              </span>
              <span style={{ minWidth: 0, flex: 1 }}>
                {renderInline(orderedMatch[2], transcript, onJumpToTranscript)}
              </span>
            </div>
          );
        }

        if (line.startsWith('- ') || line.startsWith('* ') || line.startsWith('• ')) {
          return (
            <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'flex-start', minWidth: 0 }}>
              <span style={{ color: 'var(--accent-sage)', fontWeight: 700, lineHeight: 1.55, flexShrink: 0 }}>
                •
              </span>
              <span style={{ minWidth: 0, flex: 1 }}>
                {renderInline(line.slice(2), transcript, onJumpToTranscript)}
              </span>
            </div>
          );
        }

        if (line.startsWith('> ')) {
          return (
            <blockquote
              key={i}
              style={{
                margin: '4px 0',
                paddingLeft: 11,
                borderLeft: '2px solid var(--border-strong)',
                color: 'var(--text-secondary)',
                fontStyle: 'italic',
              }}
            >
              {renderInline(line.slice(2), transcript, onJumpToTranscript)}
            </blockquote>
          );
        }

        return (
          <div key={i} style={{ minWidth: 0 }}>
            {renderInline(line, transcript, onJumpToTranscript)}
          </div>
        );
      })}
    </div>
  );
};
