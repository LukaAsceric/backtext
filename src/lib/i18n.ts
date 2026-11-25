import type { Language } from '../types';

export const translations = {
  de: {
    navWorkspace: 'Arbeitsbereich',
    navSystem: 'Intelligenz & System',
    today: 'Heute & Fokus',
    meetings: 'Meetings',
    context: 'Kontext-Gedächtnis',
    people: 'Personen & Stimmen',
    recipes: 'Recipes Studio',
    mcpPrivacy: 'MCP & Datenschutz',
    personalization: 'Personalisierung',
    settings: 'Einstellungen',
    statusActive: 'Kontext aktiv',
    statusRecording: 'Aufnahme läuft',
    statusPaused: 'Pausiert',
    resume: 'Fortsetzen',
    hoverBtn: 'Hover-Assistent',
    themeLight: 'Hell',
    themeDark: 'Dunkel',
    todayTitle: 'Ruhiger Kontext im Hintergrund.',
    todaySubtitle:
      'backtext verknüpft deine Meetings, geteilten Folien und aktiven Arbeitsfenster lokal für dich und deine KI-Agenten.',
    newAdhocMeeting: 'Neues Meeting aufnehmen',
    searchPlaceholder: 'Gedächtnis durchsuchen (Meetings, Folien-OCR, Fenster, Zwischenablage)…',
    upcomingMeetings: 'Anstehende & aktive Meetings',
    recentEpisodes: 'Heutige Arbeits-Episoden',
    startBotFree: 'Aufnahme starten',
    openWorkspace: 'Öffnen',
    notepadTab: 'Notizen',
    enhancedTab: 'KI-Notizen',
    transcriptTab: 'Transkript',
    slidesTab: 'Folien',
    yourRoughNotes: 'Meine Notizen',
    enhancedSummary: 'KI-Notizen',
    reEnhance: 'Mit KI verdichten',
    simulateStep: 'Demo-Segment',
    stopAndSummarize: 'Beenden',
    actionItems: 'Action Items',
    chatPlaceholder: 'Frage stellen oder / tippen für Recipes (z. B. /follow-up-email)…',
    meetingChatPlaceholder: 'Frage zu diesem Meeting oder / Recipe tippen…',
    send: 'Senden',
    purge15: 'Letzte 15 Min vergessen',
    purge60: 'Letzte 60 Min vergessen',
    languageLabel: 'Oberflächensprache',
    outputLangLabel: 'KI-Ausgabesprache',
    saveProfile: 'Profil speichern',
  },
  en: {
    navWorkspace: 'Workspace',
    navSystem: 'Intelligence & System',
    today: 'Today & Briefs',
    meetings: 'Meetings',
    context: 'Context Memory',
    people: 'People & Voiceprints',
    recipes: 'Recipes Studio',
    mcpPrivacy: 'MCP & Privacy',
    personalization: 'Personalization',
    settings: 'Settings',
    statusActive: 'Context Active',
    statusRecording: 'Recording',
    statusPaused: 'Paused',
    resume: 'Resume',
    hoverBtn: 'Hover Assistant',
    themeLight: 'Light',
    themeDark: 'Dark',
    todayTitle: 'Quiet background context for your agents.',
    todaySubtitle:
      'backtext unifies bot-free meeting notes, shared slide OCR, and active desktop focus locally on your Mac.',
    newAdhocMeeting: 'Record New Meeting',
    searchPlaceholder: 'Search memory (meetings, slide OCR, active windows, clipboard)…',
    upcomingMeetings: 'Upcoming & Active Meetings',
    recentEpisodes: "Today's Work Episodes",
    startBotFree: 'Start Recording',
    openWorkspace: 'Open',
    notepadTab: 'Notes',
    enhancedTab: 'AI Notes',
    transcriptTab: 'Transcript',
    slidesTab: 'Slides',
    yourRoughNotes: 'My Notes',
    enhancedSummary: 'AI Notes',
    reEnhance: 'Enhance with AI',
    simulateStep: 'Demo Segment',
    stopAndSummarize: 'Finish',
    actionItems: 'Action Items',
    chatPlaceholder: 'Ask anything or type / for Recipes (e.g. /follow-up-email)…',
    meetingChatPlaceholder: 'Ask about this meeting or type / for a Recipe…',
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
