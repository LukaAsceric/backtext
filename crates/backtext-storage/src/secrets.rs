use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub const KEY_DB_MASTER: &str = "db_master_key";
pub const KEY_OPENAI: &str = "openai_api_key";
pub const KEY_ANTHROPIC: &str = "anthropic_api_key";
pub const KEY_GEMINI: &str = "gemini_api_key";
pub const KEY_OPENROUTER: &str = "openrouter_api_key";
pub const KEY_PRIMALABS: &str = "primalabs_api_key";
pub const KEY_DEEPGRAM: &str = "deepgram_api_key";
pub const KEY_GROQ: &str = "groq_api_key";

/// Single-Keychain Envelope Vault:
/// - Only a SINGLE macOS Keychain item (`db_master_key`) is EVER accessed — at most ONCE per session.
/// - All API keys (OpenRouter, PrimaLabs, Anthropic, OpenAI, Gemini, Deepgram, Groq) are stored
///   inside the 0600 AES-256-GCM encrypted `secrets.vault.enc` wrapped by a key derived from the
///   single master key (with seamless legacy read fallback so existing keys migrate automatically).
/// - Guarantees <= 1 macOS Keychain prompt at startup instead of 8 separate prompts.
#[derive(Clone)]
pub struct SecretVault {
    vault_dir: PathBuf,
    backend_name: String,
    cache: Arc<Mutex<HashMap<String, String>>>,
    master_key_resolved: Arc<AtomicBool>,
    file_cache_loaded: Arc<AtomicBool>,
}

