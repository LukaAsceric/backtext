# backtext (`v0.2.0`)

**`backtext`** (*background* + *context*) ist eine Local-First **macOS-Desktop-App auf Basis von Tauri v2 (Rust + Swift + React/TypeScript)**, die die Stärken von **Granola** (bot-freie Meeting-Aufzeichnung, Hybrid-Notepad, KI-Zusammenfassungen & `/`-*Recipes*) und **Littlebird** (stille Hintergrund-Kontexterfassung über Fenster, Apps, Browser-Tabs, Zwischenablage und Bildschirm-OCR) vereint — maßgeschneidert als kontinuierliches, AES-256-verschlüsseltes Arbeitsgedächtnis für deine **KI-Agenten via MCP**.

---

## Alle Features (Phasen 1–6 komplett umgesetzt)

1. **Autonome Hintergrund-Kontexterfassung & 3-Stufen-Filter (`backtext-context` & `backtext-macos-sensor`)**
   - Kompilierter nativer Swift-Sensor (`ScreenCaptureKit`, `Apple Vision` OCR, `AXUIElement`, `NSPasteboard`) im Hintergrund-Daemon-Loop (respektiert Pause-Intervalle, Bildschirmsperre & Idle).
   - **3-Stufen-Filter vor jedem SQLite-Write:** Privacy-Blocklisten (`1Password`, `Bitwarden`, Banking, Privatfenster, `AXSecureTextField`), lokaler Secret-/PII-Scrubber (API-Keys, JWTs, PEM-Keys, IBANs, Kreditkarten), `dHash`/Jaccard-Deduplizierung und automatische Episoden-Verdichtung.

2. **Bot-freie Meeting-Aufzeichnung, Akustische Voiceprints & Screen-Share-Folien (`backtext-meeting`)**
   - Automatische Meeting-Erkennung (CoreAudio Mic + App/Browser + EventKit/ICS Kalender inkl. 45s Reconnect-Cooldown).
   - **Dual-Channel Audio**: Eigenes Mikrofon (`Channel 0 -> Ich`) und System-Audio via `ScreenCaptureKit` (`Channel 1 -> Remote`).
   - **Echte Akustische DSP-Pipeline (`audio_dsp.rs`)**: 16kHz PCM Energy/ZCR-VAD und lokale 256-dim MFCC/Pitch/Formanten-Stimmprofile inkl. 5s Mikrofon-Kalibrierung.
   - **4-Stufen Teilnehmer-Identifikation** + per 64-Bit `dHash` deduplizierte Screen-Sharing-Folien mit Apple `Vision` OCR.

3. **Hybrid-KI, Whisper-Modell-Manager & Multi-LLM Live-Streaming (`backtext-ai`)**
   - **STT:** In-App Whisper GGML Modell-Downloader (Apple Metal via `whisper-cli`) + Cloud-STT (Deepgram Nova-3 mit Diarisierung, OpenAI Whisper, Groq).
   - **Live-Streaming LLMs:** Native SSE-Clients für **Anthropic Claude**, **OpenAI**, **Google Gemini** und lokales **Ollama** (inkl. Modell-Auto-Erkennung & Verbindungs-Test).
   - **Neuronale Embeddings:** Wahlweise Offline Multilingual-256, Ollama (`/api/embed`) oder OpenAI (`text-embedding-3-small`).
   - **12 zweisprachige (DE/EN) `/`-Recipes** + eigenes **Recipes Studio**.

4. **SQLCipher AES-256 Verschlüsselung, macOS Keychain & MCP-Server (`backtext-storage` & `backtext-mcp`)**
   - **At-Rest Verschlüsselung:** SQLCipher AES-256 Datenbank-Verschlüsselung mit Schlüsselverwaltung über den **macOS Schlüsselbund (`Security.framework`)**.
   - **MCP-Server (`stdio` & `Streamable HTTP`):** 10 Agenten-Tools (`get_live_context`, `search_memory`, `get_meeting_context`, `get_meeting_transcript`, `get_meeting_screenshots`, `get_person_dossier`, `get_project_context`, `get_user_profile`, `remember_agent_insight`, `list_meetings`), MCP Resources, Recipe-Prompts, Governance-Schalter & Echtzeit-Audit-Log.

5. **macOS System-Integration (`src-tauri`)**
   - Systemweiter Global Shortcut **`⌥ Space`** für das schwebende **`Hover`-Overlay** (`NSWindowSharingNone` — unsichtbar bei eigener Bildschirmfreigabe).
   - Nativer **`EventKit`- & `.ics`-Kalender-Sync** mit automatischen Vorbereitungs-Briefings & 5-Minuten-Benachrichtigung.
   - **Autostart beim macOS-Login** (`LaunchAgent`), 1-Klick Berechtigungs-Onboarding, Demo-Daten-Reset & GitHub-Release-Update-Prüfung.

---

## Installation auf macOS (Apple Silicon M1–M5 Pro/Max)

1. Lade unter **GitHub Releases (`v0.2.0`)** die Datei **`backtext_0.2.0_macos_arm64.dmg`** herunter.
2. Ziehe **`backtext.app`** in deinen `/Applications`-Ordner (`backtext-mcp` und der kompilierte `backtext-macos-sensor` sind direkt im Bundle integriert und tief ad-hoc signiert).
3. Entferne vor dem ersten Start einmalig das Browser-Quarantäne-Attribut im Terminal:
   ```bash
   xattr -cr /Applications/backtext.app && open /Applications/backtext.app
   ```
4. Erteile beim ersten Start im Onboarding-Streifen die macOS-Freigaben für *ScreenCaptureKit*, *Mikrofon*, *Bedienungshilfen* und *Kalender*.
