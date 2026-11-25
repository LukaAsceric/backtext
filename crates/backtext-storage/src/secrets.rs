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
use std::sync::Arc;

pub const KEY_DB_MASTER: &str = "db_master_key";
pub const KEY_OPENAI: &str = "openai_api_key";
pub const KEY_ANTHROPIC: &str = "anthropic_api_key";
pub const KEY_GEMINI: &str = "gemini_api_key";
pub const KEY_DEEPGRAM: &str = "deepgram_api_key";
pub const KEY_GROQ: &str = "groq_api_key";

#[derive(Clone)]
pub struct SecretVault {
    vault_dir: PathBuf,
    backend_name: String,
    cache: Arc<Mutex<HashMap<String, String>>>,
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
        }
    }

    pub fn open_ephemeral() -> Self {
        Self {
            vault_dir: PathBuf::from(":memory:"),
            backend_name: "in-memory-ephemeral".to_string(),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn detect_backend_name() -> String {
        #[cfg(target_os = "macos")]
        {
            if std::env::var("CI").is_ok() || std::env::var("BACKTEXT_VAULT_FILE_ONLY").is_ok() {
                "AES-256-GCM Local Vault (CI)".to_string()
            } else {
                "macOS Keychain (Security.framework)".to_string()
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

    fn derive_file_wrapping_key(&self) -> [u8; 32] {
        let user = std::env::var("USER").unwrap_or_else(|_| "antigravity".into());
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let seed = format!("backtext::vault::v2::{user}::{home}::{}", self.vault_dir.display());
        let digest = Sha256::digest(seed.as_bytes());
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn load_file_map(&self) -> HashMap<String, String> {
        if self.vault_dir == Path::new(":memory:") {
            return HashMap::new();
        }
        let path = self.file_vault_path();
        let Ok(raw) = std::fs::read(&path) else {
            return HashMap::new();
        };
        if raw.len() <= 12 {
            return HashMap::new();
        }
        let key = self.derive_file_wrapping_key();
        let Ok(cipher) = Aes256Gcm::new_from_slice(&key) else {
            return HashMap::new();
        };
        let nonce = Nonce::from_slice(&raw[..12]);
        let Ok(plain) = cipher.decrypt(nonce, &raw[12..]) else {
            return HashMap::new();
        };
        serde_json::from_slice(&plain).unwrap_or_default()
    }

    fn save_file_map(&self, map: &HashMap<String, String>) -> Result<()> {
        if self.vault_dir == Path::new(":memory:") {
            return Ok(());
        }
        std::fs::create_dir_all(&self.vault_dir)?;
        let key = self.derive_file_wrapping_key();
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

    pub fn set_secret(&self, name: &str, value: &str) -> Result<()> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return self.delete_secret(name);
        }

        #[cfg(target_os = "macos")]
        if self.use_macos_keychain() {
            if let Ok(entry) = keyring::Entry::new("ai.backtext.desktop", name) {
                let _ = entry.set_password(trimmed);
            }
        }

        let mut map = self.load_file_map();
        map.insert(name.to_string(), trimmed.to_string());
        self.save_file_map(&map)?;
        self.cache.lock().insert(name.to_string(), trimmed.to_string());
        Ok(())
    }

    pub fn delete_secret(&self, name: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        if self.use_macos_keychain() {
            if let Ok(entry) = keyring::Entry::new("ai.backtext.desktop", name) {
                let _ = entry.delete_credential();
            }
        }

        let mut map = self.load_file_map();
        map.remove(name);
        self.save_file_map(&map)?;
        self.cache.lock().remove(name);
        Ok(())
    }

    pub fn get_secret(&self, name: &str) -> Option<String> {
        if let Some(v) = self.cache.lock().get(name).cloned() {
            if !v.is_empty() {
                return Some(v);
            }
        }

        #[cfg(target_os = "macos")]
        if self.use_macos_keychain() {
            if let Ok(entry) = keyring::Entry::new("ai.backtext.desktop", name) {
                if let Ok(pwd) = entry.get_password() {
                    if !pwd.trim().is_empty() {
                        self.cache.lock().insert(name.to_string(), pwd.clone());
                        return Some(pwd);
                    }
                }
            }
        }

        let map = self.load_file_map();
        if let Some(v) = map.get(name).cloned() {
            if !v.is_empty() {
                self.cache.lock().insert(name.to_string(), v.clone());
                return Some(v);
            }
        }

        // Environment variable fallback for power users & CI
        let env_name = match name {
            KEY_OPENAI => Some("OPENAI_API_KEY"),
            KEY_ANTHROPIC => Some("ANTHROPIC_API_KEY"),
            KEY_GEMINI => Some("GEMINI_API_KEY"),
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
        let tmp = std::env::temp_dir().join(format!("backtext_vault_{}", uuid::Uuid::new_v4().simple()));
        let vault = SecretVault::open(&tmp);
        vault.set_secret(KEY_ANTHROPIC, "sk-ant-test-999").unwrap();
        assert_eq!(vault.get_secret(KEY_ANTHROPIC).as_deref(), Some("sk-ant-test-999"));

        // Reopen fresh vault instance from disk to verify AES-GCM decrypt
        let vault2 = SecretVault::open(&tmp);
        assert_eq!(vault2.get_secret(KEY_ANTHROPIC).as_deref(), Some("sk-ant-test-999"));
        vault2.delete_secret(KEY_ANTHROPIC).unwrap();
        assert!(!vault2.has_secret(KEY_ANTHROPIC));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
