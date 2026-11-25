# backtext

**`backtext`** (*background* + *context*) ist eine Local-First **macOS-Desktop-App auf Basis von Tauri v2 (Rust + React/TypeScript)**, die die besten Konzepte von **Granola** (bot-freie Meeting-Aufzeichnung, Hybrid-Notepad, KI-Zusammenfassungen & `/`-*Recipes*) und **Littlebird** (leise Hintergrund-Kontexterfassung über Fenster, Apps, Browser-Tabs, Zwischenablage und Bildschirm-OCR) vereint — maßgeschneidert als kontinuierliche Kontext-Schicht für deine **KI-Agenten via MCP**.

---

## Kern-Features

1. **Bot-freie Meeting-Aufzeichnung & Auto-Erkennung (`backtext-meeting`)**
   - Erkennt Meeting-Beginn und -Ende automatisch über Mikrofon-Aktivität (`CoreAudio`), aktive Konferenz-Apps (`Zoom`, `Google Meet`, `Teams`, `Slack Huddle`) und Kalender-Korrelation — inklusive Reconnect-Cooldown (`45s`).
   - **Dual-Channel Audio**: Eigenes Mikrofon (`Channel 0 -> Ich`) und System-Audio via `ScreenCaptureKit` (`Channel 1 -> Remote-Teilnehmer`) sind sauber getrennt.
   - **Screen-Sharing-Screenshots**: Erkennt geteilte Folien/Code-Ansichten, dedupliziert sie per 64-Bit `dHash` (`>= 6 Bits` + `1.5s` Bildstabilität) und extrahiert Inhalte lokal via Apple `Vision` OCR (`VNRecognizeTextRequest`).

2. **4-Stufen Teilnehmer-Identifikation (`SpeakerAttributionSession`)**
   - **Stufe 1:** Dedizierte Mikrofon-Spur = verifizierter eigener Nutzer (`SELF`).
   - **Stufe 2:** Lokaler 256-dim Stimmprofil-Abgleich (`Voiceprint` Kosinus-Ähnlichkeit + Kalender-Teilnehmer-Prior).
   - **Stufe 3:** Visuelle Active-Speaker-Korrelation im Meeting-Fenster (`Accessibility` / `OCR`).
   - **Stufe 4:** Online-Stimm-Clustering + 1-Klick-Umbenennung im UI mit dauerhaftem Voiceprint-Lernen.

3. **Granola-inspiriertes Hybrid-Notepad, KI-Chat & `/`-Recipes (`backtext-ai`)**
   - Eigene Stichpunkte während des Gesprächs verschmelzen mit Transkript & Folien-OCR zu strukturierten *Enhanced Notes* und *Action Items*.
   - 12 integrierte zweisprachige (DE/EN) **`/`-Recipes** (`/follow-up-email`, `/action-items`, `/critical-review`, `/prd-draft`, `/slide-digest`, `/weekly-commitments`, `/morning-brief`, `/agent-handoff`, u. v. m.) + eigener **Recipes Studio**-Editor.

4. **Hintergrund-Kontexterfassung & 3-Stufen-Filter (`backtext-context`)**
   - Liest auf macOS primär ressourcenschonend über `AXUIElement` (< 0,4 % CPU) und bei Bedarf über `ScreenCaptureKit` + `Vision` OCR sowie sichere `NSPasteboard`-Zwischenablage (ignoriert `ConcealedType` / `1Password`).
   - **3-Stufen-Pipeline vor jedem SQLite-Write:**
     1. *Privacy Shield & PII-Scrubber* (Blocklisten für Passwortmanager/Banking/Privatfenster + lokale Maskierung von API-Keys, JWTs, PEM-Keys, IBANs, Kreditkarten).
     2. *Visuelle & textuelle Deduplizierung* (`dHash` + Token-Jaccard + `< 3s` Durchtab-Filter).
     3. *Semantisches Relevanz-Scoring & Episoden-Verdichtung*.

5. **Integrierter lokaler MCP-Server (`backtext-mcp`)**
   - Unterstützt **`stdio`** (`backtext-mcp`-Binary für Claude Desktop & Cursor) und **`Streamable HTTP`** (`http://127.0.0.1:39421/mcp`).
   - 10 spezialisierte Agenten-Tools (`get_live_context`, `search_memory`, `list_meetings`, `get_meeting_context`, `get_meeting_transcript`, `get_meeting_screenshots`, `get_person_dossier`, `get_project_context`, `get_user_profile`, `remember_agent_insight`), MCP Resources, exponierte Recipe-Prompts, granulare Berechtigungsschalter und Live-Audit-Log.

6. **Nutzerpersonalisierung & Modernes UI (*Quiet Editorial Craft*)**
   - Deutsch (Standard) & Englisch live umschaltbar, Light- & Dark-Theme, `Hover`-Schnellassistent (`⌥ Space`, unsichtbar in Bildschirmfreigaben via `NSWindowSharingNone`), Fachglossar für präzisere Spracherkennung und eigenes Stimmprofil.

---

## Projektstruktur

- `crates/backtext-core` — Domänenmodelle, Personalisierungs-Dossier, Vektor-Mathematik & Event-Bus
- `crates/backtext-storage` — SQLite (WAL + FTS5 BM25 + 256-dim Vektor-Index mit Reciprocal Rank Fusion)
- `crates/backtext-context` — 3-Stufen Privacy-Scrubber, `dHash`-Deduplizierung & Episoden-Kompaktierung
- `crates/backtext-platform` — Native macOS-Sensoren (`CoreAudio`, `CoreGraphics`, `Accessibility`, `Vision` OCR, `NSPasteboard`) + Cross-Platform-Simulator
- `crates/backtext-meeting` — Auto-Meeting-Detektor, 4-Stufen Teilnehmer-ID & Screen-Share-Deduplikator
- `crates/backtext-ai` — Hybrid-KI-Router (Lokales Whisper/Ollama + optionale Cloud-APIs) & `/`-Recipes-Engine
- `crates/backtext-mcp` — JSON-RPC 2.0 MCP-Server (`lib` + `backtext-mcp` `stdio` Binary)
- `src-tauri/` — Tauri v2 Desktop-Shell, Menüleisten-Tray, Hover-Fenster & lokaler HTTP/MCP-Daemon
- `src/` — React + TypeScript + Vanilla CSS Design-System (*Quiet Editorial Craft*)

---

## Entwicklung & Build

```bash
# Frontend-Abhängigkeiten installieren & prüfen
pnpm install
pnpm run typecheck
pnpm run test
pnpm run build

# Gesamte Rust-Test-Suite ausführen
cargo test --workspace

# Auf macOS im Entwicklungsmodus starten
pnpm tauri dev

# Auf macOS signierbares .app & .dmg + backtext-mcp Binary bauen
cargo build --release -p backtext-mcp --bin backtext-mcp
pnpm tauri build
```
