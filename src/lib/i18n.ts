import type { Language } from '../types';

export const translations = {
  de: {
    navWorkspace: 'Arbeitsbereich',
    navSystem: 'Intelligenz & Schutz',
    today: 'Heute & Fokus',
    meetings: 'Meetings & Notepad',
    context: 'Kontext-Gedächtnis',
    people: 'Personen & Stimmen',
    recipes: 'Recipes Studio',
    mcpPrivacy: 'MCP & Datenschutz',
    personalization: 'Personalisierung',
    statusActive: 'Kontext aktiv',
    statusRecording: 'Meeting läuft',
    statusPaused: 'Pausiert',
    resume: 'Fortsetzen',
    hoverBtn: 'Hover-Assistent',
    themeLight: 'Hell',
    themeDark: 'Dunkel',
    todayTitle: 'Ruhiger Kontext im Hintergrund.',
    todaySubtitle:
      'backtext verknüpft deine Meetings, geteilten Folien und aktiven Arbeitsfenster lokal für dich und deine KI-Agenten.',
    newAdhocMeeting: 'Ad-hoc Meeting starten',
    searchPlaceholder: 'Gedächtnis durchsuchen (Meetings, Folien-OCR, Fenster, Zwischenablage)…',
    upcomingMeetings: 'Anstehende & aktive Meetings',
    recentEpisodes: 'Heutige Arbeits-Episoden',
    startBotFree: 'Bot-frei aufzeichnen',
    openWorkspace: 'Im Notepad öffnen',
    notepadTab: 'Hybrid-Notepad & Summary',
    transcriptTab: 'Transkript & Teilnehmer',
    slidesTab: 'Screen-Share-Folien',
    yourRoughNotes: 'Meine Stichpunkte (während des Gesprächs)',
    enhancedSummary: 'KI-angereicherte Zusammenfassung',
    reEnhance: 'Mit KI neu verdichten',
    simulateStep: 'Live-Segment & Folie erfassen',
    stopAndSummarize: 'Beenden & Zusammenfassen',
    actionItems: 'Vereinbarte Action Items',
    chatPlaceholder: 'Frage stellen oder / tippen für Granola-Recipes (z. B. /follow-up-email)…',
    send: 'Senden',
    purge15: 'Letzte 15 Min vergessen',
    purge60: 'Letzte 60 Min vergessen',
    languageLabel: 'Oberflächensprache',
    outputLangLabel: 'KI-Ausgabesprache',
    saveProfile: 'Profil speichern',
  },
  en: {
    navWorkspace: 'Workspace',
    navSystem: 'Intelligence & Trust',
    today: 'Today & Briefs',
    meetings: 'Meetings & Notepad',
    context: 'Context Memory',
    people: 'People & Voiceprints',
    recipes: 'Recipes Studio',
    mcpPrivacy: 'MCP & Privacy',
    personalization: 'Personalization',
    statusActive: 'Context Active',
    statusRecording: 'Recording Call',
    statusPaused: 'Paused',
    resume: 'Resume',
    hoverBtn: 'Hover Assistant',
    themeLight: 'Light',
    themeDark: 'Dark',
    todayTitle: 'Quiet background context for your agents.',
    todaySubtitle:
      'backtext unifies bot-free meeting notes, shared slide OCR, and active desktop focus locally on your Mac.',
    newAdhocMeeting: 'Start Ad-hoc Meeting',
    searchPlaceholder: 'Search memory (meetings, slide OCR, active windows, clipboard)…',
    upcomingMeetings: 'Upcoming & Active Meetings',
    recentEpisodes: "Today's Work Episodes",
    startBotFree: 'Record Bot-Free',
    openWorkspace: 'Open Notepad',
    notepadTab: 'Hybrid Notepad & Summary',
    transcriptTab: 'Transcript & Speakers',
    slidesTab: 'Screen-Share Slides',
    yourRoughNotes: 'My Rough Notes (jot freely during call)',
    enhancedSummary: 'AI-Enhanced Notes',
    reEnhance: 'Re-enhance Notes',
    simulateStep: 'Simulate Live Speech & Slide',
    stopAndSummarize: 'Finish & Summarize',
    actionItems: 'Action Items',
    chatPlaceholder: 'Ask anything or type / for Recipes (e.g. /follow-up-email)…',
    send: 'Send',
    purge15: 'Forget Last 15m',
    purge60: 'Forget Last 60m',
    languageLabel: 'Interface Language',
    outputLangLabel: 'AI Output Language',
    saveProfile: 'Save Profile',
  },
} as const;

export function t(lang: Language, key: keyof (typeof translations)['de']): string {
  return translations[lang]?.[key] ?? translations.de[key];
}

export function formatDurationMs(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const mins = Math.floor(totalSec / 60);
  const secs = totalSec % 60;
  return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
}

export function sourceBadgeLabel(source: string, lang: Language): string {
  if (lang === 'en') {
    switch (source) {
      case 'mic_self':
        return 'Own Mic (Verified)';
      case 'voiceprint':
        return 'Voiceprint Match';
      case 'visual_active_speaker':
        return 'Active Speaker OCR';
      case 'calendar_prior':
        return 'Calendar Match';
      case 'manual':
        return 'Learned / Manual';
      default:
        return 'Voice Cluster';
    }
  }
  switch (source) {
    case 'mic_self':
      return 'Eigenes Mikrofon';
    case 'voiceprint':
      return 'Stimmprofil (Voiceprint)';
    case 'visual_active_speaker':
      return 'Visueller Sprecher (OCR)';
    case 'calendar_prior':
      return 'Kalender-Abgleich';
    case 'manual':
      return 'Gelernt / Bestätigt';
    default:
      return 'Stimm-Cluster';
  }
}
