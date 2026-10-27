//! Configurazione locale: account e impostazioni in JSON, segreti nel portachiavi di sistema.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "it.keynet.cobalto";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
    pub username: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    /// "ssl" (porta 465) oppure "starttls" (porta 587)
    pub smtp_security: String,
    #[serde(default)]
    pub signature: String,
    /// "password" oppure "oauth-microsoft" / "oauth-google"
    #[serde(default = "default_auth")]
    pub auth: String,
    /// Casella condivisa: id dell'account con cui si accede (se ne usano i token)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_of: Option<String>,
}

fn default_auth() -> String {
    "password".into()
}

impl Account {
    pub fn is_oauth(&self) -> bool {
        self.auth.starts_with("oauth-")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "anthropic" | "openai" | "local"
    pub ai_provider: String,
    pub anthropic_model: String,
    pub openai_model: String,
    /// Permette endpoint compatibili OpenAI (Azure, Ollama, LM Studio…)
    pub openai_base_url: String,
    pub ai_language: String,
    /// AI locale con Ollama: niente crediti, i dati non escono dal Mac
    pub local_model: String,
    pub local_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_provider: "anthropic".into(),
            anthropic_model: "claude-sonnet-5-5".into(),
            openai_model: "gpt-5".into(),
            openai_base_url: "https://api.openai.com/v1".into(),
            ai_language: "italiano".into(),
            local_model: "gemma4:26b".into(),
            local_url: "http://localhost:11434".into(),
        }
    }
}

pub struct Store {
    config_dir: PathBuf,
    cache_dir: PathBuf,
}

impl Store {
    pub fn new(config_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&config_dir);
        let _ = std::fs::create_dir_all(&cache_dir);
        Self { config_dir, cache_dir }
    }

    fn read_json<T: for<'de> Deserialize<'de> + Default>(&self, file: &str) -> T {
        std::fs::read(self.config_dir.join(file))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_json<T: Serialize>(&self, file: &str, value: &T) -> Result<()> {
        let path = self.config_dir.join(file);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn accounts(&self) -> Vec<Account> {
        self.read_json("accounts.json")
    }

    pub fn account(&self, id: &str) -> Result<Account> {
        self.accounts()
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| crate::error::AppError::msg("Account non trovato"))
    }

    pub fn save_account(&self, account: Account) -> Result<()> {
        let mut all = self.accounts();
        match all.iter_mut().find(|a| a.id == account.id) {
            Some(a) => *a = account,
            None => all.push(account),
        }
        self.write_json("accounts.json", &all)
    }

    pub fn remove_account(&self, id: &str) -> Result<()> {
        // Con un account se ne vanno anche le sue caselle condivise
        let all: Vec<Account> = self
            .accounts()
            .into_iter()
            .filter(|a| a.id != id && a.shared_of.as_deref() != Some(id))
            .collect();
        let _ = delete_secret(&format!("account:{id}"));
        crate::oauth::forget(id);
        let _ = std::fs::remove_dir_all(self.cache_dir.join(id));
        self.write_json("accounts.json", &all)
    }

    pub fn settings(&self) -> Settings {
        self.read_json("settings.json")
    }

    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        self.write_json("settings.json", s)
    }

    /// Cache su disco della prima pagina di ogni cartella: l'app si apre istantaneamente.
    pub fn cache_path(&self, account_id: &str, folder: &str) -> PathBuf {
        let safe: String = folder
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        self.cache_dir.join(account_id).join(format!("{safe}.json"))
    }

    pub fn read_cache<T: for<'de> Deserialize<'de>>(&self, account_id: &str, folder: &str) -> Option<T> {
        let bytes = std::fs::read(self.cache_path(account_id, folder)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write_cache<T: Serialize>(&self, account_id: &str, folder: &str, value: &T) {
        let path = self.cache_path(account_id, folder);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(value) {
            let _ = std::fs::write(path, bytes);
        }
    }
}

pub fn set_secret(key: &str, value: &str) -> Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, key)?.set_password(value)?;
    Ok(())
}

pub fn get_secret(key: &str) -> Result<String> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, key)?.get_password()?)
}

pub fn has_secret(key: &str) -> bool {
    get_secret(key).map(|s| !s.is_empty()).unwrap_or(false)
}

pub fn delete_secret(key: &str) -> Result<()> {
    keyring::Entry::new(KEYRING_SERVICE, key)?.delete_credential()?;
    Ok(())
}
