//! Invio posta via SMTP con lettre.

use crate::error::{AppError, Result};
use crate::store::Account;
use base64::Engine;
use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use crate::imap::Credential;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingAttachment {
    pub name: String,
    pub mime: String,
    /// Contenuto in base64 (letto dal frontend con un <input type=file>)
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardOf {
    pub folder: String,
    pub uid: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub subject: String,
    pub text: String,
    pub html: String,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<OutgoingAttachment>,
    #[serde(default)]
    pub forward_of: Option<ForwardOf>,
}

fn parse_mailbox(s: &str) -> Result<Mailbox> {
    s.trim()
        .parse::<Mailbox>()
        .map_err(|_| AppError::msg(format!("Indirizzo non valido: {s}")))
}

/// Costruisce il messaggio e restituisce anche i byte RFC 5322 (per salvarlo in "Inviata").
/// `keep_bcc`: lascia l'intestazione Bcc nel messaggio. Serve per Graph, che ricava
/// i destinatari dalle intestazioni; con SMTP invece i Ccn non devono comparire.
pub fn build(acc: &Account, d: &Draft, keep_bcc: bool) -> Result<(Message, Vec<u8>)> {
    let from = Mailbox::new(Some(acc.name.clone()), acc.email.parse()?);
    let mut b = Message::builder().from(from).subject(&d.subject);
    for t in d.to.iter().filter(|s| !s.trim().is_empty()) {
        b = b.to(parse_mailbox(t)?);
    }
    for t in d.cc.iter().filter(|s| !s.trim().is_empty()) {
        b = b.cc(parse_mailbox(t)?);
    }
    for t in d.bcc.iter().filter(|s| !s.trim().is_empty()) {
        b = b.bcc(parse_mailbox(t)?);
    }
    if let Some(id) = &d.in_reply_to {
        b = b.in_reply_to(id.clone());
    }
    if !d.references.is_empty() {
        b = b.raw_header(HeaderValue::new(
            HeaderName::new_from_ascii_str("References"),
            d.references.join(" "),
        ));
    }
    b = b.user_agent("Cobalto".into());
    if keep_bcc {
        b = b.keep_bcc();
    }

    let (html, inline) = extract_inline_images(&d.html.replace(LINKS_HTML_MARK, ""));
    let text = d.text.replace(&format!("\n\n{LINKS_TEXT_MARK}"), "").replace(LINKS_TEXT_MARK, "");
    let alternative = MultiPart::alternative()
        .singlepart(SinglePart::plain(text))
        .singlepart(SinglePart::html(html));
    // Immagini della firma come parti inline: i client bloccano le data: URI
    let body = if inline.is_empty() {
        alternative
    } else {
        let mut related = MultiPart::related().multipart(alternative);
        for img in inline {
            let ct = ContentType::parse(&img.mime)
                .unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap());
            related = related.singlepart(Attachment::new_inline(img.cid).body(img.bytes, ct));
        }
        related
    };
    let msg = if d.attachments.is_empty() {
        b.multipart(body)?
    } else {
        let mut mixed = MultiPart::mixed().multipart(body);
        for a in &d.attachments {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&a.data)
                .map_err(|_| AppError::msg(format!("Allegato illeggibile: {}", a.name)))?;
            let ct = ContentType::parse(&a.mime)
                .unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap());
            mixed = mixed.singlepart(Attachment::new(a.name.clone()).body(bytes, ct));
        }
        b.multipart(mixed)?
    };
    let raw = msg.formatted();
    Ok((msg, raw))
}

/// Limite di Graph per una richiesta sendMail (4 MB, con il base64 che aggiunge un terzo).
pub const GRAPH_MAX_MIME: usize = 2_900_000;

/// Segnaposto messi dal frontend dove inserire i link OneDrive (subito dopo il testo, prima della firma)
pub const LINKS_HTML_MARK: &str = "<!--cobalto-links-->";
pub const LINKS_TEXT_MARK: &str = "{{cobalto-links}}";

/// Invio tramite Microsoft Graph con il messaggio MIME già pronto.
/// Graph salva da sé la copia in "Posta inviata".
pub async fn graph_send(token: &str, raw: &[u8]) -> Result<()> {
    if raw.len() > GRAPH_MAX_MIME {
        return Err(AppError::msg(
            "Messaggio troppo grande per l'invio tramite Microsoft 365 (massimo circa 3 MB di allegati)",
        ));
    }
    let resp = reqwest::Client::new()
        .post("https://graph.microsoft.com/v1.0/me/sendMail")
        .bearer_auth(token)
        .header("Content-Type", "text/plain")
        .body(base64::engine::general_purpose::STANDARD.encode(raw))
        .send()
        .await?;
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    let v: serde_json::Value = resp.json().await.unwrap_or_default();
    let msg = v["error"]["message"].as_str().unwrap_or("errore sconosciuto");
    Err(AppError::msg(format!("Microsoft 365 ha rifiutato l'invio ({status}): {msg}")))
}

struct InlineImage {
    cid: String,
    mime: String,
    bytes: Vec<u8>,
}

