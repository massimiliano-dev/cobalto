//! Accesso OAuth2 (Microsoft, Google) per IMAP/SMTP con XOAUTH2.
//!
//! Flusso da app desktop: PKCE + redirect su loopback (127.0.0.1, porta casuale),
//! come raccomandato da RFC 8252. I token stanno nel portachiavi di sistema.

use crate::error::{AppError, Result};
use crate::store::{self, Account};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

// Gli ID client si configurano a compile time (vedi README → OAuth).
const MS_CLIENT_ID: Option<&str> = option_env!("COBALTO_MS_CLIENT_ID");
const GOOGLE_CLIENT_ID: Option<&str> = option_env!("COBALTO_GOOGLE_CLIENT_ID");
// Per le app desktop Google il "secret" non è segreto (lo dice Google stessa), ma va inviato.
const GOOGLE_CLIENT_SECRET: Option<&str> = option_env!("COBALTO_GOOGLE_CLIENT_SECRET");

pub struct Provider {
    pub id: &'static str,
    auth_url: &'static str,
    token_url: &'static str,
    scope: &'static str,
    client_id: Option<&'static str>,
    client_secret: Option<&'static str>,
    pub imap_host: &'static str,
    pub smtp_host: &'static str,
    pub smtp_port: u16,
    pub smtp_security: &'static str,
}

const MICROSOFT: Provider = Provider {
    id: "microsoft",
    auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    scope: "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid email profile",
    client_id: MS_CLIENT_ID,
    client_secret: None,
    imap_host: "outlook.office365.com",
    smtp_host: "smtp.office365.com",
    smtp_port: 587,
    smtp_security: "starttls",
};

const GOOGLE: Provider = Provider {
    id: "google",
    auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
    token_url: "https://oauth2.googleapis.com/token",
    scope: "https://mail.google.com/ https://www.googleapis.com/auth/calendar.readonly https://www.googleapis.com/auth/contacts.readonly https://www.googleapis.com/auth/contacts.other.readonly openid email profile",
    client_id: GOOGLE_CLIENT_ID,
    client_secret: GOOGLE_CLIENT_SECRET,
    imap_host: "imap.gmail.com",
    smtp_host: "smtp.gmail.com",
    smtp_port: 465,
    smtp_security: "ssl",
};

fn configured(v: Option<&'static str>) -> Option<&'static str> {
    v.filter(|s| !s.trim().is_empty())
}

pub fn provider(id: &str) -> Result<&'static Provider> {
    match id {
        "microsoft" => Ok(&MICROSOFT),
        "google" => Ok(&GOOGLE),
        _ => Err(AppError::msg("Provider OAuth sconosciuto")),
    }
}

/// Quali accessi OAuth sono disponibili in questa build.
pub fn available() -> Vec<&'static str> {
    [&MICROSOFT, &GOOGLE]
        .into_iter()
        .filter(|p| configured(p.client_id).is_some())
        .map(|p| p.id)
        .collect()
}

#[derive(Serialize, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn secret_key(account_id: &str) -> String {
    format!("oauth:{account_id}")
}

fn random_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| url_decode(v))
    })
}

const DONE_PAGE: &str = r#"<!doctype html><html lang="it"><meta charset="utf-8"><title>Cobalto</title>
<body style="margin:0;height:100vh;display:grid;place-items:center;background:#1c2f8f;color:#f4f7ff;font-family:-apple-system,'Segoe UI',sans-serif">
<div style="text-align:center"><div style="font-family:Georgia,serif;font-size:42px;margin-bottom:8px">Fatto.</div>
<div style="opacity:.8">Puoi chiudere questa scheda e tornare a Cobalto.</div></div></body></html>"#;

const FAIL_PAGE: &str = r#"<!doctype html><html lang="it"><meta charset="utf-8"><title>Cobalto</title>
<body style="margin:0;height:100vh;display:grid;place-items:center;background:#f4f7ff;color:#1c2f8f;font-family:-apple-system,'Segoe UI',sans-serif">
<div style="text-align:center"><div style="font-family:Georgia,serif;font-size:36px;margin-bottom:8px">Accesso non completato</div>
<div style="opacity:.8">Torna a Cobalto e riprova.</div></div></body></html>"#;

