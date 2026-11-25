use anyhow::{bail, Result};
use backtext_core::{
    embed_text_local_256, l2_normalize, AiProviderSettings, BacktextEvent, EventBus, EMBEDDING_DIM,
};
use backtext_storage::secrets::{SecretVault, KEY_ANTHROPIC, KEY_GEMINI, KEY_OPENAI};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub provider: String,
    pub ok: bool,
    pub message: String,
    pub latency_ms: u64,
}

/// Discovers locally installed Ollama models (`GET {ollama_url}/api/tags`).
pub async fn list_ollama_models(http: &reqwest::Client, ollama_url: &str) -> Vec<String> {
    let url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));
    let Ok(resp) = http.get(&url).timeout(Duration::from_millis(900)).send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(v) = resp.json::<serde_json::Value>().await else {
        return Vec::new();
    };
    v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Projects an arbitrary-length neural embedding vector (e.g. 768 or 1536 dims)
/// cleanly down to `EMBEDDING_DIM` (256) via folded sign-preserving pooling + L2 norm
/// so it remains 100% compatible with the local SQLite vector store.
pub fn project_to_256(raw: &[f32]) -> Vec<f32> {
    if raw.is_empty() {
        return vec![0.0_f32; EMBEDDING_DIM];
    }
    let mut out = vec![0.0_f32; EMBEDDING_DIM];
    for (idx, &val) in raw.iter().enumerate() {
        let sign = if (idx / EMBEDDING_DIM) % 2 == 0 {
            1.0
        } else {
            -1.0
        };
        out[idx % EMBEDDING_DIM] += sign * val;
    }
    l2_normalize(&mut out);
    out
}

/// Computes a 256-dim embedding using configured neural provider (Ollama `/api/embed` or
/// OpenAI `text-embedding-3-small`) with zero-latency fallback to offline multilingual hashing.
pub async fn compute_embedding_256(
    http: &reqwest::Client,
    text: &str,
    settings: &AiProviderSettings,
    vault: &SecretVault,
) -> Vec<f32> {
    if settings.embedding_provider == "ollama" {
        let url = format!("{}/api/embed", settings.ollama_url.trim_end_matches('/'));
        if let Ok(resp) = http
            .post(&url)
            .timeout(Duration::from_millis(1200))
            .json(&serde_json::json!({
                "model": settings.embedding_model,
                "input": text
            }))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = v["embeddings"][0].as_array() {
                        let raw: Vec<f32> = arr
                            .iter()
                            .filter_map(|x| x.as_f64().map(|f| f as f32))
                            .collect();
                        if !raw.is_empty() {
                            return project_to_256(&raw);
                        }
                    }
                }
            }
        }
    } else if settings.embedding_provider == "openai" {
        if let Some(api_key) = vault.get_secret(KEY_OPENAI) {
            if let Ok(resp) = http
                .post("https://api.openai.com/v1/embeddings")
                .bearer_auth(api_key)
                .timeout(Duration::from_secs(5))
                .json(&serde_json::json!({
                    "model": "text-embedding-3-small",
                    "dimensions": EMBEDDING_DIM,
                    "input": text
                }))
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(v) = resp.json::<serde_json::Value>().await {
                        if let Some(arr) = v["data"][0]["embedding"].as_array() {
                            let raw: Vec<f32> = arr
                                .iter()
                                .filter_map(|x| x.as_f64().map(|f| f as f32))
                                .collect();
                            if !raw.is_empty() {
                                return project_to_256(&raw);
                            }
                        }
                    }
                }
            }
        }
    }

    embed_text_local_256(text)
}

