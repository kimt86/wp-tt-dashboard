//! Runs a SQL statement against Oracle via the `remote-toolbox-sql` skill.
//! This module is the ONLY path to production Oracle. Calls are serialized by a
//! process-wide async lock so two queries never hit Oracle concurrently.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use tokio::sync::Mutex;

/// Serializes all Oracle access for the lifetime of the process.
static ORACLE_LOCK: Mutex<()> = Mutex::const_new(());

pub struct Toolbox {
    skill_dir: PathBuf,
    target: String,
    timeout_secs: u64,
}

impl Toolbox {
    /// `target` is e.g. "oracle-prod" / "oracle-uat". SKILL_DIR comes from env.
    pub fn from_env(target: &str) -> Result<Self> {
        let skill_dir = std::env::var("SKILL_DIR")
            .unwrap_or_else(|_| "/home/aiadmin/.codex/skills/yard-db-ops".to_string());
        let skill_dir = PathBuf::from(skill_dir);
        let script = skill_dir.join("scripts/remote-toolbox-sql");
        if !script.exists() {
            bail!("remote-toolbox-sql not found at {}", script.display());
        }
        Ok(Self {
            skill_dir,
            target: target.to_string(),
            timeout_secs: 90,
        })
    }

    fn script(&self) -> PathBuf {
        self.skill_dir.join("scripts/remote-toolbox-sql")
    }

    /// Execute `sql` and return raw stdout (the `{"result":"..."}` envelope).
    /// The SQL is passed via a temp file (`--file`) to avoid shell-escape damage.
    pub async fn run_sql(&self, sql: &str) -> Result<String> {
        let _guard = ORACLE_LOCK.lock().await; // serialize Oracle access

        // write SQL to a temp file the script can read
        let dir = std::env::temp_dir();
        let path = dir.join(format!("wp-extract-{}.sql", std::process::id()));
        tokio::fs::write(&path, sql)
            .await
            .with_context(|| format!("writing temp SQL to {}", path.display()))?;

        let out = tokio::process::Command::new(self.script())
            .arg(&self.target)
            .arg("--file")
            .arg(&path)
            .arg("--timeout")
            .arg(self.timeout_secs.to_string())
            .output()
            .await
            .context("spawning remote-toolbox-sql")?;

        let _ = tokio::fs::remove_file(&path).await;

        if !out.status.success() {
            bail!(
                "remote-toolbox-sql failed (status {:?}): {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8(out.stdout).context("toolbox stdout was not UTF-8")?)
    }
}
