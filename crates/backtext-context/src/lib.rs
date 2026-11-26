use backtext_core::{
    ClipboardItem, ContextEpisode, ContextSnapshot, ExtractionMethod, PrivacyRule, UserProfile,
    VocabularyEntry,
};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawContextCandidate {
    pub app_bundle_id: String,
    pub app_name: String,
    pub window_title: String,
    pub browser_url: Option<String>,
    pub dwell_ms: u64,
    pub dhash_u64: u64,
    pub visible_text: String,
    pub extraction_method: ExtractionMethod,
    pub is_private_browsing: bool,
    pub is_secure_field_focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum FilterDecision {
    BlockedByPrivacy { reason: String },
    DroppedAsNoise { reason: String },
    MergedDuplicate { previous_snapshot_id: String, added_dwell_ms: u64 },
    Accepted { snapshot: ContextSnapshot },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrubberPreview {
    pub blocked: bool,
    pub block_reason: Option<String>,
    pub scrubbed_text: String,
    pub redacted_secrets_count: u32,
    pub relevance_score: f32,
    pub matched_project: Option<String>,
}

/// Computes the bit Hamming distance between two 64-bit perceptual difference hashes (`dHash`).
pub fn dhash_hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Computes an 8x8 luminance difference hash (`dHash`) from raw grayscale pixels (at least 9x8).
pub fn compute_dhash_from_luma_grid(width: usize, height: usize, luma: &[u8]) -> u64 {
    if width < 9 || height < 8 || luma.len() < width * height {
        return 0;
    }
    let mut hash = 0_u64;
    let mut bit = 0;
    for row in 0..8 {
        let y = row * height / 8;
        for col in 0..8 {
            let x_left = col * width / 9;
            let x_right = (col + 1) * width / 9;
            let left = luma[y * width + x_left];
            let right = luma[y * width + x_right];
            if left > right {
                hash |= 1_u64 << bit;
            }
            bit += 1;
        }
    }
    hash
}

fn secret_regexes() -> &'static Vec<Regex> {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        vec![
            // OpenAI / Anthropic / Stripe / Slack / GitHub tokens
            Regex::new(r"\b(?:sk-[A-Za-z0-9_-]{16,}|sk-ant-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{16,}|AKIA[0-9A-Z]{16})\b").unwrap(),
            // JWT tokens
            Regex::new(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap(),
            // PEM private key blocks
            Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap(),
            // European IBAN
            Regex::new(r"\b[A-Z]{2}\d{2}[ ]?(?:\d{4}[ ]?){3,5}\d{1,4}\b").unwrap(),
        ]
    })
}

