pub mod secrets;
pub mod seed;

use anyhow::{Context, Result};
use backtext_core::*;
use chrono::{DateTime, Duration, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use secrets::SecretVault;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SCHEMA_SQL: &str = include_str!("../migrations/0001_initial_schema.sql");
const SQLITE_PLAINTEXT_HEADER: &[u8; 16] = b"SQLite format 3\0";

#[derive(Clone)]
pub struct StorageEngine {
    db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
    secrets: SecretVault,
    db_encrypted: bool,
}

fn encode_f32_vec(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

fn decode_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn sanitize_fts_query(raw: &str) -> String {
    let tokens: Vec<String> = raw
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|t| t.len() >= 2)
        .take(12)
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect();
    tokens.join(" OR ")
}

fn file_is_plaintext_sqlite(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 16];
    matches!(f.read_exact(&mut header), Ok(())) && &header == SQLITE_PLAINTEXT_HEADER
}

fn migrate_plaintext_to_sqlcipher(db_path: &Path, key_hex: &str) -> Result<()> {
    let migrating_path = db_path.with_extension("db.enc.migrating");
    let _ = std::fs::remove_file(&migrating_path);

    let plain_conn = Connection::open(db_path)?;
    let attach_sql = format!(
        "ATTACH DATABASE '{}' AS encrypted KEY \"x'{key_hex}'\";
         SELECT sqlcipher_export('encrypted');
         DETACH DATABASE encrypted;",
        migrating_path.display().to_string().replace('\'', "''")
    );
    plain_conn.execute_batch(&attach_sql)?;
    drop(plain_conn);

    // Clean up any leftover WAL/SHM files from unencrypted v0.1.0 DB
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    std::fs::rename(&migrating_path, db_path)?;
    Ok(())
}

impl StorageEngine {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        let vault_dir = db_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        std::fs::create_dir_all(&vault_dir)
            .with_context(|| format!("Failed to create storage directory {:?}", vault_dir))?;

        let secrets = SecretVault::open(&vault_dir);
        let key_hex = secrets.get_or_create_db_master_key_hex()?;

