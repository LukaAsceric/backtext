use anyhow::{bail, Result};
use backtext_core::{AiProviderSettings, BacktextEvent, EventBus, WhisperModelInfo};
use backtext_storage::secrets::{SecretVault, KEY_DEEPGRAM, KEY_GROQ, KEY_OPENAI};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;

const WHISPER_SERVER_PORT: u16 = 39422;

struct WhisperServerProc {
    model_path: PathBuf,
    child: Child,
}

static WHISPER_SERVER_STATE: OnceLock<AsyncMutex<Option<WhisperServerProc>>> = OnceLock::new();

fn whisper_server_lock() -> &'static AsyncMutex<Option<WhisperServerProc>> {
    WHISPER_SERVER_STATE.get_or_init(|| AsyncMutex::new(None))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribedUtterance {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker_hint: Option<String>,
    pub confidence: f32,
}

pub fn models_directory() -> PathBuf {
    if let Ok(p) = std::env::var("BACKTEXT_MODELS_DIR") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".backtext-data").join("models")
}

pub fn detect_whisper_server_path() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("whisper-server");
            if bundled.exists() {
                return Some(bundled);
            }
        }
    }
    let candidates = [
        "/Applications/backtext.app/Contents/MacOS/whisper-server",
        "/opt/homebrew/bin/whisper-server",
        "/usr/local/bin/whisper-server",
        "/usr/bin/whisper-server",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("whisper-server");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn detect_whisper_cli_path() -> Option<PathBuf> {
    // 1. Prefer bundled Apple Silicon Metal whisper-cli shipped inside backtext.app/Contents/MacOS/
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("whisper-cli");
            if bundled.exists() {
                return Some(bundled);
            }
        }
    }

    let candidates = [
        "/Applications/backtext.app/Contents/MacOS/whisper-cli",
        "/opt/homebrew/bin/whisper-cli",
        "/usr/local/bin/whisper-cli",
        "/opt/homebrew/bin/whisper-cpp",
        "/usr/bin/whisper-cli",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in ["whisper-cli", "whisper-cpp"] {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Gracefully shuts down the persistent local `whisper-server` (e.g. when a meeting finishes)
/// so zero GPU/RAM is consumed while idle.
pub async fn stop_whisper_server() {
    let mut guard = whisper_server_lock().lock().await;
    if let Some(mut proc) = guard.take() {
        let _ = proc.child.kill().await;
    }
}

/// Ensures `whisper-server` is running warm in Apple Metal for `model_path`.
/// Keeps the 574MB GGML model resident across chunks instead of cold-loading 30x/minute.
async fn ensure_whisper_server_running(
    http: &reqwest::Client,
    server_bin: &Path,
    model_path: &Path,
) -> Result<String> {
    let mut guard = whisper_server_lock().lock().await;
    let base_url = format!("http://127.0.0.1:{WHISPER_SERVER_PORT}");
    let inference_url = format!("{base_url}/inference");

    if let Some(ref mut proc) = *guard {
        let alive = matches!(proc.child.try_wait(), Ok(None));
        if alive && proc.model_path == model_path {
            return Ok(inference_url);
        }
        let _ = proc.child.kill().await;
        *guard = None;
    }

    let child = Command::new(server_bin)
        .args([
            "-m",
            &model_path.display().to_string(),
            "--host",
            "127.0.0.1",
            "--port",
            &WHISPER_SERVER_PORT.to_string(),
            "-t",
            "4",
            "-l",
            "auto",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;

    *guard = Some(WhisperServerProc {
        model_path: model_path.to_path_buf(),
        child,
    });

    // Wait up to 9s for Metal model load to complete
    for _ in 0..36 {
        if let Ok(resp) = http
            .get(&base_url)
            .timeout(std::time::Duration::from_millis(450))
            .send()
            .await
        {
            if resp.status().as_u16() < 500 {
                return Ok(inference_url);
            }
        }
        if let Some(ref mut proc) = *guard {
            if !matches!(proc.child.try_wait(), Ok(None)) {
                *guard = None;
                bail!("whisper-server exited unexpectedly during model load");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    if let Some(mut proc) = guard.take() {
        let _ = proc.child.kill().await;
    }
    bail!("Timed out waiting for whisper-server to load model")
}

/// Returns `(ready, human_status)` describing which live STT engine will transcribe recorded audio.
pub fn stt_readiness_status(settings: &AiProviderSettings, vault: &SecretVault) -> (bool, String) {
    match settings.stt_provider.as_str() {
        "deepgram" if vault.has_secret(KEY_DEEPGRAM) => {
            return (true, "Deepgram Nova-3 (Live + Diarisierung)".into());
        }
        "openai_audio" if vault.has_secret(KEY_OPENAI) => {
            return (true, "OpenAI Whisper".into());
        }
        "groq_whisper" if vault.has_secret(KEY_GROQ) => {
            return (true, "Groq Whisper Large-v3-Turbo".into());
        }
        _ => {}
    }

    let server_ok = detect_whisper_server_path().is_some();
    let cli_ok = server_ok || detect_whisper_cli_path().is_some();
    let model_path = models_directory().join(format!("{}.bin", settings.whisper_model_id));
    let model_ok = model_path.exists()
        && std::fs::metadata(&model_path)
            .map(|m| m.len() > 1_000_000)
            .unwrap_or(false);

    if cli_ok && model_ok {
        return (
            true,
            format!(
                "Lokal: {} (Apple Metal{})",
                settings.whisper_model_id,
                if server_ok { " · Warm-Server" } else { "" }
            ),
        );
    }

    if vault.has_secret(KEY_DEEPGRAM) {
        return (true, "Deepgram Nova-3 (Fallback)".into());
    }
    if vault.has_secret(KEY_OPENAI) {
        return (true, "OpenAI Whisper (Fallback)".into());
    }
    if vault.has_secret(KEY_GROQ) {
        return (true, "Groq Whisper (Fallback)".into());
    }

    if cli_ok && !model_ok {
        return (
            false,
            "Audio wird mitgeschnitten · Lade in Einstellungen → Lokale Modelle ein Whisper-Modell (oder hinterlege Deepgram/OpenAI/Groq).".into(),
        );
    }

    (
        false,
        "Audio wird lokal gespeichert · Für Live-Transkription bitte Whisper-Modell in Einstellungen laden oder Deepgram/OpenAI/Groq-Key hinterlegen.".into(),
    )
}

pub fn list_whisper_models() -> Vec<WhisperModelInfo> {
    let dir = models_directory();
    let specs = [
        (
            "ggml-base",
            "Whisper Base (Schnell · DE/EN)",
            142,
            "99 Sprachen (inkl. DE/EN)",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        ),
        (
            "ggml-medium-q5_0",
            "Whisper Medium Q5_0 (Ausgewogen · Metal)",
            539,
            "Hohe DE/EN Genauigkeit",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin",
        ),
        (
            "ggml-large-v3-turbo-q5_0",
            "Whisper Large-v3-Turbo Q5_0 (Empfohlen für M1–M5 Pro)",
            574,
            "Best-in-Class DE/EN & Fachbegriffe",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        ),
    ];

    specs
        .into_iter()
        .map(|(id, name, size_mb, lang, url)| {
            let file_path = dir.join(format!("{id}.bin"));
            let installed = file_path.exists()
                && std::fs::metadata(&file_path)
                    .map(|m| m.len() > 1_000_000)
                    .unwrap_or(false);
            WhisperModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                size_mb,
                language_support: lang.to_string(),
                download_url: url.to_string(),
                installed,
                local_path: if installed {
                    Some(file_path.display().to_string())
                } else {
                    None
                },
                downloading: false,
                progress_pct: if installed { 100 } else { 0 },
            }
        })
        .collect()
}

pub async fn download_whisper_model(
    http: &reqwest::Client,
    model_id: &str,
    events: &EventBus,
) -> Result<WhisperModelInfo> {
    let models = list_whisper_models();
    let target = models
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown Whisper model '{model_id}'"))?;

    let dir = models_directory();
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(format!("{model_id}.bin"));
    let tmp = dir.join(format!("{model_id}.bin.part"));

    let resp = http
        .get(&target.download_url)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await?
        .error_for_status()?;

    let total = resp.content_length().unwrap_or((target.size_mb as u64) * 1024 * 1024);
    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut downloaded: u64 = 0;
    let mut last_pct: u8 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        let pct = ((downloaded.saturating_mul(100)) / total.max(1)).min(99) as u8;
        if pct >= last_pct + 2 {
            last_pct = pct;
            events.publish(BacktextEvent::ModelDownloadProgress {
                model_id: model_id.to_string(),
                progress_pct: pct,
                installed: false,
                error: None,
            });
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp, &dest).await?;

    events.publish(BacktextEvent::ModelDownloadProgress {
        model_id: model_id.to_string(),
        progress_pct: 100,
        installed: true,
        error: None,
    });

    Ok(WhisperModelInfo {
        installed: true,
        local_path: Some(dest.display().to_string()),
        downloading: false,
        progress_pct: 100,
        ..target
    })
}

/// Transcribes a 16kHz PCM WAV file using the user's configured STT provider:
/// - `deepgram`: Deepgram Nova-3 with built-in multi-language & speaker diarization
/// - `openai_audio`: OpenAI Whisper / `gpt-4o-transcribe` with Custom Vocabulary prompt
/// - `groq_whisper`: Groq `whisper-large-v3-turbo`
/// - `local_whisper`: Local Apple Metal `whisper-cli` + downloaded GGML model
pub async fn transcribe_wav_file(
    http: &reqwest::Client,
    wav_path: &Path,
    vocabulary_prompt: &str,
    settings: &AiProviderSettings,
    vault: &SecretVault,
) -> Result<Vec<TranscribedUtterance>> {
    if !wav_path.exists() {
        return Ok(Vec::new());
    }

    match settings.stt_provider.as_str() {
        "deepgram" => {
            if let Some(key) = vault.get_secret(KEY_DEEPGRAM) {
                return transcribe_deepgram(http, wav_path, &key).await;
            }
        }
        "openai_audio" => {
            if let Some(key) = vault.get_secret(KEY_OPENAI) {
                return transcribe_openai_compatible(
                    http,
                    "https://api.openai.com/v1/audio/transcriptions",
                    "whisper-1",
                    wav_path,
                    vocabulary_prompt,
                    &key,
                )
                .await;
            }
        }
        "groq_whisper" => {
            if let Some(key) = vault.get_secret(KEY_GROQ) {
                return transcribe_openai_compatible(
                    http,
                    "https://api.groq.com/openai/v1/audio/transcriptions",
                    "whisper-large-v3-turbo",
                    wav_path,
                    vocabulary_prompt,
                    &key,
                )
                .await;
            }
        }
        _ => {}
    }

    // Local Apple Metal Whisper: prefer persistent warm whisper-server (zero 574MB GPU reloads!)
    let model_path = models_directory().join(format!("{}.bin", settings.whisper_model_id));
    if model_path.exists() {
        if let Some(server_bin) = detect_whisper_server_path() {
            if let Ok(endpoint) = ensure_whisper_server_running(http, &server_bin, &model_path).await
            {
                if let Ok(utts) =
                    transcribe_via_whisper_server(http, &endpoint, wav_path, vocabulary_prompt)
                        .await
                {
                    return Ok(utts);
                }
            }
        }
        if let Some(cli) = detect_whisper_cli_path() {
            return transcribe_local_whisper_cli(&cli, &model_path, wav_path, vocabulary_prompt)
                .await;
        }
    }

    // Opportunistic fallback if user stored Deepgram / OpenAI / Groq keys
    if let Some(key) = vault.get_secret(KEY_DEEPGRAM) {
        return transcribe_deepgram(http, wav_path, &key).await;
    }
    if let Some(key) = vault.get_secret(KEY_OPENAI) {
        return transcribe_openai_compatible(
            http,
            "https://api.openai.com/v1/audio/transcriptions",
            "whisper-1",
            wav_path,
            vocabulary_prompt,
            &key,
        )
        .await;
    }

    Ok(Vec::new())
}

async fn transcribe_via_whisper_server(
    http: &reqwest::Client,
    endpoint: &str,
    wav_path: &Path,
    vocabulary_prompt: &str,
) -> Result<Vec<TranscribedUtterance>> {
    let bytes = tokio::fs::read(wav_path).await?;
    let file_part = reqwest::multipart::Part::bytes(bytes)
        .file_name("chunk.wav")
        .mime_str("audio/wav")?;

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("temperature", "0.0")
        .text("response_format", "verbose_json")
        .text("prompt", vocabulary_prompt.to_string());

    let resp = http
        .post(endpoint)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await?
        .error_for_status()?;

    let v: serde_json::Value = resp.json().await?;
    let mut out = Vec::new();
    if let Some(segs) = v["segments"].as_array() {
        for seg in segs {
            let text = seg["text"].as_str().unwrap_or("").trim().to_string();
            if text.is_empty() || (text.starts_with('[') && text.ends_with(']')) {
                continue;
            }
            let start_ms = (seg["start"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            let end_ms = (seg["end"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            out.push(TranscribedUtterance {
                start_ms,
                end_ms: end_ms.max(start_ms + 500),
                text,
                speaker_hint: None,
                confidence: 0.94,
            });
        }
    } else if let Some(text) = v["text"].as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() && !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
            out.push(TranscribedUtterance {
                start_ms: 0,
                end_ms: 4000,
                text: trimmed.to_string(),
                speaker_hint: None,
                confidence: 0.93,
            });
        }
    }
    Ok(out)
}

async fn transcribe_local_whisper_cli(
    cli_path: &Path,
    model_path: &Path,
    wav_path: &Path,
    vocabulary_prompt: &str,
) -> Result<Vec<TranscribedUtterance>> {
    let out_prefix = wav_path.with_extension("whisper");
    let status = Command::new(cli_path)
        .args([
            "-m",
            &model_path.display().to_string(),
            "-f",
            &wav_path.display().to_string(),
            "-t",
            "4",
            "-l",
            "auto",
            "--prompt",
            vocabulary_prompt,
            "-oj",
            "-of",
            &out_prefix.display().to_string(),
        ])
        .output()
        .await?;

    if !status.status.success() {
        bail!(
            "whisper-cli failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    let json_path = PathBuf::from(format!("{}.json", out_prefix.display()));
    let raw = tokio::fs::read_to_string(&json_path).await?;
    let _ = tokio::fs::remove_file(&json_path).await;
    let parsed: serde_json::Value = serde_json::from_str(&raw)?;
    let mut out = Vec::new();
    if let Some(arr) = parsed["transcription"].as_array() {
        for item in arr {
            let text = item["text"].as_str().unwrap_or("").trim().to_string();
            if text.is_empty() || text.starts_with('[') && text.ends_with(']') {
                continue;
            }
            let start_ms = item["offsets"]["from"].as_u64().unwrap_or(0);
            let end_ms = item["offsets"]["to"].as_u64().unwrap_or(start_ms + 3000);
            out.push(TranscribedUtterance {
                start_ms,
                end_ms,
                text,
                speaker_hint: None,
                confidence: 0.94,
            });
        }
    }
    Ok(out)
}

async fn transcribe_deepgram(
    http: &reqwest::Client,
    wav_path: &Path,
    api_key: &str,
) -> Result<Vec<TranscribedUtterance>> {
    let bytes = tokio::fs::read(wav_path).await?;
    let resp = http
        .post("https://api.deepgram.com/v1/listen?model=nova-3&smart_format=true&diarize=true&utterances=true&detect_language=true")
        .header("Authorization", format!("Token {api_key}"))
        .header("Content-Type", "audio/wav")
        .body(bytes)
        .send()
        .await?
        .error_for_status()?;

    let v: serde_json::Value = resp.json().await?;
    let mut out = Vec::new();
    if let Some(utts) = v["results"]["utterances"].as_array() {
        for u in utts {
            let text = u["transcript"].as_str().unwrap_or("").trim().to_string();
            if text.is_empty() {
                continue;
            }
            let start_ms = (u["start"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            let end_ms = (u["end"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            let spk = u["speaker"].as_u64().map(|n| format!("DG_SPK_{n}"));
            let conf = u["confidence"].as_f64().unwrap_or(0.92) as f32;
            out.push(TranscribedUtterance {
                start_ms,
                end_ms,
                text,
                speaker_hint: spk,
                confidence: conf,
            });
        }
    }
    Ok(out)
}

async fn transcribe_openai_compatible(
    http: &reqwest::Client,
    endpoint: &str,
    model: &str,
    wav_path: &Path,
    vocabulary_prompt: &str,
    api_key: &str,
) -> Result<Vec<TranscribedUtterance>> {
    let bytes = tokio::fs::read(wav_path).await?;
    let file_part = reqwest::multipart::Part::bytes(bytes)
        .file_name("chunk.wav")
        .mime_str("audio/wav")?;

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("response_format", "verbose_json")
        .text("prompt", vocabulary_prompt.to_string());

    let resp = http
        .post(endpoint)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?;

    let v: serde_json::Value = resp.json().await?;
    let mut out = Vec::new();
    if let Some(segs) = v["segments"].as_array() {
        for seg in segs {
            let text = seg["text"].as_str().unwrap_or("").trim().to_string();
            if text.is_empty() {
                continue;
            }
            let start_ms = (seg["start"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            let end_ms = (seg["end"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            out.push(TranscribedUtterance {
                start_ms,
                end_ms,
                text,
                speaker_hint: None,
                confidence: 0.94,
            });
        }
    } else if let Some(text) = v["text"].as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            out.push(TranscribedUtterance {
                start_ms: 0,
                end_ms: 5000,
                text: trimmed.to_string(),
                speaker_hint: None,
                confidence: 0.92,
            });
        }
    }
    Ok(out)
}