/// Streams a completion across Anthropic Claude, OpenAI, Google Gemini, or Local Ollama,
/// emitting real-time `ChatStreamDelta` events over the `EventBus`.
pub async fn stream_completion(
    http: &reqwest::Client,
    settings: &AiProviderSettings,
    vault: &SecretVault,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> Option<(String, String)> {
    let provider_order: Vec<&str> = match settings.llm_provider.as_str() {
        "anthropic" => vec!["anthropic", "ollama"],
        "openai" => vec!["openai", "ollama"],
        "gemini" => vec!["gemini", "ollama"],
        "local_ollama" => vec!["ollama"],
        _ => vec!["anthropic", "openai", "gemini", "ollama"], // hybrid_auto
    };

    for provider in provider_order {
        match provider {
            "anthropic" => {
                if let Some(key) = vault.get_secret(KEY_ANTHROPIC) {
                    let model = if settings.llm_model.starts_with("claude") {
                        settings.llm_model.as_str()
                    } else {
                        "claude-sonnet-4-20250514"
                    };
                    if let Ok(text) =
                        stream_anthropic(http, &key, model, system_prompt, user_prompt, stream_ctx)
                            .await
                    {
                        if !text.trim().is_empty() {
                            return Some((text, format!("anthropic:{model}")));
                        }
                    }
                }
            }
            "openai" => {
                if let Some(key) = vault.get_secret(KEY_OPENAI) {
                    let model = if settings.llm_model.starts_with("gpt")
                        || settings.llm_model.starts_with("o3")
                        || settings.llm_model.starts_with("o4")
                    {
                        settings.llm_model.as_str()
                    } else {
                        "gpt-4o-mini"
                    };
                    if let Ok(text) =
                        stream_openai(http, &key, model, system_prompt, user_prompt, stream_ctx)
                            .await
                    {
                        if !text.trim().is_empty() {
                            return Some((text, format!("openai:{model}")));
                        }
                    }
                }
            }
            "gemini" => {
                if let Some(key) = vault.get_secret(KEY_GEMINI) {
                    let model = if settings.llm_model.starts_with("gemini") {
                        settings.llm_model.as_str()
                    } else {
                        "gemini-2.5-flash"
                    };
                    if let Ok(text) =
                        stream_gemini(http, &key, model, system_prompt, user_prompt, stream_ctx)
                            .await
                    {
                        if !text.trim().is_empty() {
                            return Some((text, format!("gemini:{model}")));
                        }
                    }
                }
            }
            "ollama" => {
                let model = if settings.llm_provider == "local_ollama"
                    && !settings.llm_model.is_empty()
                {
                    settings.llm_model.as_str()
                } else {
                    "qwen3:8b"
                };
                if let Ok(text) = stream_ollama(
                    http,
                    &settings.ollama_url,
                    model,
                    system_prompt,
                    user_prompt,
                    stream_ctx,
                )
                .await
                {
                    if !text.trim().is_empty() {
                        return Some((text, format!("ollama:{model}")));
                    }
                }
            }
            _ => {}
        }
    }

    None
}

fn emit_delta(stream_ctx: Option<(&EventBus, &str, &str)>, delta: &str, done: bool) {
    if let Some((bus, thread_id, message_id)) = stream_ctx {
        bus.publish(BacktextEvent::ChatStreamDelta {
            thread_id: thread_id.to_string(),
            message_id: message_id.to_string(),
            delta: delta.to_string(),
            done,
        });
    }
}

async fn stream_anthropic(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> Result<String> {
    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .timeout(Duration::from_secs(60))
        .json(&serde_json::json!({
            "model": model,
            "max_tokens": 2048,
            "stream": true,
            "system": system_prompt,
            "messages": [{ "role": "user", "content": user_prompt }]
        }))
        .send()
        .await?
        .error_for_status()?;

    let mut full = String::new();
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if v["type"] == "content_block_delta" {
                        if let Some(delta) = v["delta"]["text"].as_str() {
                            full.push_str(delta);
                            emit_delta(stream_ctx, delta, false);
                        }
                    }
                }
            }
        }
    }
    emit_delta(stream_ctx, "", true);
    Ok(full)
}

async fn stream_openai(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> Result<String> {
    let resp = http
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .timeout(Duration::from_secs(60))
        .json(&serde_json::json!({
            "model": model,
            "stream": true,
            "temperature": 0.25,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        }))
        .send()
        .await?
        .error_for_status()?;

    let mut full = String::new();
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if let Some(json_str) = line.strip_prefix("data: ") {
                if json_str == "[DONE]" {
                    break;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                        full.push_str(delta);
                        emit_delta(stream_ctx, delta, false);
                    }
                }
            }
        }
    }
    emit_delta(stream_ctx, "", true);
    Ok(full)
}

