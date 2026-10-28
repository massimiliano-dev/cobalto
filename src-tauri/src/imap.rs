//! Accesso IMAP: un piccolo pool di connessioni persistenti per account.
//!
//! Ogni account ha due "corsie": `sync` (liste, cartelle, ricerca) e `ui`
//! (apertura messaggi, flag, spostamenti), così leggere una mail non aspetta mai
//! la sincronizzazione della lista.

use crate::error::{AppError, Result};
use crate::store::Account;
use async_imap::types::{Fetch, Flag, NameAttribute};
use async_native_tls::TlsStream;
use base64::Engine;
use futures::TryStreamExt;
use mail_parser::{MessageParser, MimeHeaders, PartType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio::time::timeout;

type ImapSession = async_imap::Session<TlsStream<TcpStream>>;

const LIST_ITEMS: &str = "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER] BODY.PEEK[TEXT]<0.4096>)";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Addr {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub uid: u32,
    pub subject: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub date: i64,
    pub preview: String,
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub has_attachments: bool,
    pub size: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub index: usize,
    pub name: String,
    pub mime: String,
    pub size: usize,
    pub inline: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullMessage {
    pub uid: u32,
    pub subject: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub reply_to: Vec<Addr>,
    pub date: i64,
    pub flagged: bool,
    pub html: Option<String>,
    pub text: Option<String>,
    pub attachments: Vec<Attachment>,
    pub message_id: Option<String>,
    pub references: Vec<String>,
    pub list_unsubscribe: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    /// Nome IMAP grezzo, da usare nei comandi
    pub path: String,
    /// Nome leggibile (UTF-7 modificato decodificato)
    pub name: String,
    pub depth: usize,
    /// inbox | sent | drafts | trash | junk | archive | all | flagged | ""
    pub role: String,
    pub unseen: u32,
}

#[derive(Default)]
pub struct Conn {
    session: Option<ImapSession>,
    last_used: Option<Instant>,
}

#[derive(Default)]
pub struct Pool {
    conns: Mutex<HashMap<String, Arc<Mutex<Conn>>>>,
}

impl Pool {
    pub async fn lane(&self, account_id: &str, lane: &str) -> OwnedMutexGuard<Conn> {
        let conn = {
            let mut map = self.conns.lock().await;
            map.entry(format!("{account_id}:{lane}"))
                .or_default()
                .clone()
        };
        conn.lock_owned().await
    }

    pub async fn drop_account(&self, account_id: &str) {
        let mut map = self.conns.lock().await;
        map.retain(|k, _| !k.starts_with(&format!("{account_id}:")));
    }
}

/// Credenziale pronta all'uso: password oppure access token OAuth.
pub enum Credential {
    Password(String),
    Token(String),
}

struct XOAuth2(String);

impl async_imap::Authenticator for XOAuth2 {
    type Response = String;
    fn process(&mut self, challenge: &[u8]) -> String {
        // Al primo giro si invia la stringa; se il server risponde con un errore
        // (challenge non vuota) si manda una risposta vuota per chiudere lo scambio
        if challenge.is_empty() { std::mem::take(&mut self.0) } else { String::new() }
    }
}

pub async fn credential(acc: &Account) -> Result<Credential> {
    if acc.is_oauth() {
        Ok(Credential::Token(crate::oauth::access_token(acc).await?))
    } else {
        Ok(Credential::Password(crate::store::get_secret(&format!("account:{}", acc.id))?))
    }
}

pub async fn connect(acc: &Account, cred: &Credential) -> Result<ImapSession> {
    let fut = async {
        let tcp = TcpStream::connect((acc.imap_host.as_str(), acc.imap_port)).await?;
        let tls = async_native_tls::connect(acc.imap_host.as_str(), tcp).await?;
        let mut client = async_imap::Client::new(tls);
        // Saluto iniziale del server
        client
            .read_response()
            .await?
            .ok_or_else(|| AppError::msg("Il server ha chiuso la connessione"))?;
        let session = match cred {
            Credential::Password(p) => client.login(&acc.username, p).await,
            Credential::Token(t) => {
                client
                    .authenticate("XOAUTH2", XOAuth2(crate::oauth::xoauth2(&acc.username, t)))
                    .await
            }
        }
        .map_err(|(e, _)| AppError::msg(login_hint(acc, &e.to_string())))?;
        Ok::<_, AppError>(session)
    };
    timeout(Duration::from_secs(25), fut)
        .await
        .map_err(|_| AppError::msg(format!("Timeout collegandosi a {}", acc.imap_host)))?
}

fn login_hint(acc: &Account, err: &str) -> String {
    // Gli errori async-imap sono del tipo `no response: code: Some(Alert), info: Some("…")`
    let info = err
        .split_once("info: Some(\"")
        .and_then(|(_, rest)| rest.rsplit_once("\")"))
        .map(|(text, _)| text.replace("\\\"", "\""))
        .unwrap_or_else(|| err.to_string());
    let microsoft = acc.imap_host.contains("office365") || acc.imap_host.contains("outlook");
    let google = acc.imap_host.contains("gmail");
    if google && info.contains("Application-specific password") {
        "Gmail non accetta la password normale: crea una «password per le app» su myaccount.google.com/apppasswords (serve la verifica in due passaggi) oppure usa «Accedi con Google».".into()
    } else if google && info.to_lowercase().contains("invalid credentials") {
        "Gmail ha rifiutato le credenziali: controlla la password per le app (16 lettere, senza spazi) oppure usa «Accedi con Google».".into()
    } else if microsoft && acc.is_oauth() {
        format!("Microsoft ha rifiutato l'accesso IMAP ({info}). Di solito significa che IMAP è disattivato per questa casella: va abilitato dall'amministratore di Microsoft 365.")
    } else if microsoft {
        "Microsoft non accetta più le password per IMAP, né per gli account personali né per quelli aziendali: usa «Accedi con Microsoft».".into()
    } else {
        format!("Accesso rifiutato: {info}")
    }
}

impl Conn {
    /// Restituisce una sessione valida, riconnettendo se quella esistente è caduta.
    pub async fn session(&mut self, acc: &Account) -> Result<&mut ImapSession> {
        let stale = self
            .last_used
            .map(|t| t.elapsed() > Duration::from_secs(45))
            .unwrap_or(true);
        if stale {
            if let Some(s) = self.session.as_mut() {
                let alive = matches!(timeout(Duration::from_secs(6), s.noop()).await, Ok(Ok(())));
                if !alive {
                    self.session = None;
                }
            }
        }
        if self.session.is_none() {
            let cred = credential(acc).await?;
            self.session = Some(connect(acc, &cred).await?);
        }
        self.last_used = Some(Instant::now());
        Ok(self.session.as_mut().expect("sessione appena creata"))
    }

    /// Da chiamare quando un comando fallisce: la prossima richiesta riconnette.
    pub fn invalidate(&mut self) {
        self.session = None;
    }
}

// ---------- Cartelle ----------

pub async fn folders(s: &mut ImapSession) -> Result<Vec<Folder>> {
    let names: Vec<_> = s.list(Some(""), Some("*")).await?.try_collect().await?;
    let mut out = Vec::new();
    for n in &names {
        let attrs = n.attributes();
        if attrs.iter().any(|a| matches!(a, NameAttribute::NoSelect)) {
            continue;
        }
        let path = n.name().to_string();
        let delim = n.delimiter().unwrap_or("/");
        let decoded = decode_mutf7(&path);
        let mut role = attrs
            .iter()
            .find_map(|a| match a {
                NameAttribute::Sent => Some("sent"),
                NameAttribute::Drafts => Some("drafts"),
                NameAttribute::Trash => Some("trash"),
                NameAttribute::Junk => Some("junk"),
                NameAttribute::Archive => Some("archive"),
                NameAttribute::All => Some("all"),
                NameAttribute::Flagged => Some("flagged"),
                _ => None,
            })
            .unwrap_or("")
            .to_string();
        if path.eq_ignore_ascii_case("INBOX") {
            role = "inbox".into();
        }
        let leaf = decoded.rsplit(delim).next().unwrap_or(&decoded).to_string();
        if role.is_empty() {
            role = guess_role(&leaf).into();
        }
        // "INBOX.Sent" (Aruba, Dovecot…) non è davvero annidata dal punto di vista dell'utente
        let rel = decoded
            .strip_prefix(&format!("INBOX{delim}"))
            .unwrap_or(&decoded);
        let depth = if role.is_empty() { rel.matches(delim).count() } else { 0 };
        out.push(Folder { path, name: leaf, depth, role, unseen: 0 });
    }
    // Evita doppioni di ruolo (es. due cartelle "Sent"): tiene la prima, le altre tornano normali
    let mut seen_roles = std::collections::HashSet::new();
    for f in out.iter_mut() {
        if !f.role.is_empty() && !seen_roles.insert(f.role.clone()) {
            f.role.clear();
        }
    }
    out.sort_by(|a, b| {
        role_rank(&a.role)
            .cmp(&role_rank(&b.role))
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
    // Conteggio non letti solo dove serve davvero
    for f in out.iter_mut() {
        if f.role == "inbox" {
            if let Ok(n) = unseen_count(s, &f.path).await {
                f.unseen = n;
            }
        } else if f.role.is_empty() {
            if let Ok(mb) = s.status(&f.path, "(UNSEEN)").await {
                f.unseen = mb.unseen.unwrap_or(0);
            }
        }
    }
    Ok(out)
}

fn role_rank(role: &str) -> u8 {
    match role {
        "inbox" => 0,
        "flagged" => 1,
        "drafts" => 2,
        "sent" => 3,
        "archive" => 4,
        "all" => 5,
        "junk" => 6,
        "trash" => 7,
        _ => 10,
    }
}

fn guess_role(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "sent" | "sent items" | "sent mail" | "sent messages" | "posta inviata" | "inviata" | "inviati" | "elementi inviati" => "sent",
        "drafts" | "draft" | "bozze" => "drafts",
        "trash" | "deleted" | "deleted items" | "deleted messages" | "cestino" | "posta eliminata" | "elementi eliminati" => "trash",
        "junk" | "spam" | "junk e-mail" | "junk email" | "posta indesiderata" => "junk",
        "archive" | "archives" | "archivio" => "archive",
        _ => "",
    }
}

/// Decodifica i nomi cartella in UTF-7 modificato (RFC 3501 §5.1.3).
fn decode_mutf7(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('-') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let chunk = &after[..j];
        if chunk.is_empty() {
            out.push('&');
        } else {
            let b64 = chunk.replace(',', "/");
            let decoded = base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(b64.trim_end_matches('='))
                .ok();
            match decoded {
                Some(bytes) => {
                    let units: Vec<u16> = bytes
                        .as_chunks::<2>().0.iter()
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect();
                    out.push_str(&String::from_utf16_lossy(&units));
                }
                None => out.push_str(&rest[i..i + 2 + j]),
            }
        }
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    out
}

// ---------- Messaggi ----------

fn addr_list(a: Option<&mail_parser::Address>) -> Vec<Addr> {
    a.map(|a| {
        a.iter()
            .map(|x| Addr {
                name: x.name().unwrap_or_default().trim_matches('"').to_string(),
                email: x.address().unwrap_or_default().to_string(),
            })
            .collect()
    })
    .unwrap_or_default()
}

fn clean_preview(s: &str) -> String {
    let filtered: String = s
        .chars()
        .filter(|c| !matches!(c, '\u{034F}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{00AD}' | '\u{FEFF}' | '\u{2007}'))
        .collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn envelope_from(f: &Fetch) -> Option<Envelope> {
    let uid = f.uid?;
    let mut raw = f.header().unwrap_or_default().to_vec();
    if let Some(t) = f.text() {
        raw.extend_from_slice(t);
    }
    let msg = MessageParser::default().parse(&raw)?;
    let flags: Vec<Flag> = f.flags().collect();
    let has_attachments = msg
        .content_type()
        .map(|ct| {
            ct.ctype().eq_ignore_ascii_case("multipart")
                && ct.subtype().is_some_and(|s| s.eq_ignore_ascii_case("mixed"))
        })
        .unwrap_or(false);
    let date = msg
        .date()
        .map(|d| d.to_timestamp())
        .or_else(|| f.internal_date().map(|d| d.timestamp()))
        .unwrap_or(0);
    Some(Envelope {
        uid,
        subject: msg.subject().unwrap_or_default().trim().to_string(),
        from: addr_list(msg.from()).into_iter().next().unwrap_or(Addr {
            name: String::new(),
            email: String::new(),
        }),
        to: addr_list(msg.to()),
        date,
        preview: clean_preview(&msg.body_preview(220).unwrap_or_default()),
        seen: flags.iter().any(|f| matches!(f, Flag::Seen)),
        flagged: flags.iter().any(|f| matches!(f, Flag::Flagged)),
        answered: flags.iter().any(|f| matches!(f, Flag::Answered)),
        has_attachments,
        size: f.size.unwrap_or(0),
    })
}

pub struct Page {
    pub messages: Vec<Envelope>,
    pub total: u32,
}

pub async fn list(s: &mut ImapSession, folder: &str, page: u32, size: u32) -> Result<Page> {
    let mb = s.select(folder).await?;
    let total = mb.exists;
    let end = total.saturating_sub(page * size);
    if end == 0 {
        return Ok(Page { messages: vec![], total });
    }
    let start = end.saturating_sub(size - 1).max(1);
    let fetches: Vec<Fetch> = s
        .fetch(format!("{start}:{end}"), LIST_ITEMS)
        .await?
        .try_collect()
        .await?;
    let mut messages: Vec<Envelope> = fetches.iter().filter_map(envelope_from).collect();
    messages.sort_by_key(|m| std::cmp::Reverse(m.uid));
    Ok(Page { messages, total })
}

pub async fn search(s: &mut ImapSession, acc: &Account, folder: &str, query: &str) -> Result<Vec<Envelope>> {
    s.select(folder).await?;
    let q = query.replace('\\', "\\\\").replace('"', "\\\"");
    let criteria = if acc.imap_host.contains("gmail") {
        format!("X-GM-RAW \"{q}\"")
    } else if q.is_ascii() {
        format!("TEXT \"{q}\"")
    } else {
        format!("CHARSET UTF-8 TEXT \"{q}\"")
    };
    let mut uids: Vec<u32> = s.uid_search(criteria).await?.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    uids.truncate(80);
    if uids.is_empty() {
        return Ok(vec![]);
    }
    let set = uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let fetches: Vec<Fetch> = s.uid_fetch(set, LIST_ITEMS).await?.try_collect().await?;
    let mut messages: Vec<Envelope> = fetches.iter().filter_map(envelope_from).collect();
    messages.sort_by_key(|m| std::cmp::Reverse(m.date));
    Ok(messages)
}

struct Raw {
    bytes: Vec<u8>,
    seen: bool,
    flagged: bool,
}

/// Scarica il messaggio senza toccarne i flag (BODY.PEEK).
async fn fetch_raw(s: &mut ImapSession, folder: &str, uid: u32) -> Result<Raw> {
    s.select(folder).await?;
    let fetches: Vec<Fetch> = s
        .uid_fetch(uid.to_string(), "(UID FLAGS BODY.PEEK[])")
        .await?
        .try_collect()
        .await?;
    let f = fetches
        .iter()
        .find(|f| f.uid == Some(uid) && f.body().is_some())
        .ok_or_else(|| AppError::msg("Messaggio non più disponibile sul server"))?;
    let seen = f.flags().any(|f| matches!(f, Flag::Seen));
    let flagged = f.flags().any(|f| matches!(f, Flag::Flagged));
    let bytes = f.body().unwrap_or_default().to_vec();
    Ok(Raw { bytes, seen, flagged })
}

pub async fn message(s: &mut ImapSession, folder: &str, uid: u32) -> Result<FullMessage> {
    let Raw { bytes: raw, seen, flagged } = fetch_raw(s, folder, uid).await?;
    // Segna come letto in modo esplicito: non tutti i server (Exchange in primis)
    // applicano il \Seen implicito di BODY[], e al riavvio la mail tornerebbe non letta
    if !seen {
        let _: Vec<Fetch> = s
            .uid_store(uid.to_string(), "+FLAGS.SILENT (\\Seen)")
            .await?
            .try_collect()
            .await?;
    }
    let msg = MessageParser::default()
        .parse(&raw)
        .ok_or_else(|| AppError::msg("Impossibile leggere il messaggio"))?;

    let has_real_html = msg.html_bodies().any(|p| matches!(p.body, PartType::Html(_)));
    let mut html = if has_real_html { msg.body_html(0).map(|h| h.into_owned()) } else { None };
    let text = msg.body_text(0).map(|t| t.into_owned());

    let mut attachments = Vec::new();
    for (index, part) in msg.attachments().enumerate() {
        let mime = part
            .content_type()
            .map(|ct| format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or("octet-stream")))
            .unwrap_or_else(|| "application/octet-stream".into());
        let mut inline = false;
        // Immagini incorporate: cid: → data URI, così l'HTML è autosufficiente
        if let (Some(cid), Some(h)) = (part.content_id(), html.as_mut()) {
            let cid = cid.trim_matches(|c| c == '<' || c == '>');
            let needle = format!("cid:{cid}");
            if h.contains(&needle) {
                let data = base64::engine::general_purpose::STANDARD.encode(part.contents());
                *h = h.replace(&needle, &format!("data:{mime};base64,{data}"));
                inline = true;
            }
        }
        attachments.push(Attachment {
            index,
            name: part
                .attachment_name()
                .map(str::to_string)
                .unwrap_or_else(|| format!("allegato-{}", index + 1)),
            mime,
            size: part.contents().len(),
            inline,
        });
    }

    let mut references: Vec<String> = match msg.references().as_text_list() {
        Some(list) => list.iter().map(|s| s.to_string()).collect(),
        None => msg.references().as_text().map(|s| vec![s.to_string()]).unwrap_or_default(),
    };
    if let Some(id) = msg.message_id() {
        references.push(id.to_string());
    }

    Ok(FullMessage {
        uid,
        subject: msg.subject().unwrap_or_default().trim().to_string(),
        from: addr_list(msg.from()).into_iter().next().unwrap_or(Addr {
            name: String::new(),
            email: String::new(),
        }),
        to: addr_list(msg.to()),
        cc: addr_list(msg.cc()),
        reply_to: addr_list(msg.reply_to()),
        date: msg.date().map(|d| d.to_timestamp()).unwrap_or(0),
        flagged,
        html,
        text,
        attachments,
        message_id: msg.message_id().map(str::to_string),
        references,
        list_unsubscribe: msg
            .list_unsubscribe()
            .as_text_list()
            .and_then(|l| l.first().map(|s| s.to_string()))
            .or_else(|| msg.list_unsubscribe().as_text().map(str::to_string)),
    })
}

pub async fn attachment(s: &mut ImapSession, folder: &str, uid: u32, index: usize) -> Result<(String, Vec<u8>)> {
    let raw = fetch_raw(s, folder, uid).await?.bytes;
    let msg = MessageParser::default()
        .parse(&raw)
        .ok_or_else(|| AppError::msg("Impossibile leggere il messaggio"))?;
    let part = msg
        .attachments()
        .nth(index)
        .ok_or_else(|| AppError::msg("Allegato non trovato"))?;
    let name = part
        .attachment_name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("allegato-{}", index + 1));
    Ok((name, part.contents().to_vec()))
}

/// Tutti gli allegati veri (non le immagini incorporate) per l'inoltro.
pub async fn all_attachments(s: &mut ImapSession, folder: &str, uid: u32) -> Result<Vec<crate::smtp::OutgoingAttachment>> {
    let raw = fetch_raw(s, folder, uid).await?.bytes;
    let msg = MessageParser::default()
        .parse(&raw)
        .ok_or_else(|| AppError::msg("Impossibile leggere il messaggio da inoltrare"))?;
    Ok(msg
        .attachments()
        .enumerate()
        .filter(|(_, p)| p.content_id().is_none() || p.attachment_name().is_some())
        .map(|(i, p)| crate::smtp::OutgoingAttachment {
            name: p.attachment_name().map(str::to_string).unwrap_or_else(|| format!("allegato-{}", i + 1)),
            mime: p
                .content_type()
                .map(|ct| format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or("octet-stream")))
                .unwrap_or_else(|| "application/octet-stream".into()),
            data: base64::engine::general_purpose::STANDARD.encode(p.contents()),
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxCheck {
    pub unseen: u32,
    pub last_uid: u32,
    /// Messaggi non letti arrivati dopo `after_uid`
    pub new: Vec<Envelope>,
}

/// Non letti contati con una ricerca: STATUS sulla cartella già selezionata può
/// restituire un valore vecchio (Exchange lo fa), SEARCH UNSEEN no.
pub async fn unseen_count(s: &mut ImapSession, folder: &str) -> Result<u32> {
    s.examine(folder).await?;
    Ok(s.uid_search("UNSEEN").await?.len() as u32)
}

/// Controllo leggero della posta in arrivo: non letti e, solo se serve, i nuovi messaggi.
pub async fn inbox_check(s: &mut ImapSession, after_uid: Option<u32>) -> Result<InboxCheck> {
    let mb = s.examine("INBOX").await?;
    let last_uid = mb.uid_next.unwrap_or(1).saturating_sub(1);
    let unseen = s.uid_search("UNSEEN").await?.len() as u32;
    let mut new = Vec::new();
    if let Some(after) = after_uid.filter(|a| *a < last_uid) {
        let fetches: Vec<Fetch> = s
            .uid_fetch(format!("{}:*", after + 1), LIST_ITEMS)
            .await?
            .try_collect()
            .await?;
        new = fetches
            .iter()
            .filter_map(envelope_from)
            .filter(|e| e.uid > after && !e.seen)
            .collect();
        new.sort_by_key(|m| std::cmp::Reverse(m.uid));
    }
    Ok(InboxCheck { unseen, last_uid, new })
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

pub async fn set_flag(s: &mut ImapSession, folder: &str, uids: &[u32], flag: &str, value: bool) -> Result<()> {
    let imap_flag = match flag {
        "seen" => "\\Seen",
        "flagged" => "\\Flagged",
        "answered" => "\\Answered",
        _ => return Err(AppError::msg("Flag sconosciuto")),
    };
    s.select(folder).await?;
    let op = if value { "+FLAGS.SILENT" } else { "-FLAGS.SILENT" };
    let _: Vec<Fetch> = s
        .uid_store(uid_set(uids), format!("{op} ({imap_flag})"))
        .await?
        .try_collect()
        .await?;
    Ok(())
}

pub async fn move_to(s: &mut ImapSession, folder: &str, uids: &[u32], dest: &str) -> Result<()> {
    s.select(folder).await?;
    let set = uid_set(uids);
    if s.uid_mv(&set, dest).await.is_err() {
        // Server senza estensione MOVE: copia + cancella
        s.uid_copy(&set, dest).await?;
        expunge(s, &set).await?;
    }
    Ok(())
}

pub async fn delete_forever(s: &mut ImapSession, folder: &str, uids: &[u32]) -> Result<()> {
    s.select(folder).await?;
    expunge(s, &uid_set(uids)).await
}

async fn expunge(s: &mut ImapSession, set: &str) -> Result<()> {
    let _: Vec<Fetch> = s
        .uid_store(set, "+FLAGS.SILENT (\\Deleted)")
        .await?
        .try_collect()
        .await?;
    // UID EXPUNGE tocca solo questi messaggi; EXPUNGE classico solo se il server non lo supporta
    let targeted: Result<Vec<_>> = async { Ok(s.uid_expunge(set).await?.try_collect().await?) }.await;
    if targeted.is_err() {
        let _: Vec<_> = s.expunge().await?.try_collect().await?;
    }
    Ok(())
}

pub async fn append(s: &mut ImapSession, folder: &str, raw: &[u8]) -> Result<()> {
    s.append(folder, Some("(\\Seen)"), None, raw).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{decode_mutf7, login_hint};
    use crate::store::Account;

    #[test]
    fn readable_login_errors() {
        let mut acc: Account = serde_json::from_value(serde_json::json!({
            "id": "x", "name": "", "email": "a@gmail.com", "username": "a@gmail.com",
            "imapHost": "imap.gmail.com", "imapPort": 993, "smtpHost": "smtp.gmail.com",
            "smtpPort": 465, "smtpSecurity": "ssl"
        }))
        .unwrap();
        let raw = r#"no response: code: Some(Alert), info: Some("Application-specific password required: https://support.google.com/accounts/answer/185833 (Failure)")"#;
        assert!(login_hint(&acc, raw).contains("password per le app"));
        acc.imap_host = "mail.esempio.it".into();
        assert_eq!(login_hint(&acc, r#"no response: code: None, info: Some("Login failed")"#), "Accesso rifiutato: Login failed");
    }

    #[test]
    fn mutf7() {
        assert_eq!(decode_mutf7("INBOX"), "INBOX");
        assert_eq!(decode_mutf7("&AOk-l&AOk-ments"), "éléments");
        assert_eq!(decode_mutf7("A&-B"), "A&B");
    }
}