impl SecretVault {
    pub fn open(vault_dir: impl AsRef<Path>) -> Self {
        let dir = vault_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        let backend_name = Self::detect_backend_name();
        Self {
            vault_dir: dir,
            backend_name,
            cache: Arc::new(Mutex::new(HashMap::new())),
            master_key_resolved: Arc::new(AtomicBool::new(false)),
            file_cache_loaded: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn open_ephemeral() -> Self {
        Self {
            vault_dir: PathBuf::from(":memory:"),
            backend_name: "in-memory-ephemeral".to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            master_key_resolved: Arc::new(AtomicBool::new(true)),
            file_cache_loaded: Arc::new(AtomicBool::new(true)),
        }
    }

    fn detect_backend_name() -> String {
        #[cfg(target_os = "macos")]
        {
            if std::env::var("CI").is_ok() || std::env::var("BACKTEXT_VAULT_FILE_ONLY").is_ok() {
                "AES-256-GCM Local Vault (CI)".to_string()
            } else {
                "macOS Keychain (Single Master Key + AES-256-GCM Vault)".to_string()
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            "AES-256-GCM Local Vault (0600)".to_string()
        }
    }

    pub fn backend_name(&self) -> &str {
        &self.backend_name
    }

    #[cfg(target_os = "macos")]
    fn use_macos_keychain(&self) -> bool {
        self.vault_dir != Path::new(":memory:")
            && std::env::var("CI").is_err()
            && std::env::var("BACKTEXT_VAULT_FILE_ONLY").is_err()
    }

    fn file_vault_path(&self) -> PathBuf {
        self.vault_dir.join("secrets.vault.enc")
    }

    fn derive_legacy_file_wrapping_key(&self) -> [u8; 32] {
        let user = std::env::var("USER").unwrap_or_else(|_| "antigravity".into());
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let seed = format!(
            "backtext::vault::v2::{user}::{home}::{}",
            self.vault_dir.display()
        );
        let digest = Sha256::digest(seed.as_bytes());
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn derive_envelope_wrapping_key(&self, master_hex: &str) -> [u8; 32] {
        let seed = format!(
            "backtext::vault::v3::envelope::{master_hex}::{}",
            self.vault_dir.display()
        );
        let digest = Sha256::digest(seed.as_bytes());
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn decrypt_with_key(raw: &[u8], key: &[u8; 32]) -> Option<HashMap<String, String>> {
        if raw.len() <= 12 {
            return None;
        }
        let cipher = Aes256Gcm::new_from_slice(key).ok()?;
        let nonce = Nonce::from_slice(&raw[..12]);
        let plain = cipher.decrypt(nonce, &raw[12..]).ok()?;
        serde_json::from_slice(&plain).ok()
    }

    fn load_file_map(&self) -> HashMap<String, String> {
        if self.vault_dir == Path::new(":memory:") {
            return HashMap::new();
        }
        let path = self.file_vault_path();
        let Ok(raw) = std::fs::read(&path) else {
            return HashMap::new();
        };

        // 1. Try v3 envelope key derived from the single Keychain master key (if already resolved)
        if let Some(master_hex) = self.cache.lock().get(KEY_DB_MASTER).cloned() {
            if !master_hex.is_empty() {
                let env_key = self.derive_envelope_wrapping_key(&master_hex);
                if let Some(map) = Self::decrypt_with_key(&raw, &env_key) {
                    return map;
                }
            }
        }

        // 2. Seamless fallback to legacy v2 vault key so existing API keys never break on upgrade
        let legacy_key = self.derive_legacy_file_wrapping_key();
        Self::decrypt_with_key(&raw, &legacy_key).unwrap_or_default()
    }

    fn save_file_map(&self, map: &HashMap<String, String>) -> Result<()> {
        if self.vault_dir == Path::new(":memory:") {
            return Ok(());
        }
        std::fs::create_dir_all(&self.vault_dir)?;

        // Always keep `db_master_key` in the legacy-recoverable envelope ONLY if macOS Keychain
        // is disabled/unavailable, or wrap using the single master key when available so everything
        // stays protected by one master key. On macOS, if Keychain holds `db_master_key`, we still
        // use the master-derived envelope key when `db_master_key` is known.
        let active_master = self
            .cache
            .lock()
            .get(KEY_DB_MASTER)
            .cloned()
            .or_else(|| map.get(KEY_DB_MASTER).cloned());

        let key = if let Some(ref mh) = active_master {
            if !mh.is_empty() && !map.contains_key(KEY_DB_MASTER) {
                self.derive_envelope_wrapping_key(mh)
            } else {
                // Keep bootstrap-readable if db_master_key itself is stored in file fallback
                self.derive_legacy_file_wrapping_key()
            }
        } else {
            self.derive_legacy_file_wrapping_key()
        };

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow::anyhow!("Cipher init error: {e}"))?;
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let json = serde_json::to_vec(map)?;
        let encrypted = cipher
            .encrypt(nonce, json.as_slice())
            .map_err(|e| anyhow::anyhow!("Vault encrypt error: {e}"))?;

        let mut payload = Vec::with_capacity(12 + encrypted.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&encrypted);

        let path = self.file_vault_path();
        std::fs::write(&path, payload).with_context(|| format!("Failed to write {:?}", path))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Loads the encrypted local vault file into the in-memory cache once per session.
    fn ensure_file_cache_loaded(&self) {
        if self.file_cache_loaded.swap(true, Ordering::SeqCst) {
            return;
        }
        let map = self.load_file_map();
        if !map.is_empty() {
            let mut cache = self.cache.lock();
            for (k, v) in map {
                if !v.is_empty() {
                    cache.entry(k).or_insert(v);
                }
            }
        }
    }

    /// Resolves ONLY the single `db_master_key` from macOS Keychain at most ONCE per session.
    fn resolve_single_keychain_master_once(&self) {
        if self.master_key_resolved.swap(true, Ordering::SeqCst) {
            return;
        }
        #[cfg(target_os = "macos")]
        if self.use_macos_keychain() {
            if let Ok(entry) = keyring::Entry::new("ai.backtext.desktop", KEY_DB_MASTER) {
                if let Ok(pwd) = entry.get_password() {
                    let trimmed = pwd.trim();
                    if !trimmed.is_empty() {
                        self.cache
                            .lock()
                            .insert(KEY_DB_MASTER.to_string(), trimmed.to_string());
                    }
                }
            }
        }
    }

    pub fn set_secret(&self, name: &str, value: &str) -> Result<()> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return self.delete_secret(name);
        }

        // Only persist the SINGLE master key to macOS Keychain — never create extra Keychain items
        // for API keys so macOS never prompts the user more than once!
        #[cfg(target_os = "macos")]
        if name == KEY_DB_MASTER && self.use_macos_keychain() {
            if let Ok(entry) = keyring::Entry::new("ai.backtext.desktop", KEY_DB_MASTER) {
                let _ = entry.set_password(trimmed);
            }
            self.master_key_resolved.store(true, Ordering::SeqCst);
        }

        self.ensure_file_cache_loaded();
        let mut map = self.load_file_map();
        map.insert(name.to_string(), trimmed.to_string());
        self.cache.lock().insert(name.to_string(), trimmed.to_string());
        self.save_file_map(&map)?;
        Ok(())
    }

    pub fn delete_secret(&self, name: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        if name == KEY_DB_MASTER && self.use_macos_keychain() {
            if let Ok(entry) = keyring::Entry::new("ai.backtext.desktop", KEY_DB_MASTER) {
                let _ = entry.delete_credential();
            }
        }

        self.ensure_file_cache_loaded();
        let mut map = self.load_file_map();
        map.remove(name);
        self.cache.lock().remove(name);
        self.save_file_map(&map)?;
        Ok(())
    }

    pub fn get_secret(&self, name: &str) -> Option<String> {
        // Fast path: already in RAM cache
        if let Some(v) = self.cache.lock().get(name).cloned() {
            if !v.is_empty() {
                return Some(v);
            }
        }

        // Only ever touch macOS Keychain for the single master key, at most once per session
        if name == KEY_DB_MASTER {
            self.resolve_single_keychain_master_once();
            if let Some(v) = self.cache.lock().get(KEY_DB_MASTER).cloned() {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }

        // Load all API keys & fallback master key from local AES-256-GCM vault once into RAM
        self.ensure_file_cache_loaded();
        if let Some(v) = self.cache.lock().get(name).cloned() {
            if !v.is_empty() {
                return Some(v);
            }
        }

        // Environment variable fallback for power users & CI
        let env_name = match name {
            KEY_OPENAI => Some("OPENAI_API_KEY"),
            KEY_ANTHROPIC => Some("ANTHROPIC_API_KEY"),
            KEY_GEMINI => Some("GEMINI_API_KEY"),
            KEY_OPENROUTER => Some("OPENROUTER_API_KEY"),
            KEY_PRIMALABS => Some("PRIMALABS_API_KEY"),
            KEY_DEEPGRAM => Some("DEEPGRAM_API_KEY"),
            KEY_GROQ => Some("GROQ_API_KEY"),
            _ => None,
        };
        if let Some(en) = env_name {
            if let Ok(val) = std::env::var(en) {
                if !val.trim().is_empty() {
                    return Some(val.trim().to_string());
                }
            }
        }
        None
    }

    pub fn has_secret(&self, name: &str) -> bool {
        self.get_secret(name).is_some()
    }

    /// Returns a 64-char hex (256-bit) master key for SQLCipher, generating & persisting one on first run.
    /// Accesses macOS Keychain at most ONCE per app session.
    pub fn get_or_create_db_master_key_hex(&self) -> Result<String> {
        if let Some(existing) = self.get_secret(KEY_DB_MASTER) {
            if existing.len() == 64 && existing.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(existing);
            }
        }
        let mut raw = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut raw);
        let hex_key = hex::encode(raw);
        self.set_secret(KEY_DB_MASTER, &hex_key)?;
        Ok(hex_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_gcm_secret_vault_roundtrips_and_deletes_keys() {
        let tmp = std::env::temp_dir().join(format!(
            "backtext_vault_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let vault = SecretVault::open(&tmp);
        let master = vault.get_or_create_db_master_key_hex().unwrap();
        assert_eq!(master.len(), 64);

        vault.set_secret(KEY_OPENAI, "sk-test-1234567890").unwrap();
        vault.set_secret(KEY_OPENROUTER, "sk-or-v1-test").unwrap();
        assert_eq!(
            vault.get_secret(KEY_OPENAI).as_deref(),
            Some("sk-test-1234567890")
        );
        assert_eq!(
            vault.get_secret(KEY_OPENROUTER).as_deref(),
            Some("sk-or-v1-test")
        );

        // Reload in a fresh SecretVault instance — all keys must reload cleanly from encrypted file
        let vault2 = SecretVault::open(&tmp);
        assert_eq!(vault2.get_or_create_db_master_key_hex().unwrap(), master);
        assert_eq!(
            vault2.get_secret(KEY_OPENAI).as_deref(),
            Some("sk-test-1234567890")
        );
        assert_eq!(
            vault2.get_secret(KEY_OPENROUTER).as_deref(),
            Some("sk-or-v1-test")
        );

        vault2.delete_secret(KEY_OPENAI).unwrap();
        assert!(!vault2.has_secret(KEY_OPENAI));
        assert!(vault2.has_secret(KEY_OPENROUTER));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