async fn stream_gemini(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> Result<String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse&key={api_key}"
    );
    let resp = http
        .post(&url)
        .timeout(Duration::from_secs(60))
        .json(&serde_json::json!({
            "system_instruction": { "parts": [{ "text": system_prompt }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_prompt }] }]
        }))
        .send()
        .await?
        .error_for_status()?;

    let mut full = String::new();
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(delta) =
                        v["candidates"][0]["content"]["parts"][0]["text"].as_str()
                    {
                        full.push_str(delta);
                        emit_delta(stream_ctx, delta, false);
                    }
                }
            }
        }
    }
    emit_delta(stream_ctx, "", true);
    Ok(full)
}

async fn stream_ollama(
    http: &reqwest::Client,
    ollama_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> Result<String> {
    let url = format!("{}/api/chat", ollama_url.trim_end_matches('/'));
    let resp = http
        .post(&url)
        .timeout(Duration::from_secs(45))
        .json(&serde_json::json!({
            "model": model,
            "stream": true,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        }))
        .send()
        .await?
        .error_for_status()?;

    let mut full = String::new();
    let mut buf = String::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(delta) = v["message"]["content"].as_str() {
                    full.push_str(delta);
                    emit_delta(stream_ctx, delta, false);
                }
            }
        }
    }
    emit_delta(stream_ctx, "", true);
    Ok(full)
}

pub async fn test_provider_connection(
    http: &reqwest::Client,
    provider: &str,
    settings: &AiProviderSettings,
    vault: &SecretVault,
) -> ProviderTestResult {
    let start = std::time::Instant::now();
    let res: Result<String> = match provider {
        "ollama" => {
            let models = list_ollama_models(http, &settings.ollama_url).await;
            if models.is_empty() {
                Err(anyhow::anyhow!(
                    "Ollama nicht erreichbar unter {} (oder keine Modelle installiert).",
                    settings.ollama_url
                ))
            } else {
                Ok(format!(
                    "Verbunden! {} lokale Modelle: {}",
                    models.len(),
                    models.into_iter().take(4).collect::<Vec<_>>().join(", ")
                ))
            }
        }
        "anthropic" => match vault.get_secret(KEY_ANTHROPIC) {
            Some(key) => http
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .timeout(Duration::from_secs(8))
                .send()
                .await
                .map_err(Into::into)
                .and_then(|r| {
                    if r.status().is_success() {
                        Ok("Anthropic Claude API Key verifiziert.".into())
                    } else {
                        bail!("HTTP {}", r.status())
                    }
                }),
            None => Err(anyhow::anyhow!("Kein Anthropic API-Key im Schlüsselbund gespeichert.")),
        },
        "openai" => match vault.get_secret(KEY_OPENAI) {
            Some(key) => http
                .get("https://api.openai.com/v1/models")
                .bearer_auth(key)
                .timeout(Duration::from_secs(8))
                .send()
                .await
                .map_err(Into::into)
                .and_then(|r| {
                    if r.status().is_success() {
                        Ok("OpenAI API Key verifiziert.".into())
                    } else {
                        bail!("HTTP {}", r.status())
                    }
                }),
            None => Err(anyhow::anyhow!("Kein OpenAI API-Key im Schlüsselbund gespeichert.")),
        },
        "gemini" => match vault.get_secret(KEY_GEMINI) {
            Some(key) => http
                .get(format!(
                    "https://generativelanguage.googleapis.com/v1beta/models?key={key}"
                ))
                .timeout(Duration::from_secs(8))
                .send()
                .await
                .map_err(Into::into)
                .and_then(|r| {
                    if r.status().is_success() {
                        Ok("Google Gemini API Key verifiziert.".into())
                    } else {
                        bail!("HTTP {}", r.status())
                    }
                }),
            None => Err(anyhow::anyhow!("Kein Google Gemini API-Key gespeichert.")),
        },
        _ => Err(anyhow::anyhow!("Unbekannter Provider: {provider}")),
    };

    let latency_ms = start.elapsed().as_millis() as u64;
    match res {
        Ok(msg) => ProviderTestResult {
            provider: provider.to_string(),
            ok: true,
            message: msg,
            latency_ms,
        },
        Err(err) => ProviderTestResult {
            provider: provider.to_string(),
            ok: false,
            message: err.to_string(),
            latency_ms,
        },
    }
}
