use backtext_core::{
    embed_text_local_256, ActionItem, AudioChannel, ClipboardItem, ContextEpisode, ContextSnapshot,
    ExtractionMethod, IdentificationSource, LearnedPreference, Meeting, MeetingParticipant,
    MeetingStatus, Person, PrivacyRule, Recipe, RecipeScope, ScreenShareFrame, TranscriptSegment,
    VocabularyEntry,
};
use chrono::{Duration, Utc};

pub fn builtin_privacy_rules() -> Vec<PrivacyRule> {
    vec![
        PrivacyRule {
            id: "priv_1password".into(),
            kind: "app_bundle".into(),
            pattern: "com.1password.1password".into(),
            description: "1Password Passwort-Tresor komplett ausschließen".into(),
            is_enabled: true,
            is_builtin: true,
        },
        PrivacyRule {
            id: "priv_bitwarden".into(),
            kind: "app_bundle".into(),
            pattern: "com.bitwarden.desktop".into(),
            description: "Bitwarden Desktop-App ausschließen".into(),
            is_enabled: true,
            is_builtin: true,
        },
        PrivacyRule {
            id: "priv_keychain".into(),
            kind: "app_bundle".into(),
            pattern: "com.apple.keychainaccess".into(),
            description: "macOS Schlüsselbundverwaltung ausschließen".into(),
            is_enabled: true,
            is_builtin: true,
        },
        PrivacyRule {
            id: "priv_banking".into(),
            kind: "domain_pattern".into(),
            pattern: "*.banking.*, *sparkasse*, *ing.de*, *n26.com*, *stripe.com/settings*".into(),
            description: "Online-Banking & Zahlungs-Dashboards blockieren".into(),
            is_enabled: true,
            is_builtin: true,
        },
        PrivacyRule {
            id: "priv_private_window".into(),
            kind: "window_title".into(),
            pattern: "Inkognito|Private Browsing|Privates Fenster".into(),
            description: "Private & Inkognito-Browserfenster ignorieren".into(),
            is_enabled: true,
            is_builtin: true,
        },
        PrivacyRule {
            id: "priv_env_files".into(),
            kind: "window_title".into(),
            pattern: ".env.production|id_ed25519|secrets.yaml".into(),
            description: "Sensible Schlüssel- & Produktions-.env-Dateien überspringen".into(),
            is_enabled: true,
            is_builtin: true,
        },
    ]
}