/// Sostituisce ogni `src="data:…;base64,…"` con `src="cid:…"` e restituisce le immagini estratte.
/// Immagini identiche (es. lo stesso logo ripetuto) diventano una sola parte.
fn extract_inline_images(html: &str) -> (String, Vec<InlineImage>) {
    let mut out = String::with_capacity(html.len());
    let mut images: Vec<(String, InlineImage)> = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find("data:") {
        let quote = rest[..pos].chars().next_back();
        let is_src = matches!(quote, Some('"') | Some('\''))
            && rest[..pos - 1].trim_end().to_ascii_lowercase().ends_with("src=");
        let end = quote
            .filter(|_| is_src)
            .and_then(|q| rest[pos..].find(q))
            .map(|e| pos + e);
        let Some(end) = end else {
            out.push_str(&rest[..pos + 5]);
            rest = &rest[pos + 5..];
            continue;
        };
        let uri = &rest[pos..end];
        let parsed = uri
            .strip_prefix("data:")
            .and_then(|u| u.split_once(";base64,"))
            .and_then(|(mime, b64)| {
                let clean: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
                base64::engine::general_purpose::STANDARD
                    .decode(clean)
                    .ok()
                    .map(|bytes| (mime.to_string(), bytes))
            });
        out.push_str(&rest[..pos]);
        match parsed {
            Some((mime, bytes)) if mime.starts_with("image/") => {
                let cid = match images.iter().find(|(u, _)| u == uri) {
                    Some((_, img)) => img.cid.clone(),
                    None => {
                        let cid = format!("img{}.{}@cobalto", images.len() + 1, uuid::Uuid::new_v4().simple());
                        images.push((uri.to_string(), InlineImage { cid: cid.clone(), mime, bytes }));
                        cid
                    }
                };
                out.push_str("cid:");
                out.push_str(&cid);
            }
            _ => out.push_str(uri),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    (out, images.into_iter().map(|(_, img)| img).collect())
}

fn transport(acc: &Account, cred: &Credential, secs: u64) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let builder = if acc.smtp_security == "starttls" {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&acc.smtp_host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&acc.smtp_host)?
    };
    let builder = match cred {
        Credential::Password(p) => builder.credentials(Credentials::new(acc.username.clone(), p.clone())),
        // lettre costruisce da sé la stringa XOAUTH2 a partire da utente + token
        Credential::Token(t) => builder
            .credentials(Credentials::new(acc.username.clone(), t.clone()))
            .authentication(vec![Mechanism::Xoauth2]),
    };
    Ok(builder
        .port(acc.smtp_port)
        .timeout(Some(std::time::Duration::from_secs(secs)))
        .build())
}

fn smtp_hint(acc: &Account, e: AppError) -> AppError {
    let microsoft = acc.smtp_host.contains("office365") || acc.smtp_host.contains("outlook");
    let text = e.to_string();
    if microsoft && (text.contains("535") || text.contains("SmtpClientAuthentication") || text.contains("disabled")) {
        AppError::msg("Microsoft ha rifiutato l'invio: l'«SMTP autenticato» è disattivato per questa casella e va abilitato dall'amministratore di Microsoft 365.")
    } else {
        e
    }
}

pub async fn send(acc: &Account, cred: &Credential, msg: Message) -> Result<()> {
    let result: Result<_> = async { Ok(transport(acc, cred, 40)?.send(msg).await?) }.await;
    result.map(|_| ()).map_err(|e| smtp_hint(acc, e))
}

/// Verifica le credenziali SMTP senza inviare nulla.
pub async fn test(acc: &Account, cred: &Credential) -> Result<()> {
    let ok: Result<bool> = async { Ok(transport(acc, cred, 20)?.test_connection().await?) }.await;
    if ok.map_err(|e| smtp_hint(acc, e))? {
        Ok(())
    } else {
        Err(AppError::msg("Il server SMTP non risponde"))
    }
}

#[cfg(test)]
mod tests {
    use super::{build, extract_inline_images, Draft};
    use crate::store::Account;

    fn sample() -> (Account, Draft) {
        let acc: Account = serde_json::from_value(serde_json::json!({
            "id": "x", "name": "Max", "email": "m@example.com", "username": "m@example.com",
            "imapHost": "outlook.office365.com", "imapPort": 993, "smtpHost": "smtp.office365.com",
            "smtpPort": 587, "smtpSecurity": "starttls", "auth": "oauth-microsoft"
        }))
        .unwrap();
        let draft: Draft = serde_json::from_value(serde_json::json!({
            "to": ["a@esempio.it"], "bcc": ["nascosto@esempio.it"],
            "subject": "Prova", "text": "ciao", "html": "<p>ciao</p>"
        }))
        .unwrap();
        (acc, draft)
    }

    #[test]
    fn bcc_kept_only_for_graph() {
        let (acc, draft) = sample();
        let (_, graph) = build(&acc, &draft, true).unwrap();
        let (_, smtp) = build(&acc, &draft, false).unwrap();
        assert!(String::from_utf8_lossy(&graph).contains("nascosto@esempio.it"));
        assert!(!String::from_utf8_lossy(&smtp).contains("nascosto@esempio.it"));
    }

    #[test]
    fn data_images_become_cid() {
        let logo = "data:image/png;base64,iVBORw0KGgo=";
        let html = format!(r#"<p>Ciao</p><img src="{logo}"><a href="https://example.com">Example</a><img src='{logo}'>"#);
        let (out, imgs) = extract_inline_images(&html);
        assert_eq!(imgs.len(), 1, "stessa immagine = una sola parte");
        assert!(!out.contains("data:"));
        assert_eq!(out.matches(&format!("cid:{}", imgs[0].cid)).count(), 2);
        assert!(out.contains(r#"href="https://example.com""#));
        assert_eq!(imgs[0].mime, "image/png");
    }

    #[test]
    fn plain_text_mentioning_data_is_untouched() {
        let html = "<p>data: oggi</p>";
        let (out, imgs) = extract_inline_images(html);
        assert_eq!(out, html);
        assert!(imgs.is_empty());
    }
}
