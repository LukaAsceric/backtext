import React from 'react';

function renderInline(line: string): React.ReactNode[] {
  const parts = line.split(/(\*\*[^*]+\*\*|`[^`]+`)/g);
  return parts.map((part, idx) => {
    if (part.startsWith('**') && part.endsWith('**')) {
      return <strong key={idx}>{part.slice(2, -2)}</strong>;
    }
    if (part.startsWith('`') && part.endsWith('`')) {
      return (
        <code
          key={idx}
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: '0.88em',
            background: 'var(--bg-surface-hover)',
            padding: '1px 5px',
            borderRadius: 4,
          }}
        >
          {part.slice(1, -1)}
        </code>
      );
    }
    return <React.Fragment key={idx}>{part}</React.Fragment>;
  });
}

export const FormattedMarkdown: React.FC<{ text: string }> = ({ text }) => {
  const lines = text.split('\n');
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 5, lineHeight: 1.62 }}>
      {lines.map((raw, i) => {
        const line = raw.trim();
        if (!line) return <div key={i} style={{ height: 3 }} />;
        if (line.startsWith('### ')) {
          return (
            <div
              key={i}
              style={{
                fontWeight: 700,
                fontSize: 13.5,
                color: 'var(--text-primary)',
                marginTop: i === 0 ? 0 : 6,
              }}
            >
              {renderInline(line.slice(4))}
            </div>
          );
        }
        if (line.startsWith('## ') || line.startsWith('# ')) {
          const clean = line.replace(/^#+\s*/, '');
          return (
            <div key={i} style={{ fontWeight: 700, fontSize: 14.5, marginTop: i === 0 ? 0 : 6 }}>
              {renderInline(clean)}
            </div>
          );
        }
        if (line.startsWith('- ') || line.startsWith('* ')) {
          return (
            <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'flex-start' }}>
              <span style={{ color: 'var(--accent-sage)', fontWeight: 700 }}>•</span>
              <span>{renderInline(line.slice(2))}</span>
            </div>
          );
        }
        if (line.startsWith('> ')) {
          return (
            <blockquote
              key={i}
              style={{
                margin: '4px 0',
                paddingLeft: 10,
                borderLeft: '2px solid var(--accent-sage)',
                color: 'var(--text-secondary)',
              }}
            >
              {renderInline(line.slice(2))}
            </blockquote>
          );
        }
        return <div key={i}>{renderInline(line)}</div>;
      })}
    </div>
  );
};