pub fn builtin_recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            id: "rec_followup".into(),
            slug: "/follow-up-email".into(),
            name_de: "Follow-up E-Mail entwerfen".into(),
            name_en: "Draft Follow-up Email".into(),
            description_de: "Erstellt eine präzise Nachfass-Mail mit Entscheidungen, nächsten Schritten und Fälligkeiten.".into(),
            description_en: "Drafts a concise follow-up email with decisions, next steps, and deadlines.".into(),
            icon_key: "mail".into(),
            scope: RecipeScope::SingleMeeting,
            prompt_template: "Verfasse im Stil von {{user_name}} eine klare, freundliche Follow-up-E-Mail an {{participants}} basierend auf unseren Notizen ({{user_notes}}), dem Transkript ({{transcript}}) und den gezeigten Folien ({{screenshare_ocr}}). Gliedere in: 1) Wichtigste Ergebnisse, 2) Vereinbarte nächste Schritte mit Owner & Datum, 3) Offene Rückfragen.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: true,
            usage_count: 14,
        },
        Recipe {
            id: "rec_actions".into(),
            slug: "/action-items".into(),
            name_de: "Action Items & Tickets extrahieren".into(),
            name_en: "Extract Action Items & Linear Tickets".into(),
            description_de: "Zieht alle konkreten Zusagen, Owner, Deadlines und Ticket-Beschreibungen aus dem Gespräch.".into(),
            description_en: "Extracts all concrete commitments, owners, deadlines, and Linear-ready tickets.".into(),
            icon_key: "check".into(),
            scope: RecipeScope::SingleMeeting,
            prompt_template: "Analysiere {{transcript}} sowie {{screenshare_ocr}} und liste alle expliziten und impliziten Aufgaben auf. Gib pro Punkt an: Verantwortliche Person, exakte Aufgabe, genannte Frist und ein direkt nutzbares Ticket-Snippet.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: true,
            usage_count: 19,
        },
        Recipe {
            id: "rec_critical".into(),
            slug: "/critical-review".into(),
            name_de: "Blinde Flecken & Annahmen prüfen".into(),
            name_en: "Blind Spots & Unverified Claims".into(),
            description_de: "Prüft kritisch unbelegte Aussagen, unklare Zuständigkeiten und Architektur-/Projektrisiken.".into(),
            description_en: "Critically audits unverified claims, unowned decisions, and hidden project risks.".into(),
            icon_key: "shield".into(),
            scope: RecipeScope::SingleMeeting,
            prompt_template: "Gehe {{transcript}} wie ein erfahrener Sparringspartner durch: 1) Welche Annahmen wurden ohne Beleg getroffen? 2) Wo gab es verdeckten Widerspruch oder unklare Ownership? 3) Welche 3 kritischen Rückfragen sollte {{user_name}} sofort klären?".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: true,
            usage_count: 9,
        },
        Recipe {
            id: "rec_prd".into(),
            slug: "/prd-draft".into(),
            name_de: "Technisches Kurz-PRD / RFC".into(),
            name_en: "Technical Mini-PRD / RFC".into(),
            description_de: "Verwandelt Produkt- oder Architektur-Diskussionen inkl. Screen-Share-Notizen in eine Spezifikation.".into(),
            description_en: "Turns product or architecture discussions and shared slides into a structured spec.".into(),
            icon_key: "file-code".into(),
            scope: RecipeScope::SingleMeeting,
            prompt_template: "Erstelle aus {{user_notes}}, {{transcript}} und {{screenshare_ocr}} ein strukturiertes Mini-PRD: Problemstellung, Ziel-Nutzer, Funktionale Anforderungen, Nicht-Ziele, Architektur-Entscheidungen und offene technische Fragen.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: false,
            usage_count: 7,
        },
        Recipe {
            id: "rec_slides".into(),
            slug: "/slide-digest".into(),
            name_de: "Screen-Sharing & Folien-Analyse".into(),
            name_en: "Screen-Share & Slide Digest".into(),
            description_de: "Verknüpft alle erfassten Screen-Sharing-Screenshots und Zahlen mit den gesprochenen Kommentaren.".into(),
            description_en: "Correlates all captured screen-share slides and metrics with the spoken commentary.".into(),
            icon_key: "presentation".into(),
            scope: RecipeScope::SingleMeeting,
            prompt_template: "Fasse alle geteilten Bildschirminhalte ({{screenshare_ocr}}) chronologisch zusammen und ergänze jeweils, was im Gespräch ({{transcript}}) dazu diskutiert oder entschieden wurde.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: false,
            usage_count: 5,
        },
        Recipe {
            id: "rec_1on1".into(),
            slug: "/coaching-1on1".into(),
            name_de: "1:1 Reflexion & Zusagen".into(),
            name_en: "1:1 Reflection & Commitments".into(),
            description_de: "Strukturiert 1:1-Gespräche nach Stimmung, Blockern, Wachstumszielen und beidseitigen Zusagen.".into(),
            description_en: "Structures 1:1 syncs by morale, blockers, growth topics, and mutual commitments.".into(),
            icon_key: "users".into(),
            scope: RecipeScope::SingleMeeting,
            prompt_template: "Fasse dieses 1:1 ({{transcript}}) empathisch und präzise zusammen: Aktueller Fokus & Energielevel, genannte Blocker, Feedback-Punkte sowie Zusagen für bis zum nächsten 1:1.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: false,
            usage_count: 4,
        },
        Recipe {
            id: "rec_promises".into(),
            slug: "/weekly-commitments".into(),
            name_de: "Was habe ich diese Woche zugesagt?".into(),
            name_en: "What Did I Promise This Week?".into(),
            description_de: "Durchsucht alle jüngsten Meetings nach eigenen Versprechen und offenen Liefergegenständen.".into(),
            description_en: "Scans recent meetings for all commitments made by you and open deliverables.".into(),
            icon_key: "calendar".into(),
            scope: RecipeScope::MultiMeeting,
            prompt_template: "Prüfe alle jüngsten Meetings und erstelle eine priorisierte Liste aller Zusagen, die {{user_name}} gemacht hat, inklusive Gesprächspartner, Kontext-Meeting und Fälligkeit.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: true,
            usage_count: 12,
        },
        Recipe {
            id: "rec_person_prep".into(),
            slug: "/person-briefing".into(),
            name_de: "Vorbereitungs-Dossier für nächstes Gespräch".into(),
            name_en: "Pre-Meeting Person Briefing".into(),
            description_de: "Bündelt vergangene Meetings, offene Punkte und letzten Arbeitskontext zu Gesprächspartnern.".into(),
            description_en: "Synthesizes past calls, open items, and recent work context for attendees.".into(),
            icon_key: "user-check".into(),
            scope: RecipeScope::MultiMeeting,
            prompt_template: "Erstelle ein kompaktes 60-Sekunden-Vorbereitungsbriefing: Was wurde zuletzt vereinbart, was ist noch offen, woran habe ich im Hintergrund ({{active_context}}) dazu gearbeitet?".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: false,
            usage_count: 8,
        },
        Recipe {
            id: "rec_feedback".into(),
            slug: "/feedback-synthesis".into(),
            name_de: "Produkt- & Nutzerfeedback bündeln".into(),
            name_en: "Synthesize Product Feedback".into(),
            description_de: "Aggregiert wiederkehrende Wünsche, Reibungspunkte und Originalzitate über mehrere Calls hinweg.".into(),
            description_en: "Aggregates recurring feature requests, friction points, and verbatim quotes across calls.".into(),
            icon_key: "layers".into(),
            scope: RecipeScope::MultiMeeting,
            prompt_template: "Clustere alle genannten Nutzerprobleme und Feature-Wünsche über die ausgewählten Meetings nach Häufigkeit und Dringlichkeit, jeweils mit Originalzitat und Sprecher.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: false,
            usage_count: 3,
        },
        Recipe {
            id: "rec_morning".into(),
            slug: "/morning-brief".into(),
            name_de: "Morgen-Briefing aus Kontext & Meetings".into(),
            name_en: "Morning Brief from Context & Calls".into(),
            description_de: "Verbindet gestrige Arbeits-Episoden, offene Action Items und heutige Termine zu 3 Tages-Prioritäten.".into(),
            description_en: "Combines yesterday's work episodes, open action items, and today's calls into top 3 priorities.".into(),
            icon_key: "sun".into(),
            scope: RecipeScope::AmbientWorkspace,
            prompt_template: "Erstelle mein Morgen-Briefing basierend auf {{active_context}}, den letzten Meetings und {{user_profile}}: 1) Top-3-Fokusaufgaben heute, 2) Vorbereitung auf heutige Calls, 3) Offene Fäden aus gestern geöffneten Docs/PRs.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: true,
            usage_count: 21,
        },
        Recipe {
            id: "rec_handoff".into(),
            slug: "/agent-handoff".into(),
            name_de: "Kontext-Paket für Coding-/KI-Agent".into(),
            name_en: "Context Handoff Pack for AI Agent".into(),
            description_de: "Komprimiert aktuelle Fenster, Tabs, Clipboard & relevante Meeting-Entscheidungen für Claude/Cursor.".into(),
            description_en: "Packages active windows, tabs, clipboard, and meeting decisions into a crisp prompt for AI agents.".into(),
            icon_key: "terminal".into(),
            scope: RecipeScope::AmbientWorkspace,
            prompt_template: "Erzeuge ein sofort kopierbares, hochverdichtetes Kontext-Briefing für meinen Coding-Agenten basierend auf {{active_context}} und letzten Architektur-Entscheidungen: Ziel, relevante Dateien/URLs, Randbedingungen aus Meetings und konkreter Auftrag.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: true,
            usage_count: 16,
        },
        Recipe {
            id: "rec_eod".into(),
            slug: "/eod-wrapup".into(),
            name_de: "Tages-Rückblick & Arbeitslog".into(),
            name_en: "End-of-Day Wrap-Up & Work Log".into(),
            description_de: "Fasst alle heutigen Arbeits-Episoden, abgeschlossenen Themen und morgigen Anschlusspunkte zusammen.".into(),
            description_en: "Summarizes today's work episodes, decisions made, and tomorrow's pickup point.".into(),
            icon_key: "moon".into(),
            scope: RecipeScope::AmbientWorkspace,
            prompt_template: "Fasse meinen Arbeitstag aus den erfassten Hintergrund-Episoden ({{active_context}}) und heutigen Meetings ruhig und übersichtlich zusammen: Erreichtes, getroffene Entscheidungen und bester Wiedereinstiegspunkt für morgen früh.".into(),
            preferred_model: None,
            is_builtin: true,
            is_favorite: false,
            usage_count: 10,
        },
    ]
}