        // Auto-migrate legacy unencrypted v0.1.0 SQLite DB in place to AES-256 SQLCipher
        if db_path.exists() && file_is_plaintext_sqlite(&db_path) {
            let _ = migrate_plaintext_to_sqlcipher(&db_path, &key_hex);
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open SQLite database at {:?}", db_path))?;

        let db_encrypted = conn
            .execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))
            .is_ok();

        // Verify readability (and check cipher_version)
        let cipher_active: bool = db_encrypted
            && conn
                .query_row("PRAGMA cipher_version;", [], |r| r.get::<_, String>(0))
                .map(|v| !v.is_empty())
                .unwrap_or(false);

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA secure_delete = ON;",
        )?;
        conn.execute_batch(SCHEMA_SQL)?;

        let engine = Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
            secrets,
            db_encrypted: cipher_active,
        };
        engine.ensure_defaults_seeded()?;
        Ok(engine)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_SQL)?;
        let engine = Self {
            db_path: PathBuf::from(":memory:"),
            conn: Arc::new(Mutex::new(conn)),
            secrets: SecretVault::open_ephemeral(),
            db_encrypted: false,
        };
        engine.ensure_defaults_seeded()?;
        Ok(engine)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn is_encrypted(&self) -> bool {
        self.db_encrypted
    }

    pub fn secrets(&self) -> &SecretVault {
        &self.secrets
    }

    fn ensure_defaults_seeded(&self) -> Result<()> {
        let has_profile: bool = {
            let conn = self.conn.lock();
            conn.query_row("SELECT COUNT(*) FROM user_profile", [], |r| r.get::<_, i64>(0))? > 0
        };
        if has_profile {
            return Ok(());
        }

        self.save_user_profile(&UserProfile::default())?;
        for rule in seed::builtin_privacy_rules() {
            self.upsert_privacy_rule(&rule)?;
        }
        for rec in seed::builtin_recipes() {
            self.upsert_recipe(&rec)?;
        }
        for voc in seed::builtin_vocabulary() {
            self.upsert_vocabulary(&voc)?;
        }
        for pref in seed::builtin_learned_preferences() {
            self.upsert_learned_preference(&pref)?;
        }

        // Seed realistic initial data so UI & MCP tools work richly out of the box
        let demo = seed::initial_demo_dataset();
        for (person, vp) in demo.persons {
            self.upsert_person(&person)?;
            self.save_person_voiceprint(&person.id, &vp, person.voiceprint_samples, person.voiceprint_confidence)?;
        }
        for (meeting, segments, frames) in demo.meetings {
            self.save_meeting(&meeting)?;
            for seg in segments {
                self.insert_transcript_segment(&seg)?;
            }
            for frame in frames {
                self.insert_screenshare_frame(&frame)?;
            }
        }
        for ep in demo.episodes {
            self.upsert_context_episode(&ep)?;
        }
        for snap in demo.snapshots {
            self.insert_context_snapshot(&snap)?;
        }
        for clip in demo.clipboard {
            self.insert_clipboard_item(&clip)?;
        }
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Indexing Helper (Hybrid FTS5 + 256-dim Vector Index)
    // -------------------------------------------------------------------------

    pub fn index_memory_document(
        &self,
        doc_id: &str,
        source_kind: &str,
        title: &str,
        body: &str,
        snippet: &str,
        timestamp: DateTime<Utc>,
        project_tag: Option<&str>,
        metadata: &serde_json::Value,
    ) -> Result<()> {
        let combined = format!("{title}\n{body}");
        let embedding = embed_text_local_256(&combined);
        let emb_blob = encode_f32_vec(&embedding);
        let meta_str = serde_json::to_string(metadata)?;
        let ts_str = timestamp.to_rfc3339();
        let proj = project_tag.unwrap_or("");

        let conn = self.conn.lock();
        conn.execute("DELETE FROM memory_fts WHERE doc_id = ?1", params![doc_id])?;
        conn.execute(
            "INSERT INTO memory_fts (doc_id, source_kind, title, body, project_tag)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![doc_id, source_kind, title, body, proj],
        )?;
        conn.execute(
            "INSERT INTO memory_vectors (doc_id, source_kind, title, snippet, timestamp, project_tag, metadata_json, embedding_blob)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(doc_id) DO UPDATE SET
                source_kind = excluded.source_kind,
                title = excluded.title,
                snippet = excluded.snippet,
                timestamp = excluded.timestamp,
                project_tag = excluded.project_tag,
                metadata_json = excluded.metadata_json,
                embedding_blob = excluded.embedding_blob",
            params![doc_id, source_kind, title, snippet, ts_str, project_tag, meta_str, emb_blob],
        )?;
        Ok(())
    }

    pub fn search_hybrid(
        &self,
        query: &str,
        limit: usize,
        source_filter: Option<&str>,
        project_filter: Option<&str>,
        max_age_days: Option<u32>,
    ) -> Result<Vec<MemorySearchHit>> {
        let limit = limit.clamp(1, 50);
        let cutoff_iso = max_age_days
            .map(|days| (Utc::now() - Duration::days(days as i64)).to_rfc3339());
        let query_vec = embed_text_local_256(query);
        let fts_query = sanitize_fts_query(query);

        let conn = self.conn.lock();
        let mut rrf_scores: HashMap<String, f32> = HashMap::new();

        // 1. FTS5 BM25 Rank
        if !fts_query.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT doc_id, bm25(memory_fts) as rank
                 FROM memory_fts
                 WHERE memory_fts MATCH ?1
                 ORDER BY rank ASC
                 LIMIT 60",
            )?;
            let fts_rows: Vec<String> = stmt
                .query_map(params![fts_query], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            for (rank_idx, doc_id) in fts_rows.into_iter().enumerate() {
                let score = 1.0 / (40.0 + (rank_idx as f32));
                *rrf_scores.entry(doc_id).or_insert(0.0) += score * 1.35;
            }
        }

        // 2. Semantic Vector Cosine Similarity + Recency Boost
        let mut stmt = conn.prepare(
            "SELECT doc_id, source_kind, title, snippet, timestamp, project_tag, metadata_json, embedding_blob
             FROM memory_vectors
             ORDER BY timestamp DESC
             LIMIT 500",
        )?;

        struct Candidate {
            hit: MemorySearchHit,
            vec_sim: f32,
        }

        let mut candidates: Vec<Candidate> = Vec::new();
        let rows = stmt.query_map([], |row| {
            let doc_id: String = row.get(0)?;
            let source_kind: String = row.get(1)?;
            let title: String = row.get(2)?;
            let snippet: String = row.get(3)?;
            let ts_str: String = row.get(4)?;
            let project_tag: Option<String> = row.get(5)?;
            let meta_str: String = row.get(6)?;
            let blob: Vec<u8> = row.get(7)?;
            Ok((
                doc_id,
                source_kind,
                title,
                snippet,
                ts_str,
                project_tag,
                meta_str,
                blob,
            ))
        })?;

        for row in rows {
            let (doc_id, source_kind, title, snippet, ts_str, project_tag, meta_str, blob) = row?;
            if let Some(sf) = source_filter {
                if !sf.is_empty() && sf != "all" && source_kind != sf {
                    continue;
                }
            }
            if let Some(pf) = project_filter {
                if !pf.is_empty()
                    && project_tag
                        .as_deref()
                        .map(|p| !p.eq_ignore_ascii_case(pf))
                        .unwrap_or(true)
                {
                    continue;
                }
            }
            if let Some(ref cutoff) = cutoff_iso {
                if &ts_str < cutoff {
                    continue;
                }
            }

            let doc_vec = decode_f32_vec(&blob);
            let sim = cosine_similarity(&query_vec, &doc_vec).max(0.0);
            let metadata: serde_json::Value =
                serde_json::from_str(&meta_str).unwrap_or(serde_json::json!({}));

            candidates.push(Candidate {
                hit: MemorySearchHit {
                    id: doc_id,
                    source_kind,
                    title,
                    snippet,
                    timestamp: parse_dt(&ts_str),
                    score: 0.0,
                    project_tag,
                    metadata,
                },
                vec_sim: sim,
            });
        }

        // Rank by vector similarity to assign RRF score
        candidates.sort_by(|a, b| {
            b.vec_sim
                .partial_cmp(&a.vec_sim)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (rank_idx, cand) in candidates.iter().enumerate() {
            if cand.vec_sim > 0.05 || query.trim().is_empty() {
                let rrf = 1.0 / (45.0 + (rank_idx as f32));
                *rrf_scores.entry(cand.hit.id.clone()).or_insert(0.0) += rrf + (cand.vec_sim * 0.25);
            }
        }

        let mut results: Vec<MemorySearchHit> = candidates
            .into_iter()
            .filter_map(|mut c| {
                let combined = rrf_scores.get(&c.hit.id).copied().unwrap_or(0.0);
                if combined > 0.0 || query.trim().is_empty() {
                    c.hit.score = (combined * 12.0).clamp(0.05, 0.99);
                    Some(c.hit)
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    // -------------------------------------------------------------------------
    // User Profile, Vocabulary & Adaptive Personalization
    // -------------------------------------------------------------------------

    pub fn get_user_profile(&self) -> Result<UserProfile> {
        let conn = self.conn.lock();
        let opt = conn
            .query_row(
                "SELECT id, display_name, email, role, organization, ui_language,
                        preferred_output_language, writing_style, tone_instructions,
                        active_projects_json, self_voiceprint_blob, auto_record_meetings,
                        consent_reminder_enabled, retention_days, updated_at
                 FROM user_profile LIMIT 1",
                [],
                |row| {
                    let projects_str: String = row.get(9)?;
                    let vp_blob: Option<Vec<u8>> = row.get(10)?;
                    let vp = vp_blob.map(|b| decode_f32_vec(&b));
                    Ok(UserProfile {
                        id: row.get(0)?,
                        display_name: row.get(1)?,
                        email: row.get(2)?,
                        role: row.get(3)?,
                        organization: row.get(4)?,
                        ui_language: Language::from_code(&row.get::<_, String>(5)?),
                        preferred_output_language: Language::from_code(&row.get::<_, String>(6)?),
                        writing_style: row.get(7)?,
                        tone_instructions: row.get(8)?,
                        active_projects: serde_json::from_str(&projects_str).unwrap_or_default(),
                        has_self_voiceprint: vp.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
                        self_voiceprint: vp,
                        auto_record_meetings: row.get::<_, i64>(11)? != 0,
                        consent_reminder_enabled: row.get::<_, i64>(12)? != 0,
                        retention_days: row.get::<_, u32>(13)?,
                        updated_at: parse_dt(&row.get::<_, String>(14)?),
                    })
                },
            )
            .optional()?;
        Ok(opt.unwrap_or_default())
    }

    pub fn save_user_profile(&self, profile: &UserProfile) -> Result<()> {
        let projects_json = serde_json::to_string(&profile.active_projects)?;
        let vp_blob = profile.self_voiceprint.as_ref().map(|v| encode_f32_vec(v));
        let now_str = Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_profile (
                id, display_name, email, role, organization, ui_language,
                preferred_output_language, writing_style, tone_instructions,
                active_projects_json, self_voiceprint_blob, auto_record_meetings,
                consent_reminder_enabled, retention_days, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                email = excluded.email,
                role = excluded.role,
                organization = excluded.organization,
                ui_language = excluded.ui_language,
                preferred_output_language = excluded.preferred_output_language,
                writing_style = excluded.writing_style,
                tone_instructions = excluded.tone_instructions,
                active_projects_json = excluded.active_projects_json,
                self_voiceprint_blob = COALESCE(excluded.self_voiceprint_blob, user_profile.self_voiceprint_blob),
                auto_record_meetings = excluded.auto_record_meetings,
                consent_reminder_enabled = excluded.consent_reminder_enabled,
                retention_days = excluded.retention_days,
                updated_at = excluded.updated_at",
            params![
                profile.id,
                profile.display_name,
                profile.email,
                profile.role,
                profile.organization,
                profile.ui_language.as_code(),
                profile.preferred_output_language.as_code(),
                profile.writing_style,
                profile.tone_instructions,
                projects_json,
                vp_blob,
                profile.auto_record_meetings as i64,
                profile.consent_reminder_enabled as i64,
                profile.retention_days,
                now_str,
            ],
        )?;
        Ok(())
    }

    pub fn enroll_self_voiceprint(&self, sample_phrase: &str) -> Result<UserProfile> {
        let mut profile = self.get_user_profile()?;
        let embedding = embed_text_local_256(&format!("self_voiceprint::{}::{}", profile.display_name, sample_phrase));
        profile.self_voiceprint = Some(embedding);
        profile.has_self_voiceprint = true;
        self.save_user_profile(&profile)?;
        self.get_user_profile()
    }

    pub fn list_vocabulary(&self) -> Result<Vec<VocabularyEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, term, phonetic_hint, category, project_tag, created_at
             FROM custom_vocabulary ORDER BY term ASC",
        )?;
        let items = stmt
            .query_map([], |row| {
                Ok(VocabularyEntry {
                    id: row.get(0)?,
                    term: row.get(1)?,
                    phonetic_hint: row.get(2)?,
                    category: row.get(3)?,
                    project_tag: row.get(4)?,
                    created_at: parse_dt(&row.get::<_, String>(5)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn upsert_vocabulary(&self, entry: &VocabularyEntry) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO custom_vocabulary (id, term, phonetic_hint, category, project_tag, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(term) DO UPDATE SET
                phonetic_hint = excluded.phonetic_hint,
                category = excluded.category,
                project_tag = excluded.project_tag",
            params![
                entry.id,
                entry.term,
                entry.phonetic_hint,
                entry.category,
                entry.project_tag,
                entry.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn delete_vocabulary(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM custom_vocabulary WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_learned_preferences(&self) -> Result<Vec<LearnedPreference>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, category, fact_text, confidence, source_ref, is_active, created_at
             FROM learned_preferences ORDER BY created_at DESC",
        )?;
        let items = stmt
            .query_map([], |row| {
                Ok(LearnedPreference {
                    id: row.get(0)?,
                    category: row.get(1)?,
                    fact_text: row.get(2)?,
                    confidence: row.get(3)?,
                    source_ref: row.get(4)?,
                    is_active: row.get::<_, i64>(5)? != 0,
                    created_at: parse_dt(&row.get::<_, String>(6)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn upsert_learned_preference(&self, pref: &LearnedPreference) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO learned_preferences (id, category, fact_text, confidence, source_ref, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                category = excluded.category,
                fact_text = excluded.fact_text,
                confidence = excluded.confidence,
                is_active = excluded.is_active",
            params![
                pref.id,
                pref.category,
                pref.fact_text,
                pref.confidence,
                pref.source_ref,
                pref.is_active as i64,
                pref.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn toggle_learned_preference(&self, id: &str, is_active: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE learned_preferences SET is_active = ?2 WHERE id = ?1",
            params![id, is_active as i64],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // People & Speaker Voiceprints
    // -------------------------------------------------------------------------

    pub fn list_persons(&self) -> Result<Vec<Person>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.full_name, p.email, p.company, p.role_notes,
                    p.voiceprint_samples, p.voiceprint_confidence, p.open_commitments_json, p.last_seen_at,
                    (SELECT COUNT(*) FROM meeting_participants mp WHERE mp.person_id = p.id) as mtg_cnt
             FROM persons p
             ORDER BY COALESCE(p.last_seen_at, '') DESC, p.full_name ASC",
        )?;
        let items = stmt
            .query_map([], |row| {
                let commits_json: String = row.get(7)?;
                let last_str: Option<String> = row.get(8)?;
                let db_cnt: u32 = row.get(9)?;
                Ok(Person {
                    id: row.get(0)?,
                    full_name: row.get(1)?,
                    email: row.get(2)?,
                    company: row.get(3)?,
                    role_notes: row.get(4)?,
                    voiceprint_samples: row.get(5)?,
                    voiceprint_confidence: row.get(6)?,
                    meeting_count: db_cnt.max(1),
                    open_commitments: serde_json::from_str(&commits_json).unwrap_or_default(),
                    last_seen_at: last_str.map(|s| parse_dt(&s)),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn upsert_person(&self, person: &Person) -> Result<()> {
        let commits_json = serde_json::to_string(&person.open_commitments)?;
        let last_seen = person.last_seen_at.map(|dt| dt.to_rfc3339());
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO persons (id, full_name, email, company, role_notes, voiceprint_samples, voiceprint_confidence, open_commitments_json, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                full_name = excluded.full_name,
                email = COALESCE(excluded.email, persons.email),
                company = COALESCE(excluded.company, persons.company),
                role_notes = excluded.role_notes,
                open_commitments_json = excluded.open_commitments_json,
                last_seen_at = COALESCE(excluded.last_seen_at, persons.last_seen_at)",
            params![
                person.id,
                person.full_name,
                person.email,
                person.company,
                person.role_notes,
                person.voiceprint_samples,
                person.voiceprint_confidence,
                commits_json,
                last_seen,
            ],
        )?;
        Ok(())
    }

    pub fn save_person_voiceprint(
        &self,
        person_id: &str,
        embedding: &[f32],
        samples: u32,
        confidence: f32,
    ) -> Result<()> {
        let blob = encode_f32_vec(embedding);
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE persons SET voiceprint_blob = ?2, voiceprint_samples = ?3, voiceprint_confidence = ?4 WHERE id = ?1",
            params![person_id, blob, samples, confidence],
        )?;
        Ok(())
    }

    pub fn list_person_voiceprints(&self) -> Result<Vec<(Person, Vec<f32>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, full_name, email, company, role_notes, voiceprint_samples, voiceprint_confidence, open_commitments_json, last_seen_at, voiceprint_blob
             FROM persons WHERE voiceprint_blob IS NOT NULL",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let commits_json: String = row.get(7)?;
                let last_str: Option<String> = row.get(8)?;
                let blob: Vec<u8> = row.get(9)?;
                let person = Person {
                    id: row.get(0)?,
                    full_name: row.get(1)?,
                    email: row.get(2)?,
                    company: row.get(3)?,
                    role_notes: row.get(4)?,
                    voiceprint_samples: row.get(5)?,
                    voiceprint_confidence: row.get(6)?,
                    meeting_count: 1,
                    open_commitments: serde_json::from_str(&commits_json).unwrap_or_default(),
                    last_seen_at: last_str.map(|s| parse_dt(&s)),
                };
                Ok((person, decode_f32_vec(&blob)))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn match_voiceprint(
        &self,
        sample_embedding: &[f32],
        calendar_emails: &[String],
        threshold: f32,
    ) -> Result<Option<(Person, f32)>> {
        let candidates = self.list_person_voiceprints()?;
        let mut best: Option<(Person, f32)> = None;
        for (person, vp) in candidates {
            let mut sim = cosine_similarity(sample_embedding, &vp);
            // Calendar attendee prior boost (+0.05) when person is invited to the call
            if let Some(ref email) = person.email {
                if calendar_emails
                    .iter()
                    .any(|ce| ce.eq_ignore_ascii_case(email))
                {
                    sim = (sim + 0.05).min(0.99);
                }
            }
            if sim >= threshold {
                if best.as_ref().map(|(_, b)| sim > *b).unwrap_or(true) {
                    best = Some((person, sim));
                }
            }
        }
        Ok(best)
    }

    /// Renames a speaker cluster across a meeting, links or creates the Person record,
    /// and updates the person's learned voiceprint centroid.
    pub fn assign_speaker_identity(
        &self,
        meeting_id: &str,
        speaker_label: &str,
        person_name: &str,
        person_email: Option<&str>,
        sample_embedding: Option<&[f32]>,
    ) -> Result<Person> {
        let existing = self
            .list_persons()?
            .into_iter()
            .find(|p| p.full_name.eq_ignore_ascii_case(person_name));

        let person = if let Some(mut p) = existing {
            p.last_seen_at = Some(Utc::now());
            if person_email.is_some() && p.email.is_none() {
                p.email = person_email.map(|e| e.to_string());
            }
            self.upsert_person(&p)?;
            p
        } else {
            let p = Person {
                id: format!("per_{}", uuid::Uuid::new_v4().simple()),
                full_name: person_name.trim().to_string(),
                email: person_email.map(|s| s.to_string()),
                company: None,
                role_notes: "Aus Meeting-Sprecher-Zuordnung gelernt.".into(),
                voiceprint_samples: 1,
                voiceprint_confidence: 0.88,
                meeting_count: 1,
                open_commitments: vec![],
                last_seen_at: Some(Utc::now()),
            };
            self.upsert_person(&p)?;
            p
        };

        // Update learned voiceprint centroid
        let new_vec = sample_embedding
            .map(|s| s.to_vec())
            .unwrap_or_else(|| embed_text_local_256(&format!("voiceprint::{}", person.full_name)));

        let current_vp = self
            .list_person_voiceprints()?
            .into_iter()
            .find(|(p, _)| p.id == person.id)
            .map(|(_, v)| v);

        let (merged_vp, new_count) = match current_vp {
            Some(old) => (
                update_voiceprint_centroid(&old, person.voiceprint_samples, &new_vec),
                person.voiceprint_samples + 1,
            ),
            None => (new_vec, 1),
        };
        let conf = (0.82 + (new_count as f32 * 0.02)).min(0.98);
        self.save_person_voiceprint(&person.id, &merged_vp, new_count, conf)?;

        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE meeting_participants
                 SET display_name = ?3, person_id = ?4, identification_source = 'manual', confidence = 0.99
                 WHERE meeting_id = ?1 AND speaker_label = ?2",
                params![meeting_id, speaker_label, person.full_name, person.id],
            )?;
            conn.execute(
                "UPDATE transcript_segments
                 SET speaker_name = ?3, person_id = ?4
                 WHERE meeting_id = ?1 AND speaker_label = ?2",
                params![meeting_id, speaker_label, person.full_name, person.id],
            )?;
        }

        Ok(self
            .list_persons()?
            .into_iter()
            .find(|p| p.id == person.id)
            .unwrap_or(person))
    }

    // -------------------------------------------------------------------------
    // Meetings, Transcripts & Screen-Sharing Frames
    // -------------------------------------------------------------------------

    pub fn save_meeting(&self, meeting: &Meeting) -> Result<()> {
        let actions_json = serde_json::to_string(&meeting.action_items)?;
        let ended_str = meeting.ended_at.map(|d| d.to_rfc3339());
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT INTO meetings (
                    id, title, calendar_event_id, detected_app, project_tag,
                    started_at, ended_at, status, template_id, prep_brief,
                    raw_user_notes, executive_summary, enhanced_markdown, action_items_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    detected_app = excluded.detected_app,
                    project_tag = excluded.project_tag,
                    ended_at = excluded.ended_at,
                    status = excluded.status,
                    template_id = excluded.template_id,
                    prep_brief = excluded.prep_brief,
                    raw_user_notes = excluded.raw_user_notes,
                    executive_summary = excluded.executive_summary,
                    enhanced_markdown = excluded.enhanced_markdown,
                    action_items_json = excluded.action_items_json",
                params![
                    meeting.id,
                    meeting.title,
                    meeting.calendar_event_id,
                    meeting.detected_app,
                    meeting.project_tag,
                    meeting.started_at.to_rfc3339(),
                    ended_str,
                    meeting.status.as_str(),
                    meeting.template_id,
                    meeting.prep_brief,
                    meeting.raw_user_notes,
                    meeting.executive_summary,
                    meeting.enhanced_markdown,
                    actions_json,
                ],
            )?;

            for part in &meeting.participants {
                conn.execute(
                    "INSERT INTO meeting_participants (
                        meeting_id, speaker_label, display_name, person_id, email,
                        is_self, identification_source, confidence, talk_time_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT(meeting_id, speaker_label) DO UPDATE SET
                        display_name = excluded.display_name,
                        person_id = COALESCE(excluded.person_id, meeting_participants.person_id),
                        email = COALESCE(excluded.email, meeting_participants.email),
                        identification_source = excluded.identification_source,
                        confidence = excluded.confidence,
                        talk_time_ms = excluded.talk_time_ms",
                    params![
                        meeting.id,
                        part.speaker_label,
                        part.display_name,
                        part.person_id,
                        part.email,
                        part.is_self as i64,
                        part.identification_source.as_str(),
                        part.confidence,
                        part.talk_time_ms as i64,
                    ],
                )?;
            }
        }

        if !meeting.executive_summary.is_empty() || !meeting.raw_user_notes.is_empty() {
            let body = format!(
                "{}\n{}\n{}",
                meeting.executive_summary, meeting.enhanced_markdown, meeting.raw_user_notes
            );
            let snippet = if !meeting.executive_summary.is_empty() {
                meeting.executive_summary.clone()
            } else {
                meeting.raw_user_notes.chars().take(220).collect()
            };
            self.index_memory_document(
                &format!("meeting:{}", meeting.id),
                "meeting",
                &meeting.title,
                &body,
                &snippet,
                meeting.started_at,
                meeting.project_tag.as_deref(),
                &serde_json::json!({ "meeting_id": meeting.id, "app": meeting.detected_app }),
            )?;
        }
        Ok(())
    }

    pub fn update_meeting_user_notes(&self, meeting_id: &str, raw_notes: &str) -> Result<()> {
        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE meetings SET raw_user_notes = ?2 WHERE id = ?1",
                params![meeting_id, raw_notes],
            )?;
        }
        if let Some(detail) = self.get_meeting_detail(meeting_id)? {
            self.save_meeting(&detail.meeting)?;
        }
        Ok(())
    }

    pub fn update_meeting_title(&self, meeting_id: &str, title: &str) -> Result<()> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE meetings SET title = ?2 WHERE id = ?1",
                params![meeting_id, trimmed],
            )?;
        }
        if let Some(detail) = self.get_meeting_detail(meeting_id)? {
            self.save_meeting(&detail.meeting)?;
        }
        Ok(())
    }

    pub fn delete_meeting(&self, meeting_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let thread_id = format!("meeting:{meeting_id}");
        let seg_ids: Vec<String> = {
            let mut stmt =
                conn.prepare("SELECT id FROM transcript_segments WHERE meeting_id = ?1")?;
            let rows = stmt
                .query_map(params![meeting_id], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };
        let frame_ids: Vec<String> = {
            let mut stmt =
                conn.prepare("SELECT id FROM screenshare_frames WHERE meeting_id = ?1")?;
            let rows = stmt
                .query_map(params![meeting_id], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        for sid in seg_ids {
            let doc_id = format!("transcript:{sid}");
            let _ = conn.execute("DELETE FROM memory_fts WHERE doc_id = ?1", params![doc_id]);
            let _ = conn.execute("DELETE FROM memory_vectors WHERE doc_id = ?1", params![doc_id]);
        }
        for fid in frame_ids {
            let doc_id = format!("screenshare:{fid}");
            let _ = conn.execute("DELETE FROM memory_fts WHERE doc_id = ?1", params![doc_id]);
            let _ = conn.execute("DELETE FROM memory_vectors WHERE doc_id = ?1", params![doc_id]);
        }
        let mtg_doc = format!("meeting:{meeting_id}");
        let _ = conn.execute("DELETE FROM memory_fts WHERE doc_id = ?1", params![mtg_doc]);
        let _ = conn.execute("DELETE FROM memory_vectors WHERE doc_id = ?1", params![mtg_doc]);

        let _ = conn.execute("DELETE FROM chat_messages WHERE thread_id = ?1 OR meeting_id = ?2", params![thread_id, meeting_id]);
        let _ = conn.execute("DELETE FROM transcript_segments WHERE meeting_id = ?1", params![meeting_id]);
        let _ = conn.execute("DELETE FROM screenshare_frames WHERE meeting_id = ?1", params![meeting_id]);
        let _ = conn.execute("DELETE FROM meeting_participants WHERE meeting_id = ?1", params![meeting_id]);
        let affected = conn.execute("DELETE FROM meetings WHERE id = ?1", params![meeting_id])?;
        Ok(affected > 0)
    }

    pub fn insert_transcript_segment(&self, seg: &TranscriptSegment) -> Result<()> {
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO transcript_segments (
                    id, meeting_id, speaker_label, speaker_name, person_id,
                    channel, start_ms, end_ms, text, confidence
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    seg.id,
                    seg.meeting_id,
                    seg.speaker_label,
                    seg.speaker_name,
                    seg.person_id,
                    seg.channel.as_str(),
                    seg.start_ms as i64,
                    seg.end_ms as i64,
                    seg.text,
                    seg.confidence,
                ],
            )?;
        }
        self.index_memory_document(
            &format!("transcript:{}", seg.id),
            "transcript",
            &format!("{} (Meeting-Zitat)", seg.speaker_name),
            &seg.text,
            &format!("{}: {}", seg.speaker_name, seg.text),
            Utc::now(),
            None,
            &serde_json::json!({
                "meeting_id": seg.meeting_id,
                "speaker": seg.speaker_name,
                "start_ms": seg.start_ms
            }),
        )?;
        Ok(())
    }

    pub fn insert_screenshare_frame(&self, frame: &ScreenShareFrame) -> Result<()> {
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO screenshare_frames (
                    id, meeting_id, captured_at_ms, presenter_name, dhash_hex,
                    image_data_url, ocr_text, slide_title
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    frame.id,
                    frame.meeting_id,
                    frame.captured_at_ms as i64,
                    frame.presenter_name,
                    frame.dhash_hex,
                    frame.image_data_url,
                    frame.ocr_text,
                    frame.slide_title,
                ],
            )?;
        }
        self.index_memory_document(
            &format!("screenshare:{}", frame.id),
            "screenshare",
            &frame.slide_title,
            &frame.ocr_text,
            &frame.ocr_text.chars().take(240).collect::<String>(),
            Utc::now(),
            None,
            &serde_json::json!({
                "meeting_id": frame.meeting_id,
                "captured_at_ms": frame.captured_at_ms,
                "presenter": frame.presenter_name,
            }),
        )?;
        Ok(())
    }

    pub fn list_meetings(&self) -> Result<Vec<Meeting>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, calendar_event_id, detected_app, project_tag,
                    started_at, ended_at, status, template_id, prep_brief,
                    raw_user_notes, executive_summary, enhanced_markdown, action_items_json,
                    (SELECT COUNT(*) FROM transcript_segments ts WHERE ts.meeting_id = meetings.id),
                    (SELECT COUNT(*) FROM screenshare_frames sf WHERE sf.meeting_id = meetings.id)
             FROM meetings
             ORDER BY started_at DESC",
        )?;

        let mut meetings: Vec<Meeting> = stmt
            .query_map([], |row| {
                let ended_str: Option<String> = row.get(6)?;
                let actions_str: String = row.get(13)?;
                Ok(Meeting {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    calendar_event_id: row.get(2)?,
                    detected_app: row.get(3)?,
                    project_tag: row.get(4)?,
                    started_at: parse_dt(&row.get::<_, String>(5)?),
                    ended_at: ended_str.map(|s| parse_dt(&s)),
                    status: MeetingStatus::from_str(&row.get::<_, String>(7)?),
                    template_id: row.get(8)?,
                    prep_brief: row.get(9)?,
                    raw_user_notes: row.get(10)?,
                    executive_summary: row.get(11)?,
                    enhanced_markdown: row.get(12)?,
                    action_items: serde_json::from_str(&actions_str).unwrap_or_default(),
                    participants: Vec::new(),
                    transcript_count: row.get::<_, i64>(14)? as usize,
                    screenshare_count: row.get::<_, i64>(15)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for m in &mut meetings {
            let mut p_stmt = conn.prepare(
                "SELECT speaker_label, display_name, person_id, email, is_self,
                        identification_source, confidence, talk_time_ms
                 FROM meeting_participants WHERE meeting_id = ?1 ORDER BY is_self DESC, talk_time_ms DESC",
            )?;
            m.participants = p_stmt
                .query_map(params![m.id], |row| {
                    Ok(MeetingParticipant {
                        speaker_label: row.get(0)?,
                        display_name: row.get(1)?,
                        person_id: row.get(2)?,
                        email: row.get(3)?,
                        is_self: row.get::<_, i64>(4)? != 0,
                        identification_source: IdentificationSource::from_str(&row.get::<_, String>(5)?),
                        confidence: row.get(6)?,
                        talk_time_ms: row.get::<_, i64>(7)? as u64,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
        }
        Ok(meetings)
    }

    pub fn get_meeting_detail(&self, meeting_id: &str) -> Result<Option<MeetingDetail>> {
        let meetings = self.list_meetings()?;
        let Some(meeting) = meetings.into_iter().find(|m| m.id == meeting_id) else {
            return Ok(None);
        };

        let conn = self.conn.lock();
        let mut t_stmt = conn.prepare(
            "SELECT id, meeting_id, speaker_label, speaker_name, person_id,
                    channel, start_ms, end_ms, text, confidence
             FROM transcript_segments WHERE meeting_id = ?1 ORDER BY start_ms ASC",
        )?;
        let transcript = t_stmt
            .query_map(params![meeting_id], |row| {
                Ok(TranscriptSegment {
                    id: row.get(0)?,
                    meeting_id: row.get(1)?,
                    speaker_label: row.get(2)?,
                    speaker_name: row.get(3)?,
                    person_id: row.get(4)?,
                    channel: AudioChannel::from_str(&row.get::<_, String>(5)?),
                    start_ms: row.get::<_, i64>(6)? as u64,
                    end_ms: row.get::<_, i64>(7)? as u64,
                    text: row.get(8)?,
                    confidence: row.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut s_stmt = conn.prepare(
            "SELECT id, meeting_id, captured_at_ms, presenter_name, dhash_hex,
                    image_data_url, ocr_text, slide_title
             FROM screenshare_frames WHERE meeting_id = ?1 ORDER BY captured_at_ms ASC",
        )?;
        let screenshares = s_stmt
            .query_map(params![meeting_id], |row| {
                Ok(ScreenShareFrame {
                    id: row.get(0)?,
                    meeting_id: row.get(1)?,
                    captured_at_ms: row.get::<_, i64>(2)? as u64,
                    presenter_name: row.get(3)?,
                    dhash_hex: row.get(4)?,
                    image_data_url: row.get(5)?,
                    ocr_text: row.get(6)?,
                    slide_title: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        drop(s_stmt);
        drop(t_stmt);
        drop(conn);

        let chat_messages = self.list_chat_messages(&format!("meeting:{meeting_id}"))?;
        Ok(Some(MeetingDetail {
            meeting,
            transcript,
            screenshares,
            chat_messages,
        }))
    }

    // -------------------------------------------------------------------------
    // Ambient Context Snapshots, Episodes & Clipboard
    // -------------------------------------------------------------------------

    pub fn insert_context_snapshot(&self, snap: &ContextSnapshot) -> Result<()> {
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO context_snapshots (
                    id, captured_at, app_bundle_id, app_name, window_title, browser_url,
                    dwell_ms, dhash_hex, extracted_text, extraction_method,
                    relevance_score, project_tag, redacted_secrets_count, episode_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    snap.id,
                    snap.captured_at.to_rfc3339(),
                    snap.app_bundle_id,
                    snap.app_name,
                    snap.window_title,
                    snap.browser_url,
                    snap.dwell_ms as i64,
                    snap.dhash_hex,
                    snap.extracted_text,
                    snap.extraction_method.as_str(),
                    snap.relevance_score,
                    snap.project_tag,
                    snap.redacted_secrets_count as i64,
                    snap.episode_id,
                ],
            )?;
        }
        let snippet: String = snap.extracted_text.chars().take(220).collect();
        self.index_memory_document(
            &format!("snapshot:{}", snap.id),
            "screen_context",
            &format!("{} — {}", snap.app_name, snap.window_title),
            &snap.extracted_text,
            &snippet,
            snap.captured_at,
            snap.project_tag.as_deref(),
            &serde_json::json!({
                "app": snap.app_name,
                "url": snap.browser_url,
                "method": snap.extraction_method.as_str(),
            }),
        )?;
        Ok(())
    }

    pub fn list_recent_snapshots(&self, limit: usize) -> Result<Vec<ContextSnapshot>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, captured_at, app_bundle_id, app_name, window_title, browser_url,
                    dwell_ms, dhash_hex, extracted_text, extraction_method,
                    relevance_score, project_tag, redacted_secrets_count, episode_id
             FROM context_snapshots ORDER BY captured_at DESC LIMIT ?1",
        )?;
        let items = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ContextSnapshot {
                    id: row.get(0)?,
                    captured_at: parse_dt(&row.get::<_, String>(1)?),
                    app_bundle_id: row.get(2)?,
                    app_name: row.get(3)?,
                    window_title: row.get(4)?,
                    browser_url: row.get(5)?,
                    dwell_ms: row.get::<_, i64>(6)? as u64,
                    dhash_hex: row.get(7)?,
                    extracted_text: row.get(8)?,
                    extraction_method: ExtractionMethod::from_str(&row.get::<_, String>(9)?),
                    relevance_score: row.get(10)?,
                    project_tag: row.get(11)?,
                    redacted_secrets_count: row.get::<_, i64>(12)? as u32,
                    episode_id: row.get(13)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn upsert_context_episode(&self, ep: &ContextEpisode) -> Result<()> {
        let apps_json = serde_json::to_string(&ep.primary_apps)?;
        let urls_json = serde_json::to_string(&ep.urls)?;
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT INTO context_episodes (
                    id, started_at, ended_at, title, summary,
                    primary_apps_json, urls_json, project_tag, importance_score, snapshot_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    ended_at = excluded.ended_at,
                    title = excluded.title,
                    summary = excluded.summary,
                    primary_apps_json = excluded.primary_apps_json,
                    urls_json = excluded.urls_json,
                    importance_score = excluded.importance_score,
                    snapshot_count = excluded.snapshot_count",
                params![
                    ep.id,
                    ep.started_at.to_rfc3339(),
                    ep.ended_at.to_rfc3339(),
                    ep.title,
                    ep.summary,
                    apps_json,
                    urls_json,
                    ep.project_tag,
                    ep.importance_score,
                    ep.snapshot_count as i64,
                ],
            )?;
        }
        self.index_memory_document(
            &format!("episode:{}", ep.id),
            "episode",
            &ep.title,
            &format!("{}\nApps: {}\nURLs: {}", ep.summary, ep.primary_apps.join(", "), ep.urls.join(" ")),
            &ep.summary,
            ep.ended_at,
            ep.project_tag.as_deref(),
            &serde_json::json!({ "apps": ep.primary_apps, "urls": ep.urls }),
        )?;
        Ok(())
    }

    pub fn list_context_episodes(&self, limit: usize) -> Result<Vec<ContextEpisode>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, started_at, ended_at, title, summary,
                    primary_apps_json, urls_json, project_tag, importance_score, snapshot_count
             FROM context_episodes ORDER BY started_at DESC LIMIT ?1",
        )?;
        let items = stmt
            .query_map(params![limit as i64], |row| {
                let apps_str: String = row.get(5)?;
                let urls_str: String = row.get(6)?;
                Ok(ContextEpisode {
                    id: row.get(0)?,
                    started_at: parse_dt(&row.get::<_, String>(1)?),
                    ended_at: parse_dt(&row.get::<_, String>(2)?),
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    primary_apps: serde_json::from_str(&apps_str).unwrap_or_default(),
                    urls: serde_json::from_str(&urls_str).unwrap_or_default(),
                    project_tag: row.get(7)?,
                    importance_score: row.get(8)?,
                    snapshot_count: row.get::<_, i64>(9)? as u32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn insert_clipboard_item(&self, item: &ClipboardItem) -> Result<()> {
        {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT OR REPLACE INTO clipboard_items (
                    id, captured_at, source_app, content_kind, text_preview,
                    content_hash, relevance_score, project_tag
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    item.id,
                    item.captured_at.to_rfc3339(),
                    item.source_app,
                    item.content_kind,
                    item.text_preview,
                    item.content_hash,
                    item.relevance_score,
                    item.project_tag,
                ],
            )?;
        }
        self.index_memory_document(
            &format!("clipboard:{}", item.id),
            "clipboard",
            &format!("Clipboard ({})", item.source_app),
            &item.text_preview,
            &item.text_preview.chars().take(200).collect::<String>(),
            item.captured_at,
            item.project_tag.as_deref(),
            &serde_json::json!({ "source_app": item.source_app, "kind": item.content_kind }),
        )?;
        Ok(())
    }

    pub fn list_clipboard_items(&self, limit: usize) -> Result<Vec<ClipboardItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, captured_at, source_app, content_kind, text_preview, content_hash, relevance_score, project_tag
             FROM clipboard_items ORDER BY captured_at DESC LIMIT ?1",
        )?;
        let items = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ClipboardItem {
                    id: row.get(0)?,
                    captured_at: parse_dt(&row.get::<_, String>(1)?),
                    source_app: row.get(2)?,
                    content_kind: row.get(3)?,
                    text_preview: row.get(4)?,
                    content_hash: row.get(5)?,
                    relevance_score: row.get(6)?,
                    project_tag: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    /// Purges captured screen snapshots, episodes, and clipboard history from the last `minutes`.
    pub fn purge_recent_context(&self, minutes: i64) -> Result<usize> {
        let cutoff = (Utc::now() - Duration::minutes(minutes)).to_rfc3339();
        let conn = self.conn.lock();

        let mut del_ids: Vec<String> = Vec::new();
        {
            let mut s1 = conn.prepare("SELECT 'snapshot:' || id FROM context_snapshots WHERE captured_at >= ?1")?;
            del_ids.extend(s1.query_map(params![cutoff], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()));

            let mut s2 = conn.prepare("SELECT 'episode:' || id FROM context_episodes WHERE ended_at >= ?1")?;
            del_ids.extend(s2.query_map(params![cutoff], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()));

            let mut s3 = conn.prepare("SELECT 'clipboard:' || id FROM clipboard_items WHERE captured_at >= ?1")?;
            del_ids.extend(s3.query_map(params![cutoff], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()));
        }

        let c1 = conn.execute("DELETE FROM context_snapshots WHERE captured_at >= ?1", params![cutoff])?;
        let c2 = conn.execute("DELETE FROM context_episodes WHERE ended_at >= ?1", params![cutoff])?;
        let c3 = conn.execute("DELETE FROM clipboard_items WHERE captured_at >= ?1", params![cutoff])?;

        for doc_id in &del_ids {
            conn.execute("DELETE FROM memory_fts WHERE doc_id = ?1", params![doc_id])?;
            conn.execute("DELETE FROM memory_vectors WHERE doc_id = ?1", params![doc_id])?;
        }
        Ok(c1 + c2 + c3)
    }

    // -------------------------------------------------------------------------
    // Recipes & Chat Threads
    // -------------------------------------------------------------------------

    pub fn list_recipes(&self) -> Result<Vec<Recipe>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, slug, name_de, name_en, description_de, description_en,
                    icon_key, scope, prompt_template, preferred_model,
                    is_builtin, is_favorite, usage_count
             FROM recipes ORDER BY is_favorite DESC, usage_count DESC, slug ASC",
        )?;
        let items = stmt
            .query_map([], |row| {
                Ok(Recipe {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    name_de: row.get(2)?,
                    name_en: row.get(3)?,
                    description_de: row.get(4)?,
                    description_en: row.get(5)?,
                    icon_key: row.get(6)?,
                    scope: RecipeScope::from_str(&row.get::<_, String>(7)?),
                    prompt_template: row.get(8)?,
                    preferred_model: row.get(9)?,
                    is_builtin: row.get::<_, i64>(10)? != 0,
                    is_favorite: row.get::<_, i64>(11)? != 0,
                    usage_count: row.get::<_, i64>(12)? as u32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(items)
    }

    pub fn upsert_recipe(&self, recipe: &Recipe) -> Result<()> {
        let slug = if recipe.slug.starts_with('/') {
            recipe.slug.clone()
        } else {
            format!("/{}", recipe.slug)
        };
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO recipes (
                id, slug, name_de, name_en, description_de, description_en,
                icon_key, scope, prompt_template, preferred_model,
                is_builtin, is_favorite, usage_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(slug) DO UPDATE SET
                name_de = excluded.name_de,
                name_en = excluded.name_en,
                description_de = excluded.description_de,
                description_en = excluded.description_en,
                icon_key = excluded.icon_key,
                scope = excluded.scope,
                prompt_template = excluded.prompt_template,
                preferred_model = excluded.preferred_model,
                is_favorite = excluded.is_favorite",
            params![
                recipe.id,
                slug,
                recipe.name_de,
                recipe.name_en,
                recipe.description_de,
                recipe.description_en,
                recipe.icon_key,
                recipe.scope.as_str(),
                recipe.prompt_template,
                recipe.preferred_model,
                recipe.is_builtin as i64,
                recipe.is_favorite as i64,
                recipe.usage_count as i64,
            ],
        )?;
        Ok(())
    }

    pub fn increment_recipe_usage(&self, slug: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE recipes SET usage_count = usage_count + 1 WHERE slug = ?1",
            params![slug],
        )?;
        Ok(())
    }

    pub fn delete_recipe(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM recipes WHERE id = ?1 AND is_builtin = 0",
            params![id],
        )?;
        Ok(())
    }

    pub fn insert_chat_message(&self, msg: &ChatMessage) -> Result<()> {
        let sources_json = serde_json::to_string(&msg.sources)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO chat_messages (
                id, thread_id, meeting_id, role, content, recipe_slug, sources_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                msg.id,
                msg.thread_id,
                msg.meeting_id,
                msg.role,
                msg.content,
                msg.recipe_slug,
                sources_json,
                msg.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_chat_messages(&self, thread_id: &str) -> Result<Vec<ChatMessage>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, meeting_id, role, content, recipe_slug, sources_json, created_at
             FROM chat_messages WHERE thread_id = ?1 ORDER BY created_at ASC",
        )?;
        let msgs = stmt
            .query_map(params![thread_id], |row| {
                let src_str: String = row.get(6)?;
                Ok(ChatMessage {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    meeting_id: row.get(2)?,
                    role: row.get(3)?,
                    content: row.get(4)?,
                    recipe_slug: row.get(5)?,
                    sources: serde_json::from_str(&src_str).unwrap_or_default(),
                    created_at: parse_dt(&row.get::<_, String>(7)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(msgs)
    }

    // -------------------------------------------------------------------------
    // Privacy Rules, MCP Policy, Audit Log & AI Provider Settings
    // -------------------------------------------------------------------------

    pub fn list_privacy_rules(&self) -> Result<Vec<PrivacyRule>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, kind, pattern, description, is_enabled, is_builtin
             FROM privacy_rules ORDER BY is_builtin DESC, id ASC",
        )?;
        let rules = stmt
            .query_map([], |row| {
                Ok(PrivacyRule {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pattern: row.get(2)?,
                    description: row.get(3)?,
                    is_enabled: row.get::<_, i64>(4)? != 0,
                    is_builtin: row.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rules)
    }

    pub fn upsert_privacy_rule(&self, rule: &PrivacyRule) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO privacy_rules (id, kind, pattern, description, is_enabled, is_builtin)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                pattern = excluded.pattern,
                description = excluded.description,
                is_enabled = excluded.is_enabled",
            params![
                rule.id,
                rule.kind,
                rule.pattern,
                rule.description,
                rule.is_enabled as i64,
                rule.is_builtin as i64,
            ],
        )?;
        Ok(())
    }

    pub fn toggle_privacy_rule(&self, id: &str, is_enabled: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE privacy_rules SET is_enabled = ?2 WHERE id = ?1",
            params![id, is_enabled as i64],
        )?;
        Ok(())
    }

    pub fn delete_privacy_rule(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM privacy_rules WHERE id = ?1 AND is_builtin = 0",
            params![id],
        )?;
        Ok(())
    }

    pub fn get_mcp_policy(&self) -> Result<McpGovernancePolicy> {
        let conn = self.conn.lock();
        let opt: Option<String> = conn
            .query_row(
                "SELECT value_json FROM kv_settings WHERE key = 'mcp_policy'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(opt
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn save_mcp_policy(&self, policy: &McpGovernancePolicy) -> Result<()> {
        let val = serde_json::to_string(policy)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO kv_settings (key, value_json, updated_at) VALUES ('mcp_policy', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![val, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_ai_settings(&self) -> Result<AiProviderSettings> {
        let conn = self.conn.lock();
        let opt: Option<String> = conn
            .query_row(
                "SELECT value_json FROM kv_settings WHERE key = 'ai_settings'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        drop(conn);
        let mut settings: AiProviderSettings = opt
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        settings.openai_api_key_set = self.secrets.has_secret(secrets::KEY_OPENAI);
        settings.anthropic_api_key_set = self.secrets.has_secret(secrets::KEY_ANTHROPIC);
        settings.gemini_api_key_set = self.secrets.has_secret(secrets::KEY_GEMINI);
        settings.openrouter_api_key_set = self.secrets.has_secret(secrets::KEY_OPENROUTER);
        settings.primalabs_api_key_set = self.secrets.has_secret(secrets::KEY_PRIMALABS);
        settings.deepgram_api_key_set = self.secrets.has_secret(secrets::KEY_DEEPGRAM);
        settings.groq_api_key_set = self.secrets.has_secret(secrets::KEY_GROQ);
        Ok(settings)
    }

    pub fn save_ai_settings(&self, settings: &AiProviderSettings) -> Result<()> {
        let val = serde_json::to_string(settings)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO kv_settings (key, value_json, updated_at) VALUES ('ai_settings', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![val, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn save_self_voiceprint_embedding(&self, embedding: &[f32]) -> Result<UserProfile> {
        let mut profile = self.get_user_profile()?;
        let mut normalized = embedding.to_vec();
        l2_normalize(&mut normalized);
        profile.self_voiceprint = Some(normalized);
        profile.has_self_voiceprint = true;
        self.save_user_profile(&profile)?;
        self.get_user_profile()
    }

    pub fn update_snapshot_dwell(&self, snapshot_id: &str, added_ms: u64) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE context_snapshots SET dwell_ms = dwell_ms + ?2 WHERE id = ?1",
            params![snapshot_id, added_ms as i64],
        )?;
        Ok(())
    }

    pub fn update_memory_vector_embedding(&self, doc_id: &str, embedding: &[f32]) -> Result<()> {
        if embedding.len() != EMBEDDING_DIM {
            return Ok(());
        }
        let blob = encode_f32_vec(embedding);
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE memory_vectors SET embedding_blob = ?2 WHERE doc_id = ?1",
            params![doc_id, blob],
        )?;
        Ok(())
    }

    pub fn has_demo_data(&self) -> Result<bool> {
        let conn = self.conn.lock();
        let cnt: i64 = conn.query_row(
            "SELECT COUNT(*) FROM meetings WHERE id IN ('mtg_arch_sync', 'mtg_upcoming_velox')",
            [],
            |r| r.get(0),
        )?;
        Ok(cnt > 0)
    }

    /// Removes the initial demo dataset while preserving user profile, recipes, vocabulary, and privacy rules.
    pub fn clear_demo_data(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let mut removed = 0;
        for sql in [
            "DELETE FROM transcript_segments WHERE meeting_id IN ('mtg_arch_sync', 'mtg_upcoming_velox')",
            "DELETE FROM screenshare_frames WHERE meeting_id IN ('mtg_arch_sync', 'mtg_upcoming_velox')",
            "DELETE FROM meeting_participants WHERE meeting_id IN ('mtg_arch_sync', 'mtg_upcoming_velox')",
            "DELETE FROM meetings WHERE id IN ('mtg_arch_sync', 'mtg_upcoming_velox')",
            "DELETE FROM persons WHERE id IN ('per_anna', 'per_markus', 'per_clara')",
            "DELETE FROM context_episodes WHERE id IN ('ep_mcp_spec', 'ep_recipes_design')",
            "DELETE FROM context_snapshots WHERE id IN ('snap_1', 'snap_2')",
            "DELETE FROM clipboard_items WHERE id IN ('clip_1', 'clip_2')",
            "DELETE FROM memory_fts WHERE doc_id LIKE 'meeting:mtg_%' OR doc_id LIKE 'transcript:seg_%' OR doc_id LIKE 'screenshare:frame_%' OR doc_id LIKE 'episode:ep_%' OR doc_id LIKE 'snapshot:snap_%' OR doc_id LIKE 'clipboard:clip_%'",
            "DELETE FROM memory_vectors WHERE doc_id LIKE 'meeting:mtg_%' OR doc_id LIKE 'transcript:seg_%' OR doc_id LIKE 'screenshare:frame_%' OR doc_id LIKE 'episode:ep_%' OR doc_id LIKE 'snapshot:snap_%' OR doc_id LIKE 'clipboard:clip_%'",
        ] {
            removed += conn.execute(sql, [])?;
        }
        conn.execute(
            "INSERT INTO kv_settings (key, value_json, updated_at) VALUES ('demo_cleared', 'true', ?1)
             ON CONFLICT(key) DO UPDATE SET value_json = 'true', updated_at = excluded.updated_at",
            params![Utc::now().to_rfc3339()],
        )?;
        Ok(removed)
    }

    pub fn list_calendar_feeds(&self) -> Result<Vec<CalendarFeedConfig>> {
        let conn = self.conn.lock();
        let opt: Option<String> = conn
            .query_row(
                "SELECT value_json FROM kv_settings WHERE key = 'calendar_feeds'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(opt
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn save_calendar_feeds(&self, feeds: &[CalendarFeedConfig]) -> Result<()> {
        let val = serde_json::to_string(feeds)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO kv_settings (key, value_json, updated_at) VALUES ('calendar_feeds', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![val, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Upserts a real calendar event as a scheduled meeting with an automatically synthesized Pre-Meeting Brief.
    pub fn upsert_calendar_event_meeting(
        &self,
        event_id: &str,
        title: &str,
        starts_at: DateTime<Utc>,
        ends_at: Option<DateTime<Utc>>,
        conference_app: &str,
        attendees: &[(String, String)],
    ) -> Result<bool> {
        let existing = self
            .list_meetings()?
            .into_iter()
            .find(|m| m.calendar_event_id.as_deref() == Some(event_id));
        if existing.is_some() {
            return Ok(false);
        }

        let profile = self.get_user_profile()?;
        let known_persons = self.list_persons()?;
        let recent_eps = self.list_context_episodes(3)?;

        let mut participants = vec![MeetingParticipant {
            speaker_label: "SELF".into(),
            display_name: format!("{} (Ich)", profile.display_name),
            person_id: None,
            email: profile.email.clone(),
            is_self: true,
            identification_source: IdentificationSource::CalendarPrior,
            confidence: 1.0,
            talk_time_ms: 0,
        }];

        let mut brief_lines = vec![format!(
            "**Vorbereitungs-Briefing (aus Kalender & Gedächtnis):**"
        )];

        for (idx, (name, email)) in attendees.iter().enumerate() {
            let matched = known_persons.iter().find(|p| {
                p.email
                    .as_deref()
                    .map(|e| e.eq_ignore_ascii_case(email))
                    .unwrap_or(false)
                    || p.full_name.eq_ignore_ascii_case(name)
            });

            if let Some(p) = matched {
                if !p.open_commitments.is_empty() {
                    brief_lines.push(format!(
                        "- **Offen mit {}:** {}",
                        p.full_name,
                        p.open_commitments.join(" · ")
                    ));
                } else {
                    brief_lines.push(format!("- **Teilnehmer {}:** {}", p.full_name, p.role_notes));
                }
            }

            participants.push(MeetingParticipant {
                speaker_label: format!("CAL_{}", idx + 1),
                display_name: if name.trim().is_empty() {
                    email.clone()
                } else {
                    name.clone()
                },
                person_id: matched.map(|p| p.id.clone()),
                email: if email.is_empty() { None } else { Some(email.clone()) },
                is_self: false,
                identification_source: IdentificationSource::CalendarPrior,
                confidence: 0.95,
                talk_time_ms: 0,
            });
        }

        if let Some(ep) = recent_eps.first() {
            brief_lines.push(format!(
                "- **Dein jüngster Arbeitskontext:** *{}* ({})",
                ep.title,
                ep.primary_apps.join(", ")
            ));
        }

        let meeting = Meeting {
            id: format!("mtg_cal_{}", uuid::Uuid::new_v4().simple()),
            title: title.to_string(),
            calendar_event_id: Some(event_id.to_string()),
            detected_app: conference_app.to_string(),
            project_tag: profile.active_projects.first().cloned(),
            started_at: starts_at,
            ended_at: ends_at,
            status: MeetingStatus::Scheduled,
            template_id: "general".into(),
            prep_brief: Some(brief_lines.join("\n")),
            raw_user_notes: String::new(),
            executive_summary: String::new(),
            enhanced_markdown: String::new(),
            action_items: vec![],
            participants,
            transcript_count: 0,
            screenshare_count: 0,
        };
        self.save_meeting(&meeting)?;
        Ok(true)
    }

    pub fn record_mcp_audit(&self, entry: &McpAuditEntry) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO mcp_audit_logs (
                id, client_name, transport, tool_name, query_summary,
                items_returned, duration_ms, allowed, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id,
                entry.client_name,
                entry.transport,
                entry.tool_name,
                entry.query_summary,
                entry.items_returned as i64,
                entry.duration_ms as i64,
                entry.allowed as i64,
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_mcp_audit_logs(&self, limit: usize) -> Result<Vec<McpAuditEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, client_name, transport, tool_name, query_summary,
                    items_returned, duration_ms, allowed, created_at
             FROM mcp_audit_logs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(McpAuditEntry {
                    id: row.get(0)?,
                    client_name: row.get(1)?,
                    transport: row.get(2)?,
                    tool_name: row.get(3)?,
                    query_summary: row.get(4)?,
                    items_returned: row.get::<_, i64>(5)? as usize,
                    duration_ms: row.get::<_, i64>(6)? as u64,
                    allowed: row.get::<_, i64>(7)? != 0,
                    created_at: parse_dt(&row.get::<_, String>(8)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_search_finds_seeded_meeting_and_screenshare() {
        let store = StorageEngine::open_in_memory().unwrap();
        let hits = store
            .search_hybrid("ScreenCaptureKit dHash Vision OCR", 10, None, None, None)
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|h| h.source_kind == "screenshare" || h.source_kind == "meeting"));
    }

    #[test]
    fn voiceprint_matching_and_manual_learning_work() {
        let store = StorageEngine::open_in_memory().unwrap();
        let probe = embed_text_local_256("voiceprint::markus_klein_baritone_de_pitch_122hz");
        let matched = store
            .match_voiceprint(&probe, &["markus@kernwerk.dev".to_string()], 0.75)
            .unwrap();
        assert!(matched.is_some());
        let (person, conf) = matched.unwrap();
        assert_eq!(person.full_name, "Markus Klein");
        assert!(conf > 0.90);
    }

    #[test]
    fn purge_recent_context_clears_recent_items_only() {
        let store = StorageEngine::open_in_memory().unwrap();
        let purged = store.purge_recent_context(25).unwrap();
        assert!(purged >= 1);
        let meetings = store.list_meetings().unwrap();
        assert!(meetings.len() >= 2, "Meetings should remain intact when purging ambient snapshots");
    }

    #[test]
    fn sqlcipher_encrypts_database_at_rest_and_clears_demo_data() {
        let dir = std::env::temp_dir().join(format!("backtext_sqlcipher_{}", uuid::Uuid::new_v4().simple()));
        let db_path = dir.join("backtext.db");
        {
            let store = StorageEngine::open(&db_path).unwrap();
            assert!(store.is_encrypted(), "Expected SQLCipher AES-256 encryption to be active");
            assert!(store.has_demo_data().unwrap());
            let removed = store.clear_demo_data().unwrap();
            assert!(removed > 0);
            assert!(!store.has_demo_data().unwrap());
        }
        // Raw file bytes on disk MUST NOT start with plaintext SQLite header
        assert!(
            !file_is_plaintext_sqlite(&db_path),
            "Encrypted SQLCipher DB should never expose plaintext SQLite header"
        );
        // Reopening with persisted vault key succeeds transparently
        let reopened = StorageEngine::open(&db_path).unwrap();
        assert!(reopened.is_encrypted());
        assert!(!reopened.has_demo_data().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_delete_meeting_and_provider_keys_work() {
        let store = StorageEngine::open_in_memory().unwrap();
        let meetings = store.list_meetings().unwrap();
        let first_id = meetings[0].id.clone();

        store.update_meeting_title(&first_id, "Q4 Launch Sync").unwrap();
        let renamed = store.get_meeting_detail(&first_id).unwrap().unwrap();
        assert_eq!(renamed.meeting.title, "Q4 Launch Sync");

        store.secrets().set_secret(secrets::KEY_OPENROUTER, "sk-or-test").unwrap();
        store.secrets().set_secret(secrets::KEY_PRIMALABS, "pl-test-key").unwrap();
        let mut cfg = store.get_ai_settings().unwrap();
        assert!(cfg.openrouter_api_key_set);
        assert!(cfg.primalabs_api_key_set);
        cfg.hover_shortcut = "CommandOrControl+Shift+Space".into();
        store.save_ai_settings(&cfg).unwrap();
        assert_eq!(
            store.get_ai_settings().unwrap().hover_shortcut,
            "CommandOrControl+Shift+Space"
        );

        assert!(store.delete_meeting(&first_id).unwrap());
        assert!(store.get_meeting_detail(&first_id).unwrap().is_none());
    }
}