fn luhn_valid(digits: &str) -> bool {
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    let mut sum = 0;
    let mut alt = false;
    for ch in digits.chars().rev() {
        let Some(mut d) = ch.to_digit(10) else {
            return false;
        };
        if alt {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        alt = !alt;
    }
    sum % 10 == 0
}

/// Redacts API keys, tokens, PEM keys, IBANs, and Luhn-verified credit card numbers locally.
pub fn scrub_secrets_and_pii(input: &str) -> (String, u32) {
    let mut redacted = 0_u32;
    let mut text = input.to_string();

    for re in secret_regexes() {
        let count = re.find_iter(&text).count() as u32;
        if count > 0 {
            redacted += count;
            text = re.replace_all(&text, "[REDACTED_SECRET]").into_owned();
        }
    }

    // Credit card check with Luhn validation to prevent false positives
    static CC_RE: OnceLock<Regex> = OnceLock::new();
    let cc_re = CC_RE.get_or_init(|| Regex::new(r"\b(?:\d[ -]*?){13,19}\b").unwrap());
    text = cc_re
        .replace_all(&text, |caps: &regex::Captures| {
            let raw = &caps[0];
            let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
            if luhn_valid(&digits) {
                redacted += 1;
                "[REDACTED_CARD]".to_string()
            } else {
                raw.to_string()
            }
        })
        .into_owned();

    (text, redacted)
}

fn wildcard_or_list_match(pattern_list: &str, target: &str) -> bool {
    let target_low = target.to_lowercase();
    for raw_part in pattern_list.split(|c| c == ',' || c == '|') {
        let p = raw_part.trim().to_lowercase();
        if p.is_empty() {
            continue;
        }
        let cleaned = p.trim_matches('*');
        if !cleaned.is_empty() && target_low.contains(cleaned) {
            return true;
        }
    }
    false
}

pub fn check_privacy_gate(
    candidate: &RawContextCandidate,
    rules: &[PrivacyRule],
) -> Option<String> {
    if candidate.is_private_browsing {
        return Some("Privates / Inkognito-Browserfenster erkannt".into());
    }
    if candidate.is_secure_field_focused {
        return Some("Sicheres Passwortfeld (AXSecureTextField) fokussiert".into());
    }

    for rule in rules.iter().filter(|r| r.is_enabled) {
        match rule.kind.as_str() {
            "app_bundle" => {
                if wildcard_or_list_match(&rule.pattern, &candidate.app_bundle_id)
                    || wildcard_or_list_match(&rule.pattern, &candidate.app_name)
                {
                    return Some(format!("Blockierte App-Regel: {}", rule.description));
                }
            }
            "domain_pattern" => {
                if let Some(ref url) = candidate.browser_url {
                    if wildcard_or_list_match(&rule.pattern, url) {
                        return Some(format!("Blockierte Domain-Regel: {}", rule.description));
                    }
                }
            }
            "window_title" => {
                if wildcard_or_list_match(&rule.pattern, &candidate.window_title) {
                    return Some(format!("Blockierter Fenstertitel: {}", rule.description));
                }
            }
            _ => {}
        }
    }
    None
}

fn token_jaccard_similarity(a: &str, b: &str) -> f32 {
    let set_a: HashSet<&str> = a
        .split_whitespace()
        .filter(|t| t.len() >= 3)
        .take(200)
        .collect();
    let set_b: HashSet<&str> = b
        .split_whitespace()
        .filter(|t| t.len() >= 3)
        .take(200)
        .collect();
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let inter = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

pub fn score_relevance_and_project(
    app_name: &str,
    window_title: &str,
    browser_url: Option<&str>,
    scrubbed_text: &str,
    dwell_ms: u64,
    profile: &UserProfile,
    vocabulary: &[VocabularyEntry],
) -> (f32, Option<String>) {
    let combined_lower = format!(
        "{} {} {} {}",
        app_name,
        window_title,
        browser_url.unwrap_or(""),
        scrubbed_text
    )
    .to_lowercase();

    let unique_words: HashSet<&str> = scrubbed_text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .collect();

    // Base score from information density (include title + URL + visible text)
    let title_words: usize = window_title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .count();
    let effective_words = unique_words.len().max(title_words);
    let density_score = ((effective_words as f32) / 50.0).clamp(0.24, 0.55);

    // Productive app / browser URL bonus so normal desktop work windows aren't dropped on first 4s tick
    let app_low = app_name.to_lowercase();
    let productive_bonus = if browser_url.is_some()
        || [
            "code", "cursor", "xcode", "terminal", "iterm", "warp", "ghostty", "chrome",
            "safari", "arc", "brave", "edge", "firefox", "dia", "slack", "teams", "zoom",
            "notion", "obsidian", "linear", "figma", "mail", "notes", "notizen", "word",
            "excel", "powerpoint", "pages", "keynote", "numbers", "outlook", "calendar",
            "kalender", "github", "zed", "intellij", "postman",
        ]
        .iter()
        .any(|k| app_low.contains(k))
    {
        0.12_f32
    } else {
        0.04_f32
    };

    // Dwell duration bonus (up to +0.20)
    let dwell_bonus = ((dwell_ms as f32) / 150_000.0).clamp(0.0, 0.20);

    let mut matched_project: Option<String> = None;
    let mut domain_bonus = 0.0_f32;

    for proj in &profile.active_projects {
        if !proj.is_empty() && combined_lower.contains(&proj.to_lowercase()) {
            matched_project = Some(proj.clone());
            domain_bonus += 0.18;
            break;
        }
    }

    for voc in vocabulary {
        if combined_lower.contains(&voc.term.to_lowercase()) {
            domain_bonus += 0.06;
            if matched_project.is_none() {
                matched_project = voc.project_tag.clone();
            }
        }
    }

    let score = (density_score + productive_bonus + dwell_bonus + domain_bonus.min(0.30))
        .clamp(0.10, 0.99);
    (score, matched_project)
}

/// Runs the full 3-stage context pipeline (Privacy Gate -> Dedup & Noise -> Relevance Scoring).
pub fn evaluate_context_candidate(
    candidate: RawContextCandidate,
    previous: Option<&ContextSnapshot>,
    rules: &[PrivacyRule],
    profile: &UserProfile,
    vocabulary: &[VocabularyEntry],
) -> FilterDecision {
    // Stage 1: Privacy & Security Gate
    if let Some(reason) = check_privacy_gate(&candidate, rules) {
        return FilterDecision::BlockedByPrivacy { reason };
    }

    let (scrubbed_text, redacted_count) = scrub_secrets_and_pii(&candidate.visible_text);

    // Stage 2: Noise & Transient Switch Filter
    if candidate.dwell_ms < 3_000 {
        return FilterDecision::DroppedAsNoise {
            reason: format!(
                "Kurzer Fokuswechsel ({} ms < 3000 ms Mindestverweildauer)",
                candidate.dwell_ms
            ),
        };
    }

    let alpha_chars = scrubbed_text.chars().filter(|c| c.is_alphanumeric()).count();
    let has_meaningful_title = candidate.window_title.trim().len() >= 4;
    if alpha_chars < 10 && !has_meaningful_title && candidate.browser_url.is_none() {
        return FilterDecision::DroppedAsNoise {
            reason: "Zu geringe Informationsdichte (leeres Fenster / System-UI)".into(),
        };
    }

    let dhash_hex = format!("{:016x}", candidate.dhash_u64);
    if let Some(prev) = previous {
        if prev.app_bundle_id == candidate.app_bundle_id && prev.window_title == candidate.window_title {
            let prev_dhash = u64::from_str_radix(&prev.dhash_hex, 16).unwrap_or(0);
            let visual_dist = dhash_hamming_distance(prev_dhash, candidate.dhash_u64);
            let text_sim = token_jaccard_similarity(&prev.extracted_text, &scrubbed_text);
            if visual_dist < 5 || text_sim > 0.90 {
                return FilterDecision::MergedDuplicate {
                    previous_snapshot_id: prev.id.clone(),
                    added_dwell_ms: candidate.dwell_ms,
                };
            }
        }
    }

    // Stage 3: Semantic Relevance Scoring & Project Attribution
    let (relevance_score, project_tag) = score_relevance_and_project(
        &candidate.app_name,
        &candidate.window_title,
        candidate.browser_url.as_deref(),
        &scrubbed_text,
        candidate.dwell_ms,
        profile,
        vocabulary,
    );

    if relevance_score < 0.20 {
        return FilterDecision::DroppedAsNoise {
            reason: format!("Relevanz-Score zu niedrig ({relevance_score:.2})"),
        };
    }

    FilterDecision::Accepted {
        snapshot: ContextSnapshot {
            id: format!("snap_{}", uuid::Uuid::new_v4().simple()),
            captured_at: Utc::now(),
            app_bundle_id: candidate.app_bundle_id,
            app_name: candidate.app_name,
            window_title: candidate.window_title,
            browser_url: candidate.browser_url,
            dwell_ms: candidate.dwell_ms,
            dhash_hex,
            extracted_text: scrubbed_text,
            extraction_method: candidate.extraction_method,
            relevance_score,
            project_tag,
            redacted_secrets_count: redacted_count,
            episode_id: None,
        },
    }
}

/// Interactive preview helper for testing the Privacy Gate & Secret Scrubber from the UI.
pub fn preview_filter_on_sample(
    app_name: &str,
    window_title: &str,
    browser_url: Option<&str>,
    raw_text: &str,
    rules: &[PrivacyRule],
    profile: &UserProfile,
    vocabulary: &[VocabularyEntry],
) -> ScrubberPreview {
    let candidate = RawContextCandidate {
        app_bundle_id: app_name.to_lowercase(),
        app_name: app_name.to_string(),
        window_title: window_title.to_string(),
        browser_url: browser_url.map(|s| s.to_string()),
        dwell_ms: 15_000,
        dhash_u64: 0x9988776655443322,
        visible_text: raw_text.to_string(),
        extraction_method: ExtractionMethod::Accessibility,
        is_private_browsing: false,
        is_secure_field_focused: false,
    };
    let block_reason = check_privacy_gate(&candidate, rules);
    let (scrubbed_text, redacted_secrets_count) = scrub_secrets_and_pii(raw_text);
    let (relevance_score, matched_project) = score_relevance_and_project(
        app_name,
        window_title,
        browser_url,
        &scrubbed_text,
        15_000,
        profile,
        vocabulary,
    );
    ScrubberPreview {
        blocked: block_reason.is_some(),
        block_reason,
        scrubbed_text,
        redacted_secrets_count,
        relevance_score,
        matched_project,
    }
}

/// Evaluates safe clipboard capture (ignoring concealed password manager types and duplicates).
pub fn evaluate_clipboard_capture(
    source_app: &str,
    raw_text: &str,
    is_concealed: bool,
    last_hash: Option<&str>,
    rules: &[PrivacyRule],
    profile: &UserProfile,
    vocabulary: &[VocabularyEntry],
) -> Option<ClipboardItem> {
    if is_concealed || raw_text.trim().len() < 4 {
        return None;
    }
    for rule in rules.iter().filter(|r| r.is_enabled && r.kind == "app_bundle") {
        if wildcard_or_list_match(&rule.pattern, source_app) {
            return None;
        }
    }

    let (scrubbed, _) = scrub_secrets_and_pii(raw_text.trim());
    if scrubbed == "[REDACTED_SECRET]" || scrubbed == "[REDACTED_CARD]" {
        return None;
    }

    let hash = hex::encode(&Sha256::digest(scrubbed.as_bytes())[..8]);
    if last_hash.map(|h| h == hash).unwrap_or(false) {
        return None;
    }

    let kind = if scrubbed.starts_with("http://") || scrubbed.starts_with("https://") {
        "url"
    } else if scrubbed.contains("fn ")
        || scrubbed.contains("const ")
        || scrubbed.contains('{')
        || scrubbed.contains("SELECT ")
    {
        "code_snippet"
    } else {
        "text"
    };

    let (score, project_tag) =
        score_relevance_and_project(source_app, "Clipboard", None, &scrubbed, 10_000, profile, vocabulary);

    Some(ClipboardItem {
        id: format!("clip_{}", uuid::Uuid::new_v4().simple()),
        captured_at: Utc::now(),
        source_app: source_app.to_string(),
        content_kind: kind.to_string(),
        text_preview: scrubbed.chars().take(480).collect(),
        content_hash: hash,
        relevance_score: score,
        project_tag,
    })
}

/// Compacts a slice of recent snapshots into a coherent ContextEpisode summary.
pub fn compact_snapshots_to_episode(snapshots: &[ContextSnapshot]) -> Option<ContextEpisode> {
    if snapshots.is_empty() {
        return None;
    }
    let started_at = snapshots
        .iter()
        .map(|s| s.captured_at)
        .min()
        .unwrap_or_else(Utc::now);
    let ended_at = snapshots
        .iter()
        .map(|s| s.captured_at)
        .max()
        .unwrap_or_else(Utc::now);

    let mut apps: Vec<String> = Vec::new();
    let mut urls: Vec<String> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    let mut project_tag: Option<String> = None;
    let mut score_sum = 0.0_f32;

    for s in snapshots {
        if !apps.contains(&s.app_name) {
            apps.push(s.app_name.clone());
        }
        if let Some(ref u) = s.browser_url {
            if !urls.contains(u) {
                urls.push(u.clone());
            }
        }
        if !titles.contains(&s.window_title) && titles.len() < 4 {
            titles.push(s.window_title.clone());
        }
        if project_tag.is_none() && s.project_tag.is_some() {
            project_tag = s.project_tag.clone();
        }
        score_sum += s.relevance_score;
    }

    let avg_score = (score_sum / (snapshots.len() as f32)).clamp(0.2, 0.99);
    let title = match &project_tag {
        Some(proj) => format!("{proj}: Fokus in {}", apps.join(" & ")),
        None => format!("Arbeitsphase in {}", apps.join(", ")),
    };
    let summary = format!(
        "Aktive Themen: {}. Erfasste Quellen ({} Snapshots) über {}.",
        titles.join(" · "),
        snapshots.len(),
        apps.join(", ")
    );

    Some(ContextEpisode {
        id: format!("ep_{}", uuid::Uuid::new_v4().simple()),
        started_at,
        ended_at,
        title,
        summary,
        primary_apps: apps,
        urls,
        project_tag,
        importance_score: avg_score,
        snapshot_count: snapshots.len() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_gate_blocks_1password_and_scrubs_api_keys() {
        let rules = vec![PrivacyRule {
            id: "r1".into(),
            kind: "app_bundle".into(),
            pattern: "com.1password.1password".into(),
            description: "1Password".into(),
            is_enabled: true,
            is_builtin: true,
        }];
        let profile = UserProfile::default();

        let blocked = evaluate_context_candidate(
            RawContextCandidate {
                app_bundle_id: "com.1password.1password".into(),
                app_name: "1Password".into(),
                window_title: "Tresor".into(),
                browser_url: None,
                dwell_ms: 10_000,
                dhash_u64: 1234,
                visible_text: "Master Password".into(),
                extraction_method: ExtractionMethod::Accessibility,
                is_private_browsing: false,
                is_secure_field_focused: false,
            },
            None,
            &rules,
            &profile,
            &[],
        );
        assert!(matches!(blocked, FilterDecision::BlockedByPrivacy { .. }));

        let (scrubbed, count) = scrub_secrets_and_pii(
            "Testing token sk-ant-api03-1234567890abcdefghij and ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ1234 in config",
        );
        assert_eq!(count, 2);
        assert!(!scrubbed.contains("sk-ant-"));
        assert!(!scrubbed.contains("ghp_"));
        assert!(scrubbed.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn deduplicates_identical_window_snapshots() {
        let profile = UserProfile::default();
        let first = evaluate_context_candidate(
            RawContextCandidate {
                app_bundle_id: "com.cursor".into(),
                app_name: "Cursor".into(),
                window_title: "backtext — server.rs".into(),
                browser_url: None,
                dwell_ms: 8_000,
                dhash_u64: 0xAABBCCDDEEFF0011,
                visible_text: "Implementing Model Context Protocol tools for backtext agent memory layer".into(),
                extraction_method: ExtractionMethod::Accessibility,
                is_private_browsing: false,
                is_secure_field_focused: false,
            },
            None,
            &[],
            &profile,
            &[],
        );
        let FilterDecision::Accepted { snapshot } = first else {
            panic!("Expected first snapshot to be accepted");
        };

        let second = evaluate_context_candidate(
            RawContextCandidate {
                app_bundle_id: "com.cursor".into(),
                app_name: "Cursor".into(),
                window_title: "backtext — server.rs".into(),
                browser_url: None,
                dwell_ms: 5_000,
                dhash_u64: 0xAABBCCDDEEFF0013, // only 1 bit diff!
                visible_text: "Implementing Model Context Protocol tools for backtext agent memory layer".into(),
                extraction_method: ExtractionMethod::Accessibility,
                is_private_browsing: false,
                is_secure_field_focused: false,
            },
            Some(&snapshot),
            &[],
            &profile,
            &[],
        );
        assert!(matches!(second, FilterDecision::MergedDuplicate { .. }));
    }
}