/// Attende la redirect del browser sulla porta di loopback e restituisce la query string.
async fn wait_for_redirect(listener: TcpListener) -> Result<String> {
    loop {
        let (mut sock, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let n = sock.read(&mut buf).await?;
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req.split_whitespace().nth(1).unwrap_or("");
        // Il browser può chiedere anche /favicon.ico: si ignora
        let Some(query) = path.strip_prefix("/?") else {
            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
            continue;
        };
        let ok = query_param(query, "code").is_some();
        let page = if ok { DONE_PAGE } else { FAIL_PAGE };
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.shutdown().await;
        return Ok(query.to_string());
    }
}

async fn token_request(p: &Provider, form: &[(&str, &str)]) -> Result<Value> {
    let mut params: Vec<(&str, &str)> = form.to_vec();
    let client_id = configured(p.client_id).ok_or_else(|| AppError::msg("OAuth non configurato"))?;
    params.push(("client_id", client_id));
    if let Some(secret) = configured(p.client_secret) {
        params.push(("client_secret", secret));
    }
    let body = params
        .iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let resp = reqwest::Client::new()
        .post(p.token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;
    let status = resp.status();
    let v: Value = resp.json().await?;
    if !status.is_success() {
        let msg = v["error_description"]
            .as_str()
            .or_else(|| v["error"].as_str())
            .unwrap_or("errore sconosciuto");
        return Err(AppError::msg(format!("Accesso rifiutato: {}", msg.lines().next().unwrap_or(msg))));
    }
    Ok(v)
}

/// Legge email e nome dall'id_token. Arriva direttamente dal token endpoint via TLS,
/// quindi non serve verificarne la firma.
fn identity(v: &Value) -> (String, String) {
    let claims: Value = v["id_token"]
        .as_str()
        .and_then(|t| t.split('.').nth(1))
        .and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let email = claims["email"]
        .as_str()
        .or_else(|| claims["preferred_username"].as_str())
        .unwrap_or_default()
        .to_string();
    let name = claims["name"].as_str().unwrap_or_default().to_string();
    (email, name)
}

fn store_tokens(account_id: &str, v: &Value, previous_refresh: Option<&str>) -> Result<String> {
    let access = v["access_token"]
        .as_str()
        .ok_or_else(|| AppError::msg("Risposta OAuth senza access token"))?
        .to_string();
    let refresh = v["refresh_token"]
        .as_str()
        .or(previous_refresh)
        .ok_or_else(|| AppError::msg("Risposta OAuth senza refresh token"))?
        .to_string();
    let tokens = Tokens {
        access_token: access.clone(),
        refresh_token: refresh,
        expires_at: now() + v["expires_in"].as_u64().unwrap_or(3600),
    };
    store::set_secret(&secret_key(account_id), &serde_json::to_string(&tokens)?)?;
    Ok(access)
}

pub struct Authorized {
    pub account: Account,
    pub access_token: String,
}

/// Apre il browser per il consenso e restituisce un account pronto (non ancora salvato).
/// Messaggio d'errore riservato all'annullamento: il frontend lo riconosce e non lo mostra.
pub const CANCELLED: &str = "Accesso annullato";

pub async fn authorize(
    provider_id: &str,
    open: impl FnOnce(&str) -> Result<()>,
    cancel: tokio::sync::oneshot::Receiver<()>,
) -> Result<Authorized> {
    let p = provider(provider_id)?;
    let client_id = configured(p.client_id).ok_or_else(|| {
        AppError::msg("Accesso OAuth non configurato in questa build: vedi README → OAuth")
    })?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    // Microsoft accetta "http://localhost" registrato senza porta: la porta è libera
    let redirect = format!("http://localhost:{}", listener.local_addr()?.port());
    let verifier = random_token();
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let state = random_token();

    let mut url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        p.auth_url,
        url_encode(client_id),
        url_encode(&redirect),
        url_encode(p.scope),
        state,
        challenge
    );
    url.push_str(if p.id == "google" {
        "&access_type=offline&prompt=consent"
    } else {
        "&prompt=select_account"
    });
    open(&url)?;

    // Si smette di aspettare il browser se l'utente annulla, se avvia un nuovo accesso
    // (il vecchio canale si chiude) o dopo 5 minuti
    let query = tokio::select! {
        r = wait_for_redirect(listener) => r?,
        _ = cancel => return Err(AppError::msg(CANCELLED)),
        _ = tokio::time::sleep(Duration::from_secs(300)) => {
            return Err(AppError::msg("Tempo scaduto: l'accesso nel browser non è stato completato"))
        }
    };
    if query_param(&query, "state").as_deref() != Some(state.as_str()) {
        return Err(AppError::msg("Risposta OAuth non valida (state)"));
    }
    if let Some(err) = query_param(&query, "error_description").or_else(|| query_param(&query, "error")) {
        return Err(AppError::msg(format!("Accesso annullato: {err}")));
    }
    let code = query_param(&query, "code").ok_or_else(|| AppError::msg("Nessun codice di autorizzazione"))?;

    let v = token_request(
        p,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("code_verifier", &verifier),
        ],
    )
    .await?;
    let (email, name) = identity(&v);
    if email.is_empty() {
        return Err(AppError::msg("Il provider non ha restituito l'indirizzo email"));
    }

    let account = Account {
        id: uuid::Uuid::new_v4().to_string(),
        name: if name.is_empty() { email.split('@').next().unwrap_or_default().to_string() } else { name },
        email: email.clone(),
        username: email,
        imap_host: p.imap_host.into(),
        imap_port: 993,
        smtp_host: p.smtp_host.into(),
        smtp_port: p.smtp_port,
        smtp_security: p.smtp_security.into(),
        signature: String::new(),
        auth: format!("oauth-{}", p.id),
        shared_of: None,
    };
    let access_token = store_tokens(&account.id, &v, None)?;
    Ok(Authorized { account, access_token })
}