pub fn builtin_vocabulary() -> Vec<VocabularyEntry> {
    let now = Utc::now();
    vec![
        VocabularyEntry {
            id: "voc_backtext".into(),
            term: "backtext".into(),
            phonetic_hint: Some("bäck-text".into()),
            category: "Produkt".into(),
            project_tag: Some("backtext".into()),
            created_at: now,
        },
        VocabularyEntry {
            id: "voc_sck".into(),
            term: "ScreenCaptureKit".into(),
            phonetic_hint: Some("Screen Capture Kit".into()),
            category: "macOS API".into(),
            project_tag: Some("backtext".into()),
            created_at: now,
        },
        VocabularyEntry {
            id: "voc_mcp".into(),
            term: "Model Context Protocol (MCP)".into(),
            phonetic_hint: Some("M-C-P".into()),
            category: "KI-Protokoll".into(),
            project_tag: Some("AI Agent Workflows".into()),
            created_at: now,
        },
        VocabularyEntry {
            id: "voc_dhash".into(),
            term: "dHash Deduplizierung".into(),
            phonetic_hint: Some("D-Hash".into()),
            category: "Algorithmus".into(),
            project_tag: Some("backtext".into()),
            created_at: now,
        },
    ]
}

pub fn builtin_learned_preferences() -> Vec<LearnedPreference> {
    let now = Utc::now();
    vec![
        LearnedPreference {
            id: "lp_1".into(),
            category: "Zusammenfassungen".into(),
            fact_text: "Bevorzugt knappe Bullet-Points mit klarem Owner-Namen und konkretem Fälligkeitsdatum am Ende jedes Meetings.".into(),
            confidence: 0.94,
            source_ref: Some("Aus 8 bearbeiteten Meeting-Notizen gelernt".into()),
            is_active: true,
            created_at: now - Duration::days(3),
        },
        LearnedPreference {
            id: "lp_2".into(),
            category: "Tech-Kontext".into(),
            fact_text: "Arbeitet primär mit Rust, Tauri v2, TypeScript, Claude Desktop und Cursor via lokalem MCP-Server.".into(),
            confidence: 0.97,
            source_ref: Some("Hintergrund-Episoden & Glossar".into()),
            is_active: true,
            created_at: now - Duration::days(2),
        },
        LearnedPreference {
            id: "lp_3".into(),
            category: "Datenschutz".into(),
            fact_text: "Sensible Tokens und Passwort-Manager müssen strikt lokal gefiltert werden, bevor ein LLM- oder MCP-Aufruf erfolgt.".into(),
            confidence: 0.99,
            source_ref: Some("Explizite Privacy-Konfiguration".into()),
            is_active: true,
            created_at: now - Duration::days(1),
        },
    ]
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn make_slide_svg_data_url(badge: &str, title: &str, bullets: &[&str]) -> String {
    let mut bullet_svg = String::new();
    for (idx, line) in bullets.iter().enumerate() {
        let y = 138 + idx * 34;
        let safe_line = xml_escape(line);
        bullet_svg.push_str(&format!(
            r##"<circle cx="44" cy="{cy}" r="4" fill="#2E7D5B"/><text x="60" y="{y}" font-family="Inter, -apple-system, sans-serif" font-size="14" fill="#22252A">{safe_line}</text>"##,
            cy = y - 5,
            y = y,
            safe_line = safe_line
        ));
    }
    let safe_badge = xml_escape(badge);
    let safe_title = xml_escape(title);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360"><rect width="640" height="360" rx="14" fill="#F7F5F0"/><rect x="16" y="16" width="608" height="328" rx="10" fill="#FFFFFF" stroke="#E4E0D8"/><rect x="36" y="38" width="148" height="24" rx="6" fill="#EBF4EF"/><text x="48" y="54" font-family="JetBrains Mono, monospace" font-size="11" font-weight="600" fill="#2E7D5B">{safe_badge}</text><text x="36" y="96" font-family="Inter, -apple-system, sans-serif" font-size="21" font-weight="700" fill="#141619">{safe_title}</text><line x1="36" y1="112" x2="604" y2="112" stroke="#ECE8E0"/>{bullet_svg}</svg>"##,
        safe_badge = safe_badge,
        safe_title = safe_title,
        bullet_svg = bullet_svg
    );
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    format!("data:image/svg+xml;base64,{encoded}")
}

