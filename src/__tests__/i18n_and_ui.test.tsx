import { describe, expect, it } from 'vitest';
import { formatDurationMs, sourceBadgeLabel, t } from '../lib/i18n';

describe('backtext i18n and formatting utilities', () => {
  it('supports German default and switchable English labels', () => {
    expect(t('de', 'statusActive')).toBe('Kontext aktiv');
    expect(t('en', 'statusActive')).toBe('Context Active');
    expect(t('de', 'todayTitle')).toContain('Hintergrund');
    expect(t('en', 'todayTitle')).toContain('background');
  });

  it('formats transcript and slide timestamps cleanly', () => {
    expect(formatDurationMs(34_000)).toBe('00:34');
    expect(formatDurationMs(108_000)).toBe('01:48');
  });

  it('renders 4-stage speaker attribution badges in DE and EN', () => {
    expect(sourceBadgeLabel('mic_self', 'de')).toBe('Eigenes Mikrofon');
    expect(sourceBadgeLabel('voiceprint', 'de')).toContain('Voiceprint');
    expect(sourceBadgeLabel('visual_active_speaker', 'en')).toContain('Active Speaker OCR');
  });

  it('exposes Settings navigation and clean Granola meeting labels', () => {
    expect(t('de', 'settings')).toBe('Einstellungen');
    expect(t('en', 'settings')).toBe('Settings');
    expect(t('de', 'notepadTab')).toBe('Notizen');
    expect(t('de', 'enhancedTab')).toBe('KI-Notizen');
  });
});
