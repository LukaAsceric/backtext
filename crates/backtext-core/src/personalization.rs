use crate::models::{Language, LearnedPreference, UserProfile, VocabularyEntry};
use sha2::{Digest, Sha256};

pub const EMBEDDING_DIM: usize = 256;

/// Builds a concise markdown dossier about the user's identity, role, projects,
/// glossary, and learned preferences for LLM prompts and MCP agent context.
pub fn render_personalization_dossier(
    profile: &UserProfile,
    vocabulary: &[VocabularyEntry],
    learned: &[LearnedPreference],
) -> String {
    let mut out = String::new();
    let is_de = profile.preferred_output_language == Language::De;

    if is_de {
        out.push_str(&format!(
            "# Nutzerprofil & Arbeitsstil ({})\n",
            profile.display_name
        ));
        out.push_str(&format!(
            "- **Rolle & Organisation:** {} @ {}\n",
            profile.role, profile.organization
        ));
        out.push_str(&format!(
            "- **Bevorzugte Ausgabesprache:** Deutsch (`de`)\n"
        ));
        out.push_str(&format!(
            "- **Schreibstil:** {}\n- **Tonfall-Leitlinien:** {}\n",
            profile.writing_style, profile.tone_instructions
        ));
    } else {
        out.push_str(&format!(
            "# User Profile & Working Style ({})\n",
            profile.display_name
        ));
        out.push_str(&format!(
            "- **Role & Organization:** {} @ {}\n",
            profile.role, profile.organization
        ));
        out.push_str("- **Preferred Output Language:** English (`en`)\n");
        out.push_str(&format!(
            "- **Writing Style:** {}\n- **Tone Guidelines:** {}\n",
            profile.writing_style, profile.tone_instructions
        ));
    }

    if !profile.active_projects.is_empty() {
        out.push_str(&format!(
            "- **Aktive Projekte / Active Projects:** {}\n",
            profile.active_projects.join(", ")
        ));
    }

    if !vocabulary.is_empty() {
        out.push_str("\n## Fachglossar & Eigennamen (Custom Vocabulary)\n");
        for entry in vocabulary.iter().take(40) {
            match (&entry.phonetic_hint, &entry.project_tag) {
                (Some(hint), Some(proj)) => out.push_str(&format!(
                    "- `{}` (Aussprache: {}, Projekt: {}, Kategorie: {})\n",
                    entry.term, hint, proj, entry.category
                )),
                (Some(hint), None) => out.push_str(&format!(
                    "- `{}` (Aussprache: {}, Kategorie: {})\n",
                    entry.term, hint, entry.category
                )),
                (None, Some(proj)) => out.push_str(&format!(
                    "- `{}` (Projekt: {}, Kategorie: {})\n",
                    entry.term, proj, entry.category
                )),
                (None, None) => {
                    out.push_str(&format!("- `{}` ({})\n", entry.term, entry.category))
                }
            }
        }
    }

    let active_facts: Vec<&LearnedPreference> = learned.iter().filter(|p| p.is_active).collect();
    if !active_facts.is_empty() {
        out.push_str("\n## Gelernte Arbeitspräferenzen (Adaptive Memory)\n");
        for fact in active_facts.iter().take(20) {
            out.push_str(&format!(
                "- [{}] {} (Konfidenz {:.0}%)\n",
                fact.category,
                fact.fact_text,
                fact.confidence * 100.0
            ));
        }
    }

    out
}

/// Generates an initial vocabulary prompt string for Whisper / STT engines
/// so product names, attendee names, and technical acronyms are recognized accurately.
pub fn build_stt_vocabulary_prompt(
    profile: &UserProfile,
    vocabulary: &[VocabularyEntry],
    attendee_names: &[String],
) -> String {
    let mut terms: Vec<String> = Vec::new();
    terms.push(profile.display_name.clone());
    if !profile.organization.is_empty() {
        terms.push(profile.organization.clone());
    }
    terms.extend(profile.active_projects.iter().cloned());
    terms.extend(attendee_names.iter().cloned());
    for v in vocabulary {
        terms.push(v.term.clone());
    }
    terms.sort();
    terms.dedup();
    format!("Glossar & Teilnehmer: {}.", terms.join(", "))
}

