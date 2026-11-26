use anyhow::Result;
use backtext_core::{BacktextEvent, EventBus};
use serde_json::Value;

pub const PERSISTENT_SIGNING_IDENTITY: &str = "backtext Local Signer";

/// Checks whether the user's persistent local macOS Code-Signing certificate
/// (`backtext Local Signer`) is installed in the Login Keychain so TCC
/// permissions (Accessibility, Screen Recording, Microphone) survive in-app updates.
pub fn is_persistent_signing_ready() -> bool {
    #[cfg(target_os = "macos")]
    {
        let Ok(out) = std::process::Command::new("/usr/bin/security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
        else {
            return false;
        };
        if !out.status.success() {
            return false;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout.contains(PERSISTENT_SIGNING_IDENTITY)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
async fn run_cmd_checked(program: &str, args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        anyhow::bail!(
            "{} {:?} fehlgeschlagen: {}",
            program,
            args,
            if !stderr.is_empty() { stderr } else { stdout }
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
async fn codesign_app_bundle(app_path: &std::path::Path, identity: &str) -> Result<()> {
    let macos_dir = app_path.join("Contents").join("MacOS");
    for helper in [
        "backtext-macos-sensor",
        "backtext-mcp",
        "whisper-cli",
        "whisper-server",
    ] {
        let p = macos_dir.join(helper);
        if p.exists() {
            let p_str = p.to_string_lossy().to_string();
            let _ = run_cmd_checked("/usr/bin/codesign", &["--force", "--sign", identity, &p_str])
                .await;
        }
    }
    let app_str = app_path.to_string_lossy().to_string();
    run_cmd_checked(
        "/usr/bin/codesign",
        &["--force", "--deep", "--sign", identity, &app_str],
    )
    .await?;
    Ok(())
}

/// Creates (if missing) a persistent self-signed macOS Code-Signing certificate
/// (`backtext Local Signer`) inside the user's Login Keychain and re-signs
/// `/Applications/backtext.app` so macOS TCC preserves all permissions across future updates.
pub async fn ensure_persistent_signing_cert_and_sign_current() -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        let mut created = false;
        if !is_persistent_signing_ready() {
            let home = std::env::var("HOME")
                .map_err(|_| anyhow::anyhow!("HOME Umgebungsvariable nicht gefunden"))?;
            let keychain = format!("{home}/Library/Keychains/login.keychain-db");
            let tmp_dir = std::env::temp_dir()
                .join(format!("backtext-cert-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&tmp_dir)?;

            let cnf_path = tmp_dir.join("codesign.cnf");
            let key_path = tmp_dir.join("signer.key.pem");
            let cert_path = tmp_dir.join("signer.cert.pem");
            let p12_path = tmp_dir.join("signer.p12");

            let cnf = format!(
                "[req]\n\
distinguished_name = req_distinguished_name\n\
x509_extensions = v3_codesign\n\
prompt = no\n\n\
[req_distinguished_name]\n\
CN = {PERSISTENT_SIGNING_IDENTITY}\n\n\
[v3_codesign]\n\
basicConstraints = critical, CA:FALSE\n\
keyUsage = critical, digitalSignature\n\
extendedKeyUsage = critical, codeSigning\n"
            );
            std::fs::write(&cnf_path, cnf)?;

            let cnf_s = cnf_path.to_string_lossy().to_string();
            let key_s = key_path.to_string_lossy().to_string();
            let cert_s = cert_path.to_string_lossy().to_string();
            let p12_s = p12_path.to_string_lossy().to_string();

            let gen_res = async {
                run_cmd_checked(
                    "/usr/bin/openssl",
                    &[
                        "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "3650",
                        "-config", &cnf_s, "-keyout", &key_s, "-out", &cert_s,
                    ],
                )
                .await?;
                run_cmd_checked(
                    "/usr/bin/openssl",
                    &[
                        "pkcs12", "-export", "-inkey", &key_s, "-in", &cert_s, "-out", &p12_s,
                        "-passout", "pass:backtext-local",
                    ],
                )
                .await?;
                run_cmd_checked(
                    "/usr/bin/security",
                    &[
                        "import", &p12_s, "-k", &keychain, "-P", "backtext-local",
                        "-T", "/usr/bin/codesign", "-T", "/usr/bin/security",
                    ],
                )
                .await?;
                run_cmd_checked(
                    "/usr/bin/security",
                    &[
                        "add-trusted-cert", "-r", "trustRoot", "-p", "codeSign",
                        "-k", &keychain, &cert_s,
                    ],
                )
                .await?;
                Ok::<(), anyhow::Error>(())
            }
            .await;

            let _ = std::fs::remove_dir_all(&tmp_dir);
            gen_res?;
            created = true;
        }

        let app_path = std::path::PathBuf::from("/Applications/backtext.app");
        let mut resigned_app = false;
        if app_path.exists() && is_persistent_signing_ready() {
            let _ = run_cmd_checked("/usr/bin/xattr", &["-cr", "/Applications/backtext.app"]).await;
            codesign_app_bundle(&app_path, PERSISTENT_SIGNING_IDENTITY).await?;
            resigned_app = true;
        }

        Ok(serde_json::json!({
            "ok": true,
            "ready": is_persistent_signing_ready(),
            "created": created,
            "resigned_app": resigned_app,
            "identity": PERSISTENT_SIGNING_IDENTITY,
            "message": if resigned_app {
                "Lokales Signatur-Zertifikat aktiv und /Applications/backtext.app signiert. Künftige In-App-Updates behalten alle macOS-Berechtigungen."
            } else {
                "Lokales Signatur-Zertifikat im Schlüsselbund angelegt."
            }
        }))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(serde_json::json!({
            "ok": false,
            "ready": false,
            "created": false,
            "resigned_app": false,
            "identity": PERSISTENT_SIGNING_IDENTITY,
            "message": "Lokale Code-Signatur ist nur auf macOS verfügbar."
        }))
    }
}

fn emit_progress(events: &EventBus, stage: &str, pct: u8, msg: &str, error: Option<String>) {
    events.publish(BacktextEvent::UpdateInstallProgress {
        stage: stage.to_string(),
        progress_pct: pct,
        message: msg.to_string(),
        error,
    });
}

/// Downloads a macOS `.app.zip` release from GitHub Releases, strips quarantine,
/// signs with the user's persistent `backtext Local Signer` identity (when installed),
/// atomically updates `/Applications/backtext.app`, and schedules a clean relaunch.
pub async fn download_and_install_update(
    http: &reqwest::Client,
    events: &EventBus,
    download_url: &str,
    asset_name: Option<&str>,
) -> Result<Value> {
    let url = download_url.trim();
    if url.is_empty() || !url.starts_with("https://") {
        let msg = "Ungültige Release-Download-URL.".to_string();
        emit_progress(events, "error", 0, &msg, Some(msg.clone()));
        anyhow::bail!(msg);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (http, asset_name);
        let msg = "Der automatische 1-Klick-Installer ist für macOS (.app.zip) ausgelegt.".to_string();
        emit_progress(events, "error", 0, &msg, Some(msg.clone()));
        return Ok(serde_json::json!({
            "ok": false,
            "relaunching": false,
            "message": msg
        }));
    }

    #[cfg(target_os = "macos")]
    {
        use tokio::io::AsyncWriteExt;

        let label = asset_name.unwrap_or("backtext_macos_arm64.app.zip");
        emit_progress(
            events,
            "downloading",
            3,
            &format!("Lade {label} herunter…"),
            None,
        );

        let work_dir = std::env::temp_dir()
            .join(format!("backtext-update-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&work_dir)?;
        let zip_path = work_dir.join("update.zip");
        let extract_dir = work_dir.join("extracted");
        std::fs::create_dir_all(&extract_dir)?;

        let result: Result<Value> = async {
            let mut resp = http
                .get(url)
                .header("User-Agent", "backtext-desktop")
                .timeout(std::time::Duration::from_secs(300))
                .send()
                .await?;
            if !resp.status().is_success() {
                anyhow::bail!("Download fehlgeschlagen (HTTP {})", resp.status());
            }

            let total_bytes = resp.content_length().unwrap_or(0);
            let mut file = tokio::fs::File::create(&zip_path).await?;
            let mut downloaded: u64 = 0;
            let mut last_pct: u8 = 3;

            while let Some(chunk) = resp.chunk().await? {
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;
                if total_bytes > 0 {
                    let pct = ((downloaded as f64 / total_bytes as f64) * 65.0) as u8 + 5;
                    let pct = pct.min(70);
                    if pct >= last_pct + 4 {
                        last_pct = pct;
                        let mb_done = downloaded as f64 / (1024.0 * 1024.0);
                        let mb_tot = total_bytes as f64 / (1024.0 * 1024.0);
                        emit_progress(
                            events,
                            "downloading",
                            pct,
                            &format!("Lade herunter… {mb_done:.1} / {mb_tot:.1} MB"),
                            None,
                        );
                    }
                }
            }
            file.flush().await?;
            drop(file);

            emit_progress(events, "extracting", 75, "Entpacke macOS App-Bundle…", None);
            let zip_s = zip_path.to_string_lossy().to_string();
            let ext_s = extract_dir.to_string_lossy().to_string();
            run_cmd_checked("/usr/bin/ditto", &["-x", "-k", &zip_s, &ext_s]).await?;

            // Locate extracted `backtext.app` (either directly or inside one subfolder)
            let direct_app = extract_dir.join("backtext.app");
            let staged_app = if direct_app.exists() {
                direct_app
            } else {
                std::fs::read_dir(&extract_dir)?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .find(|p| p.extension().and_then(|s| s.to_str()) == Some("app"))
                    .ok_or_else(|| anyhow::anyhow!("Kein backtext.app Bundle im Archiv gefunden"))?
            };

            emit_progress(
                events,
                "extracting",
                82,
                "Entferne macOS Quarantine-Flag…",
                None,
            );
            let staged_s = staged_app.to_string_lossy().to_string();
            let _ = run_cmd_checked("/usr/bin/xattr", &["-cr", &staged_s]).await;

            let identity = if is_persistent_signing_ready() {
                PERSISTENT_SIGNING_IDENTITY
            } else {
                "-"
            };
            emit_progress(
                events,
                "signing",
                88,
                if identity == PERSISTENT_SIGNING_IDENTITY {
                    "Signiere mit lokalem Zertifikat (Berechtigungen bleiben erhalten)…"
                } else {
                    "Signiere App-Bundle lokal…"
                },
                None,
            );
            codesign_app_bundle(&staged_app, identity).await?;

            emit_progress(
                events,
                "installing",
                94,
                "Installiere in /Applications/backtext.app…",
                None,
            );
            let target_app = std::path::PathBuf::from("/Applications/backtext.app");
            let backup_app = std::path::PathBuf::from("/Applications/.backtext-previous.app");
            if backup_app.exists() {
                let _ = std::fs::remove_dir_all(&backup_app);
            }
            let had_existing = target_app.exists();
            if had_existing {
                if let Err(err) = std::fs::rename(&target_app, &backup_app) {
                    anyhow::bail!(
                        "Konnte /Applications/backtext.app nicht ersetzen (Schreibrechte prüfen): {err}"
                    );
                }
            }
            let target_s = target_app.to_string_lossy().to_string();
            if let Err(err) = run_cmd_checked("/usr/bin/ditto", &[&staged_s, &target_s]).await {
                // Roll back cleanly if copy fails
                if had_existing && backup_app.exists() {
                    let _ = std::fs::rename(&backup_app, &target_app);
                }
                anyhow::bail!("Installation fehlgeschlagen (Rollback durchgeführt): {err}");
            }
            if backup_app.exists() {
                let _ = std::fs::remove_dir_all(&backup_app);
            }
            let _ = run_cmd_checked("/usr/bin/xattr", &["-cr", &target_s]).await;

            emit_progress(
                events,
                "restarting",
                100,
                "Update installiert! backtext startet neu…",
                None,
            );

            let _ = std::process::Command::new("/bin/sh")
                .args(["-c", "sleep 1.2 && open -n /Applications/backtext.app"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            Ok(serde_json::json!({
                "ok": true,
                "relaunching": true,
                "signed_with": identity,
                "persistent_permissions_preserved": identity == PERSISTENT_SIGNING_IDENTITY
            }))
        }
        .await;

        let _ = std::fs::remove_dir_all(&work_dir);

        if let Err(ref err) = result {
            emit_progress(
                events,
                "error",
                0,
                &format!("Update fehlgeschlagen: {err}"),
                Some(err.to_string()),
            );
        }
        result
    }
}