// Un solo refresh alla volta: le due corsie IMAP e l'SMTP possono chiederlo insieme
static REFRESH: Mutex<()> = Mutex::const_new(());

/// Access token valido per l'account, rinnovato se in scadenza.
/// Di chi sono i token: le caselle condivise usano quelli dell'account principale.
fn token_owner(acc: &Account) -> &str {
    acc.shared_of.as_deref().unwrap_or(&acc.id)
}

pub async fn access_token(acc: &Account) -> Result<String> {
    let provider_id = acc
        .auth
        .strip_prefix("oauth-")
        .ok_or_else(|| AppError::msg("L'account non usa OAuth"))?;
    let p = provider(provider_id)?;
    let _guard = REFRESH.lock().await;
    let raw = store::get_secret(&secret_key(token_owner(acc)))
        .map_err(|_| AppError::msg("Accesso scaduto: ricollega l'account"))?;
    let tokens: Tokens = serde_json::from_str(&raw)?;
    if tokens.expires_at > now() + 120 {
        return Ok(tokens.access_token);
    }
    let v = token_request(
        p,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &tokens.refresh_token),
            ("scope", p.scope),
        ],
    )
    .await
    .map_err(|e| AppError::msg(format!("{e} — ricollega l'account dalle Impostazioni")))?;
    store_tokens(token_owner(acc), &v, Some(&tokens.refresh_token))
}

// Invio (Mail.Send) e allegati grandi su OneDrive (Files.ReadWrite)
const GRAPH_SCOPE: &str = "https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/Files.ReadWrite https://graph.microsoft.com/Calendars.Read offline_access";

#[derive(Serialize, Deserialize)]
struct CachedToken {
    access_token: String,
    expires_at: u64,
}

// Ambiti Graph per caselle condivise (invio "come" la casella) e per trovarle nella directory
const SHARED_GRAPH_SCOPE: &str = "https://graph.microsoft.com/Mail.Send.Shared https://graph.microsoft.com/Files.ReadWrite offline_access";
const DIRECTORY_SCOPE: &str = "https://graph.microsoft.com/User.Read.All https://graph.microsoft.com/Mail.Read.Shared offline_access";

fn graph_key(tag: &str, account_id: &str) -> String {
    format!("oauth-graph-{tag}:{account_id}")
}

/// Token per Microsoft Graph (invio, OneDrive, calendario), ottenuto dallo stesso refresh token:
/// così si può inviare anche dove l'SMTP autenticato è disattivato.
pub async fn graph_send_token(acc: &Account) -> Result<String> {
    if acc.shared_of.is_some() {
        graph_token(acc, SHARED_GRAPH_SCOPE, "shared").await
    } else {
        graph_token(acc, GRAPH_SCOPE, "v3").await
    }
}

