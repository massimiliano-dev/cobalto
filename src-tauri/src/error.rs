use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("IMAP: {0}")]
    Imap(#[from] async_imap::error::Error),
    #[error("Connessione: {0}")]
    Io(#[from] std::io::Error),
    #[error("TLS: {0}")]
    Tls(#[from] async_native_tls::Error),
    #[error("Invio: {0}")]
    Smtp(#[from] lettre::transport::smtp::Error),
    #[error("Messaggio non valido: {0}")]
    Mail(#[from] lettre::error::Error),
    #[error("Indirizzo non valido: {0}")]
    Address(#[from] lettre::address::AddressError),
    #[error("Rete: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Portachiavi: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Dati: {0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }
}

// Il frontend riceve sempre una stringa leggibile
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