pub struct SeedDataset {
    pub persons: Vec<(Person, Vec<f32>)>,
    pub meetings: Vec<(Meeting, Vec<TranscriptSegment>, Vec<ScreenShareFrame>)>,
    pub episodes: Vec<ContextEpisode>,
    pub snapshots: Vec<ContextSnapshot>,
    pub clipboard: Vec<ClipboardItem>,
}

pub fn initial_demo_dataset() -> SeedDataset {
    let now = Utc::now();

    let anna_vp = embed_text_local_256("voiceprint::anna_weber_warm_alto_de_pitch_195hz");
    let markus_vp = embed_text_local_256("voiceprint::markus_klein_baritone_de_pitch_122hz");
    let clara_vp = embed_text_local_256("voiceprint::clara_novak_clear_mezzo_en_de_210hz");

    let persons = vec![
        (
            Person {
                id: "per_anna".into(),
                full_name: "Anna Weber".into(),
                email: Some("anna.weber@nordlicht.io".into()),
                company: Some("Nordlicht Systems".into()),
                role_notes: "Staff Product Designerin — verantwortet das Desktop-Onboarding, Menüleisten-Flows und Privacy-UX.".into(),
                voiceprint_samples: 14,
                voiceprint_confidence: 0.94,
                meeting_count: 6,
                open_commitments: vec![
                    "Finales Tray-Popover-Prototyping bis Donnerstag liefern".into(),
                    "Consent-Hinweis-Texte für EU/DE-Teams abstimmen".into(),
                ],
                last_seen_at: Some(now - Duration::hours(2)),
            },
            anna_vp,
        ),
        (
            Person {
                id: "per_markus".into(),
                full_name: "Markus Klein".into(),
                email: Some("markus@kernwerk.dev".into()),
                company: Some("Kernwerk Engineering".into()),
                role_notes: "Systems Engineer (macOS Audio & Rust) — berät zu ScreenCaptureKit, CoreAudio und Energieverbrauch.".into(),
                voiceprint_samples: 9,
                voiceprint_confidence: 0.91,
                meeting_count: 4,
                open_commitments: vec![
                    "Benchmark für Apple Vision OCR vs. AXUIElement auf M3 Air teilen".into(),
                ],
                last_seen_at: Some(now - Duration::hours(2)),
            },
            markus_vp,
        ),
        (
            Person {
                id: "per_clara".into(),
                full_name: "Clara Novak".into(),
                email: Some("clara.novak@velox-ai.com".into()),
                company: Some("Velox Labs".into()),
                role_notes: "AI Agent Lead — testet die MCP-Integration in Cursor und Claude Desktop im Alltag.".into(),
                voiceprint_samples: 5,
                voiceprint_confidence: 0.87,
                meeting_count: 3,
                open_commitments: vec![
                    "Feedback zur Token-Größe von get_live_context nach 3 Tagen Nutzung senden".into(),
                ],
                last_seen_at: Some(now - Duration::days(1)),
            },
            clara_vp,
        ),
    ];

    let m1_id = "mtg_arch_sync".to_string();
    let m1_start = now - Duration::minutes(135);
    let m1_end = now - Duration::minutes(95);

    let m1 = Meeting {
        id: m1_id.clone(),
        title: "Architektur-Sync: Kontexterfassung, ScreenCaptureKit & MCP-Latenz".into(),
        calendar_event_id: Some("cal_evt_9021".into()),
        detected_app: "Google Meet (Arc Browser)".into(),
        project_tag: Some("backtext".into()),
        started_at: m1_start,
        ended_at: Some(m1_end),
        status: MeetingStatus::Completed,
        template_id: "architecture".into(),
        prep_brief: Some("Letzter Stand: Markus testete Dual-Channel Audio ohne Bot; Anna stellte Entwürfe für den 1-Klick-Pause-Modus vor.".into()),
        raw_user_notes: "- Kein dauerhaftes Video aufnehmen -> AXUIElement zuerst, Vision OCR als Fallback\n- Screen-Share im Call über dHash deduplizieren (>= 6 Bits Differenz)\n- MCP get_live_context muss unter 50ms antworten\n- Anna schickt Tray-Icons bis Donnerstag".into(),
        executive_summary: "Entscheidung für eine ereignisbasierte Kontexterfassung auf macOS: Primär wird sichtbarer Text per AXUIElement nahezu ohne CPU-Last gelesen, ergänzt durch gezielte ScreenCaptureKit-Einzelbilder mit Apple Vision OCR bei Fokus- oder Folienwechseln. Für den MCP-Server wurde ein Token-Budget von ~1.200 Tokens für `get_live_context` sowie ein lückenloses lokales Audit-Log beschlossen.".into(),
        enhanced_markdown: "### Kern-Entscheidungen zur Hintergrund-Pipeline\n- **Zero-Video-Prinzip bestätigt [00:12] [00:29]:** Statt kontinuierlicher Bildschirmaufnahme liest `backtext` auf macOS zunächst den `AXUIElement`-Baum aus (`< 0.4%` CPU-Last auf M3) und triggert `ScreenCaptureKit` + `VNRecognizeTextRequest` gezielt bei Canvas-Apps, PDFs und geteilten Präsentationen.\n- **Screen-Sharing-Deduplizierung [01:19]:** Während laufender Meetings prüft ein `8x8` Luminanz-`dHash` alle 3 Sekunden das Präsentationsfenster; nur bei einer Hamming-Distanz `>= 6` nach `1.5s` Stabilisierung wird eine neue Folie gespeichert.\n\n### MCP-Server & Agenten-Anbindung\n- `get_live_context` bündelt die aktuelle App, die letzten 60 Minuten verdichteter Arbeits-Episoden und offene Action Items in rund `8ms` Antwortzeit [01:44].\n- Jede Abfrage von Claude Desktop oder Cursor erscheint transparent im lokalen **MCP-Audit-Log**.\n\n### UX & Datenschutz\n- Passwortmanager (`1Password`, `Bitwarden`, `Keychain`) und `ConcealedType`-Zwischenablage-Einträge werden vor dem SQLite-Write verworfen [00:55].\n- Anna liefert bis Donnerstag die finalen Zustands-Indikatoren für das Menüleisten-Popover inklusive 15-Minuten-Pause und Schnell-Löschen [02:10].".into(),
        action_items: vec![
            ActionItem {
                id: "act_1".into(),
                owner: "Luka".into(),
                task: "dHash-Schwellwert (Distanz >= 6) und 1.5s Stabilitäts-Debounce in backtext-meeting verdrahten".into(),
                due_hint: Some("Heute".into()),
                completed: true,
            },
            ActionItem {
                id: "act_2".into(),
                owner: "Markus Klein".into(),
                task: "M3-Energie-Benchmark zwischen AXUIElement-Polling und Vision-OCR dokumentieren".into(),
                due_hint: Some("Mittwoch".into()),
                completed: false,
            },
            ActionItem {
                id: "act_3".into(),
                owner: "Anna Weber".into(),
                task: "Menüleisten-Popover für Pause-Intervalle (15m / 1h / Bis Neustart) finalisieren".into(),
                due_hint: Some("Donnerstag".into()),
                completed: false,
            },
        ],
        participants: vec![
            MeetingParticipant {
                speaker_label: "SELF".into(),
                display_name: "Luka (Ich)".into(),
                person_id: None,
                email: None,
                is_self: true,
                identification_source: IdentificationSource::MicSelf,
                confidence: 1.0,
                talk_time_ms: 680_000,
            },
            MeetingParticipant {
                speaker_label: "SPK_1".into(),
                display_name: "Markus Klein".into(),
                person_id: Some("per_markus".into()),
                email: Some("markus@kernwerk.dev".into()),
                is_self: false,
                identification_source: IdentificationSource::Voiceprint,
                confidence: 0.93,
                talk_time_ms: 820_000,
            },
            MeetingParticipant {
                speaker_label: "SPK_2".into(),
                display_name: "Anna Weber".into(),
                person_id: Some("per_anna".into()),
                email: Some("anna.weber@nordlicht.io".into()),
                is_self: false,
                identification_source: IdentificationSource::VisualActiveSpeaker,
                confidence: 0.91,
                talk_time_ms: 610_000,
            },
        ],
        transcript_count: 6,
        screenshare_count: 2,
    };

    let m1_transcript = vec![
        TranscriptSegment {
            id: "seg_1".into(),
            meeting_id: m1_id.clone(),
            speaker_label: "SELF".into(),
            speaker_name: "Luka".into(),
            person_id: None,
            channel: AudioChannel::Microphone,
            start_ms: 12_000,
            end_ms: 28_500,
            text: "Lasst uns heute zwei Dinge festzurren: Erstens wie wir die Kontexterfassung auf macOS maximal akkuschonend halten, und zweitens wie kompakt der Live-Kontext für unsere MCP-Agenten sein muss.".into(),
            confidence: 0.98,
        },
        TranscriptSegment {
            id: "seg_2".into(),
            meeting_id: m1_id.clone(),
            speaker_label: "SPK_1".into(),
            speaker_name: "Markus Klein".into(),
            person_id: Some("per_markus".into()),
            channel: AudioChannel::System,
            start_ms: 29_200,
            end_ms: 54_000,
            text: "Ich teile kurz meine Benchmark-Folie. Wenn wir zuerst über AXUIElement den sichtbaren Fenstertext lesen, liegen wir unter 0,4 Prozent CPU-Last. Nur bei Figma, PDFs oder aktivem Screen-Sharing ziehen wir einen Einzel-Frame über ScreenCaptureKit und schicken ihn durch Apple Vision OCR.".into(),
            confidence: 0.95,
        },
        TranscriptSegment {
            id: "seg_3".into(),
            meeting_id: m1_id.clone(),
            speaker_label: "SPK_2".into(),
            speaker_name: "Anna Weber".into(),
            person_id: Some("per_anna".into()),
            channel: AudioChannel::System,
            start_ms: 55_400,
            end_ms: 78_000,
            text: "Das passt super zu unserem Datenschutz-Versprechen. Wichtig ist mir noch: Sobald jemand 1Password öffnet oder ein privates Browserfenster aktiv ist, muss die Status-Pille sofort zeigen, dass nichts erfasst wird.".into(),
            confidence: 0.94,
        },
        TranscriptSegment {
            id: "seg_4".into(),
            meeting_id: m1_id.clone(),
            speaker_label: "SELF".into(),
            speaker_name: "Luka".into(),
            person_id: None,
            channel: AudioChannel::Microphone,
            start_ms: 79_100,
            end_ms: 103_000,
            text: "Absolut, die 3-Stufen-Filterung läuft vor jedem Datenbank-Write. Und bei geteilten Präsentationen nutzen wir einen 64-Bit dHash mit mindestens 6 Bit Differenz plus anderthalb Sekunden Debounce, damit wir keine tausend Scroll-Zwischenbilder speichern.".into(),
            confidence: 0.98,
        },
        TranscriptSegment {
            id: "seg_5".into(),
            meeting_id: m1_id.clone(),
            speaker_label: "SPK_1".into(),
            speaker_name: "Markus Klein".into(),
            person_id: Some("per_markus".into()),
            channel: AudioChannel::System,
            start_ms: 104_500,
            end_ms: 129_000,
            text: "Auf Folie zwei seht ihr noch die MCP-Latenzwerte: Mit SQLite im WAL-Modus plus FTS5 und 256-dim Vektoren antwortet get_live_context lokal in rund 8 Millisekunden. Ich dokumentiere den vollständigen M3-Benchmark bis Mittwoch.".into(),
            confidence: 0.96,
        },
        TranscriptSegment {
            id: "seg_6".into(),
            meeting_id: m1_id.clone(),
            speaker_label: "SPK_2".into(),
            speaker_name: "Anna Weber".into(),
            person_id: Some("per_anna".into()),
            channel: AudioChannel::System,
            start_ms: 130_200,
            end_ms: 148_000,
            text: "Perfekt. Ich schicke euch bis Donnerstag die finalen Komponenten für das Menüleisten-Popover inklusive 15-Minuten-Pause und dem Schnell-Lösch-Button für die letzten 15 Minuten.".into(),
            confidence: 0.95,
        },
    ];

    let m1_slides = vec![
        ScreenShareFrame {
            id: "frame_1".into(),
            meeting_id: m1_id.clone(),
            captured_at_ms: 34_000,
            presenter_name: Some("Markus Klein".into()),
            dhash_hex: "8f3a2c91e04b7d12".into(),
            image_data_url: Some(make_slide_svg_data_url(
                "SLIDE 01 · 00:34",
                "macOS Capture Overhead (Apple M3 Air)",
                &[
                    "AXUIElement Active Window Read: 0.32% CPU · 0 MB Disk Video",
                    "SCScreenshotManager + Apple Vision OCR: 18 ms / Frame (Neural Engine)",
                    "dHash 64-bit Gate (>= 6 bits + 1.5s settle): -94% redundante Frames",
                    "Dual-Audio Tap (Mic + System SCStream): Bot-frei, 100% Self-Trennung",
                ],
            )),
            ocr_text: "macOS Capture Overhead (Apple M3 Air) — AXUIElement Active Window Read: 0.32% CPU, 0 MB Disk Video. SCScreenshotManager + Apple Vision OCR: 18 ms / Frame. dHash 64-bit Gate (>= 6 bits + 1.5s settle): -94% redundante Frames.".into(),
            slide_title: "macOS Capture Overhead & Vision OCR Benchmark".into(),
        },
        ScreenShareFrame {
            id: "frame_2".into(),
            meeting_id: m1_id.clone(),
            captured_at_ms: 108_000,
            presenter_name: Some("Markus Klein".into()),
            dhash_hex: "19c47f02b58e63a1".into(),
            image_data_url: Some(make_slide_svg_data_url(
                "SLIDE 02 · 01:48",
                "backtext MCP Retrieval Architecture",
                &[
                    "SQLite WAL + FTS5 (BM25) + 256-dim Cosine Vectors (RRF Fusion)",
                    "get_live_context Budget: <= 1.200 Tokens · p95 Latenz: 8.4 ms",
                    "Transports: stdio (Claude Desktop / Cursor) & HTTP 127.0.0.1:39421",
                    "Governance: Granulare Tool-Policies + lokales Echtzeit-Audit-Log",
                ],
            )),
            ocr_text: "backtext MCP Retrieval Architecture — SQLite WAL + FTS5 (BM25) + 256-dim Cosine Vectors (RRF Fusion). get_live_context Budget <= 1200 Tokens, p95 Latenz 8.4 ms. Transports: stdio & HTTP 127.0.0.1:39421.".into(),
            slide_title: "backtext MCP Retrieval Architecture & Latenz".into(),
        },
    ];

    // Upcoming scheduled meeting today so TodayView shows a Pre-Meeting Brief!
    let m2_id = "mtg_upcoming_velox".to_string();
    let m2 = Meeting {
        id: m2_id,
        title: "MCP Workflow Check-in mit Clara Novak (Velox Labs)".into(),
        calendar_event_id: Some("cal_evt_9055".into()),
        detected_app: "Zoom (Kalender-Termin)".into(),
        project_tag: Some("AI Agent Workflows".into()),
        started_at: now + Duration::minutes(35),
        ended_at: None,
        status: MeetingStatus::Scheduled,
        template_id: "discovery".into(),
        prep_brief: Some("**Vorbereitungs-Briefing (aus Gedächtnis & Kontext):**\n- **Letzter Stand mit Clara Novak:** Vor 2 Tagen vereinbart, `get_live_context` und `/agent-handoff` direkt in Cursor zu erproben.\n- **Offene Zusage von Clara:** Feedback zur optimalen Token-Dichte bei mehrstündigen Coding-Sessions.\n- **Dein aktueller Kontext heute:** Im Architektur-Sync mit Markus & Anna wurde das Budget auf `~1.200 Tokens` und `8.4 ms` p95-Latenz festgelegt.".into()),
        raw_user_notes: String::new(),
        executive_summary: String::new(),
        enhanced_markdown: String::new(),
        action_items: vec![],
        participants: vec![
            MeetingParticipant {
                speaker_label: "SELF".into(),
                display_name: "Luka (Ich)".into(),
                person_id: None,
                email: None,
                is_self: true,
                identification_source: IdentificationSource::CalendarPrior,
                confidence: 1.0,
                talk_time_ms: 0,
            },
            MeetingParticipant {
                speaker_label: "CAL_1".into(),
                display_name: "Clara Novak".into(),
                person_id: Some("per_clara".into()),
                email: Some("clara.novak@velox-ai.com".into()),
                is_self: false,
                identification_source: IdentificationSource::CalendarPrior,
                confidence: 0.95,
                talk_time_ms: 0,
            },
        ],
        transcript_count: 0,
        screenshare_count: 0,
    };

    let episodes = vec![
        ContextEpisode {
            id: "ep_mcp_spec".into(),
            started_at: now - Duration::minutes(75),
            ended_at: now - Duration::minutes(30),
            title: "Entwicklung: MCP Server Tools & Privacy-Scrubber in Rust".into(),
            summary: "Arbeit an `crates/backtext-mcp` und `crates/backtext-context`: Definition der JSON-RPC-Schemas für `get_live_context` und `search_memory`, Recherche zur MCP-Spezifikation `2025-06-18` im Browser sowie Test der automatischen API-Key-Maskierung.".into(),
            primary_apps: vec!["Cursor".into(), "Arc Browser".into(), "Ghostty Terminal".into()],
            urls: vec![
                "https://modelcontextprotocol.io/specification/2025-06-18".into(),
                "https://developer.apple.com/documentation/screencapturekit".into(),
            ],
            project_tag: Some("backtext".into()),
            importance_score: 0.92,
            snapshot_count: 8,
        },
        ContextEpisode {
            id: "ep_recipes_design".into(),
            started_at: now - Duration::minutes(28),
            ended_at: now - Duration::minutes(4),
            title: "Konzeption: Granola-inspirierte /-Recipes & Hybrid Notepad UX".into(),
            summary: "Feinschliff der zweisprachigen Prompt-Templates (`/follow-up-email`, `/critical-review`, `/agent-handoff`) und Überprüfung der Typografie-Tokens für das warme Editorial-Interface.".into(),
            primary_apps: vec!["Notion".into(), "Cursor".into()],
            urls: vec!["https://docs.granola.ai/help-center/getting-more-from-your-notes/recipes".into()],
            project_tag: Some("backtext".into()),
            importance_score: 0.86,
            snapshot_count: 5,
        },
    ];

    let snapshots = vec![
        ContextSnapshot {
            id: "snap_1".into(),
            captured_at: now - Duration::minutes(52),
            app_bundle_id: "company.thebrowser.Browser".into(),
            app_name: "Arc Browser".into(),
            window_title: "ScreenCaptureKit | Apple Developer Documentation".into(),
            browser_url: Some("https://developer.apple.com/documentation/screencapturekit".into()),
            dwell_ms: 145_000,
            dhash_hex: "4a91bc02fe118820".into(),
            extracted_text: "SCScreenshotManager.captureImage(contentFilter:configuration:) captures a single frame with minimal energy impact on macOS 14+. Combine with capturesAudio = true and excludesCurrentProcessAudio = true for bot-free meeting audio.".into(),
            extraction_method: ExtractionMethod::Accessibility,
            relevance_score: 0.91,
            project_tag: Some("backtext".into()),
            redacted_secrets_count: 0,
            episode_id: Some("ep_mcp_spec".into()),
        },
        ContextSnapshot {
            id: "snap_2".into(),
            captured_at: now - Duration::minutes(18),
            app_bundle_id: "com.todesktop.230313mzl4w4u92".into(),
            app_name: "Cursor".into(),
            window_title: "server.rs — backtext-mcp — backtext".into(),
            browser_url: None,
            dwell_ms: 310_000,
            dhash_hex: "77d1e409aa3012f5".into(),
            extracted_text: "Implementing MCP tools: get_live_context, search_memory, get_meeting_context, get_person_dossier, get_user_profile. Enforcing McpGovernancePolicy and recording real-time McpAuditEntry rows.".into(),
            extraction_method: ExtractionMethod::Accessibility,
            relevance_score: 0.95,
            project_tag: Some("backtext".into()),
            redacted_secrets_count: 1,
            episode_id: Some("ep_recipes_design".into()),
        },
    ];

    let clipboard = vec![
        ClipboardItem {
            id: "clip_1".into(),
            captured_at: now - Duration::minutes(41),
            source_app: "Arc Browser".into(),
            content_kind: "url".into(),
            text_preview: "https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http".into(),
            content_hash: "a91f4421bc90".into(),
            relevance_score: 0.84,
            project_tag: Some("backtext".into()),
        },
        ClipboardItem {
            id: "clip_2".into(),
            captured_at: now - Duration::minutes(12),
            source_app: "Cursor".into(),
            content_kind: "code_snippet".into(),
            text_preview: "window.set_content_protected(true); // NSWindowSharingNone hides Hover bar from screen sharing".into(),
            content_hash: "e02819cc4187".into(),
            relevance_score: 0.89,
            project_tag: Some("backtext".into()),
        },
    ];

    SeedDataset {
        persons,
        meetings: vec![(m1, m1_transcript, m1_slides), (m2, vec![], vec![])],
        episodes,
        snapshots,
        clipboard,
    }
}