/// Token per i suggerimenti dei destinatari (rubrica, colleghi, contatti frequenti).
pub async fn people_token(acc: &Account) -> Result<String> {
    graph_token(acc, "https://graph.microsoft.com/People.Read offline_access", "people").await
}

/// Token per cercare le caselle condivise a cui l'utente ha accesso.
pub async fn directory_token(acc: &Account) -> Result<String> {
    graph_token(acc, DIRECTORY_SCOPE, "dir").await
}

async fn graph_token(acc: &Account, scope: &str, tag: &str) -> Result<String> {
    if acc.auth != "oauth-microsoft" {
        return Err(AppError::msg("Graph è disponibile solo per gli account Microsoft"));
    }
    let _guard = REFRESH.lock().await;
    let cache = graph_key(tag, &acc.id);
    if let Some(cached) = store::get_secret(&cache)
        .ok()
        .and_then(|raw| serde_json::from_str::<CachedToken>(&raw).ok())
        .filter(|t| t.expires_at > now() + 120)
    {
        return Ok(cached.access_token);
    }
    let owner = token_owner(acc);
    let raw = store::get_secret(&secret_key(owner))?;
    let mut tokens: Tokens = serde_json::from_str(&raw)?;
    let v = token_request(
        &MICROSOFT,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &tokens.refresh_token),
            ("scope", scope),
        ],
    )
    .await?;
    let access = v["access_token"]
        .as_str()
        .ok_or_else(|| AppError::msg("Risposta OAuth senza access token"))?
        .to_string();
    // Microsoft ruota il refresh token: quello nuovo vale per tutte le risorse
    if let Some(r) = v["refresh_token"].as_str() {
        tokens.refresh_token = r.to_string();
        store::set_secret(&secret_key(owner), &serde_json::to_string(&tokens)?)?;
    }
    let cached = CachedToken {
        access_token: access.clone(),
        expires_at: now() + v["expires_in"].as_u64().unwrap_or(3600),
    };
    store::set_secret(&cache, &serde_json::to_string(&cached)?)?;
    Ok(access)
}

pub fn forget(account_id: &str) {
    let _ = store::delete_secret(&secret_key(account_id));
    for tag in ["v3", "shared", "dir", "people"] {
        let _ = store::delete_secret(&graph_key(tag, account_id));
    }
    for old in ["oauth-graph", "oauth-graph-v2"] {
        let _ = store::delete_secret(&format!("{old}:{account_id}"));
    }
}

/// Stringa SASL XOAUTH2 (senza base64: ci pensano async-imap e lettre).
pub fn xoauth2(user: &str, token: &str) -> String {
    format!("user={user}\x01auth=Bearer {token}\x01\x01")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parsing() {
        let q = "code=M.C5_abc%2Fdef&state=xyz&session_state=1";
        assert_eq!(query_param(q, "code").as_deref(), Some("M.C5_abc/def"));
        assert_eq!(query_param(q, "state").as_deref(), Some("xyz"));
        assert_eq!(query_param(q, "missing"), None);
    }

    #[test]
    fn pkce_challenge_is_rfc7636() {
        // Vettore di esempio dall'appendice B di RFC 7636
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(b64url(&Sha256::digest(verifier.as_bytes())), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[tokio::test]
    async fn closing_the_pending_channel_cancels_the_wait() {
        if available().is_empty() {
            return; // build senza ID OAuth: niente da provare
        }
        let provider = available()[0];
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let mut opened = None;
        let wait = authorize(provider, |url| { opened = Some(url.to_string()); Ok(()) }, rx);
        drop(tx); // come oauth_cancel o un nuovo accesso
        let err = tokio::time::timeout(Duration::from_secs(2), wait).await.expect("non deve restare appeso");
        assert_eq!(err.err().map(|e| e.to_string()).as_deref(), Some(CANCELLED));
    }

    #[test]
    fn id_token_identity() {
        let payload = b64url(br#"{"email":"mario@outlook.com","name":"Mario Rossi"}"#);
        let v = serde_json::json!({ "id_token": format!("h.{payload}.s") });
        assert_eq!(identity(&v), ("mario@outlook.com".into(), "Mario Rossi".into()));
    }
}