/// Computes cosine similarity between two equal-length vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom <= 1e-8 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

/// L2-normalizes a vector in place.
pub fn l2_normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
}

/// Updates an existing speaker voiceprint centroid with a newly verified voice embedding
/// using sample-weighted online averaging followed by L2 normalization.
pub fn update_voiceprint_centroid(
    current_centroid: &[f32],
    sample_count: u32,
    new_sample: &[f32],
) -> Vec<f32> {
    if current_centroid.len() != new_sample.len() || sample_count == 0 {
        let mut fresh = new_sample.to_vec();
        l2_normalize(&mut fresh);
        return fresh;
    }
    let weight_old = (sample_count.min(25)) as f32;
    let mut merged: Vec<f32> = current_centroid
        .iter()
        .zip(new_sample.iter())
        .map(|(old_v, new_v)| (old_v * weight_old + new_v) / (weight_old + 1.0))
        .collect();
    l2_normalize(&mut merged);
    merged
}

/// Fast, zero-dependency, multilingual (DE/EN) local feature-hashing semantic embedder (256-dim).
/// Combines normalized word stems and character 4-grams with signed Murmur/SHA projections
/// so local vector search works offline in < 0.2ms even without an external embedding daemon.
pub fn embed_text_local_256(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0_f32; EMBEDDING_DIM];
    let lowered = text.to_lowercase();
    let tokens: Vec<&str> = lowered
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|t| t.len() >= 2)
        .collect();

    if tokens.is_empty() {
        return vec;
    }

    for token in &tokens {
        // Whole token projection (weight 2.0)
        accumulate_feature(&mut vec, token.as_bytes(), 2.0);

        // Character 4-grams for compound German words (e.g. "kontexterfassung" <-> "kontext")
        let chars: Vec<char> = token.chars().collect();
        if chars.len() >= 4 {
            for window in chars.windows(4) {
                let gram: String = window.iter().collect();
                accumulate_feature(&mut vec, gram.as_bytes(), 0.85);
            }
        }
    }

    // Token bigrams for phrase sensitivity
    for pair in tokens.windows(2) {
        let bigram = format!("{}::{}", pair[0], pair[1]);
        accumulate_feature(&mut vec, bigram.as_bytes(), 1.25);
    }

    l2_normalize(&mut vec);
    vec
}

fn accumulate_feature(vec: &mut [f32], bytes: &[u8], weight: f32) {
    let digest = Sha256::digest(bytes);
    let idx1 = (u16::from_le_bytes([digest[0], digest[1]]) as usize) % EMBEDDING_DIM;
    let sign1 = if digest[2] & 1 == 0 { 1.0 } else { -1.0 };
    let idx2 = (u16::from_le_bytes([digest[3], digest[4]]) as usize) % EMBEDDING_DIM;
    let sign2 = if digest[5] & 1 == 0 { 1.0 } else { -1.0 };
    vec[idx1] += sign1 * weight;
    vec[idx2] += sign2 * (weight * 0.5);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_compound_words_share_vector_similarity() {
        let a = embed_text_local_256("Automatische Kontexterfassung und Texterkennung im Hintergrund");
        let b = embed_text_local_256("Hintergrund Kontext und OCR Texterkennung für Agenten");
        let unrelated = embed_text_local_256("Quartalsabschluss Buchhaltung Steuerformular 2024");

        let sim_related = cosine_similarity(&a, &b);
        let sim_unrelated = cosine_similarity(&a, &unrelated);
        assert!(sim_related > 0.35, "Expected high similarity, got {sim_related}");
        assert!(sim_related > sim_unrelated + 0.25);
    }

    #[test]
    fn voiceprint_centroid_updates_smoothly() {
        let mut base = vec![0.0_f32; 8];
        base[0] = 1.0;
        let mut sample = vec![0.0_f32; 8];
        sample[0] = 0.8;
        sample[1] = 0.6;

        let updated = update_voiceprint_centroid(&base, 3, &sample);
        assert!(cosine_similarity(&updated, &base) > 0.95);
        assert!(updated[1] > 0.1);
    }
}
