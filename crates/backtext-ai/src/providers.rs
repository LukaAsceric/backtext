use anyhow::{bail, Result};
use backtext_core::{
    embed_text_local_256, l2_normalize, AiProviderSettings, BacktextEvent, EventBus, EMBEDDING_DIM,
};
use backtext_storage::secrets::{
    SecretVault, KEY_ANTHROPIC, KEY_DEEPGRAM, KEY_GEMINI, KEY_GROQ, KEY_OPENAI, KEY_OPENROUTER,
    KEY_PRIMALABS,
};
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

#[derive(Debug, Clone)]
pub struct StreamCompletionOutcome {
    pub result: Option<(String, String)>,
    pub last_error: Option<String>,
}

pub const PRIMALABS_BASE_URL: &str = "https://api.primalabs.ai/v1";
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

pub fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514",
        "openai" => "gpt-4o-mini",
        "gemini" => "gemini-2.5-flash",
        "openrouter" => "anthropic/claude-sonnet-4",
        "primalabs" => "primalabs-ai/DeepSeek-V4.1-Flash-1017",
        "local_ollama" | "ollama" => "qwen3:8b",
        _ => "claude-sonnet-4-20250514",
    }
}

pub fn should_omit_temperature(provider: &str, model: &str) -> bool {
    let low = model.to_lowercase();
    provider == "primalabs"
        || low.starts_with("o1")
        || low.starts_with("o3")
        || low.starts_with("o4")
        || low.contains("kimi-k3")
        || low.contains("mimo-v2")
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

/// Lists curated and (when available) live-fetched models for any configured LLM provider.
pub async fn list_provider_models(
    http: &reqwest::Client,
    provider: &str,
    settings: &AiProviderSettings,
    vault: &SecretVault,
) -> Vec<String> {
    match provider {
        "ollama" | "local_ollama" => list_ollama_models(http, &settings.ollama_url).await,
        "primalabs" => {
            let fallback = vec![
                "primalabs-ai/DeepSeek-V4.1-Flash-1017".to_string(),
                "primalabs-ai/Kimi-K3.1-Pro".to_string(),
                "primalabs-ai/MiMo-V2.6-Pro".to_string(),
                "primalabs-ai/GLM-5.3-Flash".to_string(),
                "primalabs-ai/Nemotron-3.5-Lightning-NVFP4".to_string(),
                "primalabs-ai/Nemotron-3.5-Super-VL-NVFP4".to_string(),
            ];
            let Some(key) = vault.get_secret(KEY_PRIMALABS) else {
                return fallback;
            };
            let Ok(resp) = http
                .get(format!("{PRIMALABS_BASE_URL}/models"))
                .bearer_auth(key)
                .timeout(Duration::from_secs(6))
                .send()
                .await
            else {
                return fallback;
            };
            if !resp.status().is_success() {
                return fallback;
            }
            let Ok(v) = resp.json::<serde_json::Value>().await else {
                return fallback;
            };
            let live: Vec<String> = v["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if live.is_empty() {
                fallback
            } else {
                live
            }
        }
        "openrouter" => {
            let mut curated = vec![
                "anthropic/claude-sonnet-4".to_string(),
                "anthropic/claude-3.7-sonnet".to_string(),
                "openai/gpt-4o-mini".to_string(),
                "openai/gpt-4.1".to_string(),
                "google/gemini-2.5-flash".to_string(),
                "google/gemini-2.5-pro".to_string(),
                "deepseek/deepseek-chat-v3-0324".to_string(),
                "moonshotai/kimi-k3.1-pro".to_string(),
                "openrouter/auto".to_string(),
            ];
            if let Some(key) = vault.get_secret(KEY_OPENROUTER) {
                if let Ok(resp) = http
                    .get(format!("{OPENROUTER_BASE_URL}/models"))
                    .bearer_auth(key)
                    .timeout(Duration::from_secs(6))
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(v) = resp.json::<serde_json::Value>().await {
                            if let Some(arr) = v["data"].as_array() {
                                for m in arr.iter().take(40) {
                                    if let Some(id) = m["id"].as_str() {
                                        if !curated.iter().any(|c| c == id) {
                                            curated.push(id.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            curated
        }
        "anthropic" => vec![
            "claude-sonnet-4-20250514".into(),
            "claude-3-7-sonnet-latest".into(),
            "claude-3-5-haiku-latest".into(),
        ],
        "openai" => vec![
            "gpt-4o-mini".into(),
            "gpt-4.1".into(),
            "gpt-4.1-mini".into(),
            "o4-mini".into(),
        ],
        "gemini" => vec![
            "gemini-2.5-flash".into(),
            "gemini-2.5-pro".into(),
            "gemini-2.0-flash".into(),
        ],
        _ => vec![
            "claude-sonnet-4-20250514".into(),
            "anthropic/claude-sonnet-4".into(),
            "primalabs-ai/DeepSeek-V4.1-Flash-1017".into(),
            "gpt-4o-mini".into(),
            "gemini-2.5-flash".into(),
        ],
    }
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

fn select_model_for_provider<'a>(provider: &str, settings: &'a AiProviderSettings) -> &'a str {
    let configured = settings.llm_model.trim();
    if configured.is_empty() {
        return default_model_for_provider(provider);
    }
    let explicit = settings.llm_provider == provider
        || (provider == "ollama" && settings.llm_provider == "local_ollama");
    if explicit {
        return configured;
    }

    match provider {
        "anthropic" if configured.starts_with("claude") => configured,
        "openai"
            if configured.starts_with("gpt")
                || configured.starts_with("o1")
                || configured.starts_with("o3")
                || configured.starts_with("o4") =>
        {
            configured
        }
        "gemini" if configured.starts_with("gemini") => configured,
        "openrouter" if configured.contains('/') && !configured.starts_with("primalabs-ai/") => {
            configured
        }
        "primalabs" if configured.starts_with("primalabs-ai/") => configured,
        _ => default_model_for_provider(provider),
    }
}

/// Streams a completion across Anthropic Claude, OpenRouter, PrimaLabs, OpenAI, Google Gemini,
/// or Local Ollama, emitting real-time `ChatStreamDelta` events over the `EventBus`.
pub async fn stream_completion(
    http: &reqwest::Client,
    settings: &AiProviderSettings,
    vault: &SecretVault,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> StreamCompletionOutcome {
    let provider_order: Vec<&str> = match settings.llm_provider.as_str() {
        "anthropic" => vec!["anthropic"],
        "openrouter" => vec!["openrouter"],
        "primalabs" => vec!["primalabs"],
        "openai" => vec!["openai"],
        "gemini" => vec!["gemini"],
        "local_ollama" => vec!["ollama"],
        _ => vec![
            "anthropic",
            "openrouter",
            "primalabs",
            "openai",
            "gemini",
            "ollama",
        ], // hybrid_auto
    };

    let explicit_mode = settings.llm_provider != "hybrid_auto";
    let mut last_error: Option<String> = None;

    for provider in provider_order {
        match provider {
            "anthropic" => match vault.get_secret(KEY_ANTHROPIC) {
                Some(key) => {
                    let model = select_model_for_provider("anthropic", settings);
                    match stream_anthropic(
                        http,
                        &key,
                        model,
                        system_prompt,
                        user_prompt,
                        stream_ctx,
                    )
                    .await
                    {
                        Ok(text) if !text.trim().is_empty() => {
                            return StreamCompletionOutcome {
                                result: Some((text, format!("anthropic:{model}"))),
                                last_error: None,
                            };
                        }
                        Ok(_) => last_error = Some("Anthropic lieferte eine leere Antwort.".into()),
                        Err(e) => last_error = Some(format!("Anthropic ({model}): {e}")),
                    }
                }
                None if explicit_mode => {
                    last_error = Some("Kein Anthropic API-Key in den Einstellungen hinterlegt.".into());
                }
                None => {}
            },
            "openrouter" => match vault.get_secret(KEY_OPENROUTER) {
                Some(key) => {
                    let model = select_model_for_provider("openrouter", settings);
                    match stream_openai_compatible(
                        http,
                        &format!("{OPENROUTER_BASE_URL}/chat/completions"),
                        &key,
                        model,
                        &[
                            ("HTTP-Referer", "https://backtext.ai"),
                            ("X-Title", "backtext"),
                        ],
                        should_omit_temperature("openrouter", model),
                        system_prompt,
                        user_prompt,
                        stream_ctx,
                    )
                    .await
                    {
                        Ok(text) if !text.trim().is_empty() => {
                            return StreamCompletionOutcome {
                                result: Some((text, format!("openrouter:{model}"))),
                                last_error: None,
                            };
                        }
                        Ok(_) => last_error = Some("OpenRouter lieferte eine leere Antwort.".into()),
                        Err(e) => last_error = Some(format!("OpenRouter ({model}): {e}")),
                    }
                }
                None if explicit_mode => {
                    last_error = Some("Kein OpenRouter API-Key in den Einstellungen hinterlegt.".into());
                }
                None => {}
            },
            "primalabs" => match vault.get_secret(KEY_PRIMALABS) {
                Some(key) => {
                    let model = select_model_for_provider("primalabs", settings);
                    match stream_openai_compatible(
                        http,
                        &format!("{PRIMALABS_BASE_URL}/chat/completions"),
                        &key,
                        model,
                        &[],
                        should_omit_temperature("primalabs", model),
                        system_prompt,
                        user_prompt,
                        stream_ctx,
                    )
                    .await
                    {
                        Ok(text) if !text.trim().is_empty() => {
                            return StreamCompletionOutcome {
                                result: Some((text, format!("primalabs:{model}"))),
                                last_error: None,
                            };
                        }
                        Ok(_) => last_error = Some("PrimaLabs lieferte eine leere Antwort.".into()),
                        Err(e) => last_error = Some(format!("PrimaLabs ({model}): {e}")),
                    }
                }
                None if explicit_mode => {
                    last_error = Some("Kein PrimaLabs API-Key in den Einstellungen hinterlegt.".into());
                }
                None => {}
            },
            "openai" => match vault.get_secret(KEY_OPENAI) {
                Some(key) => {
                    let model = select_model_for_provider("openai", settings);
                    match stream_openai_compatible(
                        http,
                        "https://api.openai.com/v1/chat/completions",
                        &key,
                        model,
                        &[],
                        should_omit_temperature("openai", model),
                        system_prompt,
                        user_prompt,
                        stream_ctx,
                    )
                    .await
                    {
                        Ok(text) if !text.trim().is_empty() => {
                            return StreamCompletionOutcome {
                                result: Some((text, format!("openai:{model}"))),
                                last_error: None,
                            };
                        }
                        Ok(_) => last_error = Some("OpenAI lieferte eine leere Antwort.".into()),
                        Err(e) => last_error = Some(format!("OpenAI ({model}): {e}")),
                    }
                }
                None if explicit_mode => {
                    last_error = Some("Kein OpenAI API-Key in den Einstellungen hinterlegt.".into());
                }
                None => {}
            },
            "gemini" => match vault.get_secret(KEY_GEMINI) {
                Some(key) => {
                    let model = select_model_for_provider("gemini", settings);
                    match stream_gemini(http, &key, model, system_prompt, user_prompt, stream_ctx)
                        .await
                    {
                        Ok(text) if !text.trim().is_empty() => {
                            return StreamCompletionOutcome {
                                result: Some((text, format!("gemini:{model}"))),
                                last_error: None,
                            };
                        }
                        Ok(_) => last_error = Some("Gemini lieferte eine leere Antwort.".into()),
                        Err(e) => last_error = Some(format!("Gemini ({model}): {e}")),
                    }
                }
                None if explicit_mode => {
                    last_error = Some("Kein Google Gemini API-Key in den Einstellungen hinterlegt.".into());
                }
                None => {}
            },
            "ollama" => {
                let model = select_model_for_provider("ollama", settings);
                match stream_ollama(
                    http,
                    &settings.ollama_url,
                    model,
                    system_prompt,
                    user_prompt,
                    stream_ctx,
                )
                .await
                {
                    Ok(text) if !text.trim().is_empty() => {
                        return StreamCompletionOutcome {
                            result: Some((text, format!("ollama:{model}"))),
                            last_error: None,
                        };
                    }
                    Ok(_) => {
                        if explicit_mode {
                            last_error = Some("Ollama lieferte eine leere Antwort.".into());
                        }
                    }
                    Err(e) => {
                        if explicit_mode {
                            last_error = Some(format!("Ollama ({model}): {e}"));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    StreamCompletionOutcome {
        result: None,
        last_error,
    }
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

async fn check_http_status(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let snippet: String = body.chars().take(180).collect();
    if snippet.trim().is_empty() {
        bail!("HTTP {status}");
    }
    bail!("HTTP {status}: {snippet}");
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
        .await?;
    let resp = check_http_status(resp).await?;

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

#[allow(clippy::too_many_arguments)]
async fn stream_openai_compatible(
    http: &reqwest::Client,
    endpoint_url: &str,
    api_key: &str,
    model: &str,
    extra_headers: &[(&str, &str)],
    omit_temperature: bool,
    system_prompt: &str,
    user_prompt: &str,
    stream_ctx: Option<(&EventBus, &str, &str)>,
) -> Result<String> {
    let mut body = serde_json::json!({
        "model": model,
        "stream": true,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ]
    });
    if !omit_temperature {
        body["temperature"] = serde_json::json!(0.25);
    }

    let mut req = http
        .post(endpoint_url)
        .bearer_auth(api_key)
        .timeout(Duration::from_secs(75))
        .json(&body);

    for (h_key, h_val) in extra_headers {
        req = req.header(*h_key, *h_val);
    }

    let resp = check_http_status(req.send().await?).await?;

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
                    // Only stream visible assistant answer tokens (`delta.content`),
                    // skipping reasoning traces (`delta.reasoning_content`) on PrimaLabs/OpenRouter.
                    if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                        if !delta.is_empty() {
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
        .await?;
    let resp = check_http_status(resp).await?;

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
        .await?;
    let resp = check_http_status(resp).await?;

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
        "openrouter" => match vault.get_secret(KEY_OPENROUTER) {
            Some(key) => {
                let r = http
                    .get(format!("{OPENROUTER_BASE_URL}/auth/key"))
                    .bearer_auth(key)
                    .timeout(Duration::from_secs(8))
                    .send()
                    .await;
                match r {
                    Ok(resp) if resp.status().is_success() => {
                        let v = resp.json::<serde_json::Value>().await.unwrap_or_default();
                        let label = v["data"]["label"].as_str().unwrap_or("aktiv");
                        Ok(format!("OpenRouter API-Key verifiziert ({label})."))
                    }
                    Ok(resp) => Err(anyhow::anyhow!("HTTP {}", resp.status())),
                    Err(e) => Err(e.into()),
                }
            }
            None => Err(anyhow::anyhow!(
                "Kein OpenRouter API-Key im Schlüsselbund gespeichert."
            )),
        },
        "primalabs" => match vault.get_secret(KEY_PRIMALABS) {
            Some(key) => {
                let r = http
                    .get(format!("{PRIMALABS_BASE_URL}/models"))
                    .bearer_auth(key)
                    .timeout(Duration::from_secs(8))
                    .send()
                    .await;
                match r {
                    Ok(resp) if resp.status().is_success() => {
                        let v = resp.json::<serde_json::Value>().await.unwrap_or_default();
                        let count = v["data"].as_array().map(|a| a.len()).unwrap_or(0);
                        Ok(format!(
                            "PrimaLabs API-Key verifiziert ({count} Serverless-Modelle verfügbar, Zero Data Retention)."
                        ))
                    }
                    Ok(resp) => Err(anyhow::anyhow!("HTTP {}", resp.status())),
                    Err(e) => Err(e.into()),
                }
            }
            None => Err(anyhow::anyhow!(
                "Kein PrimaLabs API-Key im Schlüsselbund gespeichert."
            )),
        },
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
            None => Err(anyhow::anyhow!(
                "Kein Anthropic API-Key im Schlüsselbund gespeichert."
            )),
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
            None => Err(anyhow::anyhow!(
                "Kein OpenAI API-Key im Schlüsselbund gespeichert."
            )),
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
        "deepgram" => match vault.get_secret(KEY_DEEPGRAM) {
            Some(key) => http
                .get("https://api.deepgram.com/v1/projects")
                .header("Authorization", format!("Token {key}"))
                .timeout(Duration::from_secs(8))
                .send()
                .await
                .map_err(Into::into)
                .and_then(|r| {
                    if r.status().is_success() {
                        Ok("Deepgram Nova-3 STT Key verifiziert.".into())
                    } else {
                        bail!("HTTP {}", r.status())
                    }
                }),
            None => Err(anyhow::anyhow!("Kein Deepgram API-Key gespeichert.")),
        },
        "groq" => match vault.get_secret(KEY_GROQ) {
            Some(key) => http
                .get("https://api.groq.com/openai/v1/models")
                .bearer_auth(key)
                .timeout(Duration::from_secs(8))
                .send()
                .await
                .map_err(Into::into)
                .and_then(|r| {
                    if r.status().is_success() {
                        Ok("Groq Whisper API Key verifiziert.".into())
                    } else {
                        bail!("HTTP {}", r.status())
                    }
                }),
            None => Err(anyhow::anyhow!("Kein Groq API-Key gespeichert.")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primalabs_and_reasoning_models_omit_fixed_temperature() {
        assert!(should_omit_temperature(
            "primalabs",
            "primalabs-ai/Kimi-K3.1-Pro"
        ));
        assert!(should_omit_temperature(
            "primalabs",
            "primalabs-ai/DeepSeek-V4.1-Flash-1017"
        ));
        assert!(should_omit_temperature("openai", "o4-mini"));
        assert!(!should_omit_temperature("openai", "gpt-4o-mini"));
        assert!(!should_omit_temperature(
            "openrouter",
            "anthropic/claude-sonnet-4"
        ));
    }

    #[test]
    fn provider_model_selection_respects_explicit_and_hybrid_routing() {
        let mut cfg = AiProviderSettings::default();
        cfg.llm_provider = "primalabs".into();
        cfg.llm_model = "primalabs-ai/Kimi-K3.1-Pro".into();
        assert_eq!(
            select_model_for_provider("primalabs", &cfg),
            "primalabs-ai/Kimi-K3.1-Pro"
        );

        cfg.llm_provider = "hybrid_auto".into();
        cfg.llm_model = "claude-sonnet-4-20250514".into();
        assert_eq!(
            select_model_for_provider("primalabs", &cfg),
            "primalabs-ai/DeepSeek-V4.1-Flash-1017"
        );
        assert_eq!(
            select_model_for_provider("openrouter", &cfg),
            "anthropic/claude-sonnet-4"
        );
    }
}
