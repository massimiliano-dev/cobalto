//! Allegati grandi su OneDrive, come fa Outlook: il file si carica nel drive
//! dell'utente e nel messaggio finisce un link condiviso.

use crate::error::{AppError, Result};
use crate::smtp::OutgoingAttachment;
use base64::Engine;
use serde_json::{json, Value};

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
/// Multiplo di 320 KiB, come richiesto dalle sessioni di upload di Graph
const CHUNK: usize = 320 * 1024 * 32;
const FOLDER: &str = "Allegati Cobalto";

pub struct SharedFile {
    pub name: String,
    pub size: usize,
    pub url: String,
}

fn segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b' ' => {
                if b == b' ' { "%20".to_string() } else { (b as char).to_string() }
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn graph_error(resp: reqwest::Response) -> AppError {
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or_default();
    let msg = v["error"]["message"].as_str().unwrap_or("errore sconosciuto");
    AppError::msg(format!("OneDrive ({status}): {msg}"))
}

async fn upload(client: &reqwest::Client, token: &str, name: &str, bytes: &[u8]) -> Result<Value> {
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let path = format!("{}/{}/{}", segment(FOLDER), day, segment(name));
    let resp = client
        .post(format!("{GRAPH}/me/drive/root:/{path}:/createUploadSession"))
        .bearer_auth(token)
        .json(&json!({ "item": { "@microsoft.graph.conflictBehavior": "rename" } }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(graph_error(resp).await);
    }
    let session: Value = resp.json().await?;
    let url = session["uploadUrl"]
        .as_str()
        .ok_or_else(|| AppError::msg("OneDrive non ha aperto la sessione di caricamento"))?
        .to_string();

    let total = bytes.len();
    let mut start = 0;
    loop {
        let end = (start + CHUNK).min(total);
        // L'URL di upload è già autorizzato: niente bearer token qui
        let resp = client
            .put(&url)
            .header("Content-Range", format!("bytes {}-{}/{}", start, end - 1, total))
            .body(bytes[start..end].to_vec())
            .send()
            .await?;
        let status = resp.status().as_u16();
        if status == 200 || status == 201 {
            return Ok(resp.json().await?);
        }
        if status != 202 {
            return Err(graph_error(resp).await);
        }
        start = end;
        if start >= total {
            return Err(AppError::msg("OneDrive non ha confermato il caricamento"));
        }
    }
}

/// Link di sola lettura: prima solo per i destinatari, poi per l'organizzazione,
/// infine per chiunque abbia il link (se il tenant lo consente).
async fn share(client: &reqwest::Client, token: &str, item_id: &str, recipients: &[String], same_org: bool) -> Result<String> {
    let mut attempts = vec![json!({
        "type": "view",
        "scope": "users",
        "recipients": recipients.iter().map(|e| json!({ "email": e })).collect::<Vec<_>>(),
    })];
    if same_org {
        attempts.push(json!({ "type": "view", "scope": "organization" }));
    }
    attempts.push(json!({ "type": "view", "scope": "anonymous" }));

    let mut last = None;
    for body in attempts {
        let resp = client
            .post(format!("{GRAPH}/me/drive/items/{item_id}/createLink"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?;
        if resp.status().is_success() {
            let v: Value = resp.json().await?;
            if let Some(url) = v["link"]["webUrl"].as_str() {
                return Ok(url.to_string());
            }
        } else {
            last = Some(graph_error(resp).await);
        }
    }
    Err(last.unwrap_or_else(|| AppError::msg("Impossibile creare il link di condivisione")))
}

pub async fn upload_and_share(token: &str, att: &OutgoingAttachment, recipients: &[String], same_org: bool) -> Result<SharedFile> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&att.data)
        .map_err(|_| AppError::msg(format!("Allegato illeggibile: {}", att.name)))?;
    let client = reqwest::Client::new();
    let item = upload(&client, token, &att.name, &bytes).await?;
    let id = item["id"].as_str().ok_or_else(|| AppError::msg("OneDrive non ha restituito il file"))?;
    let url = share(&client, token, id, recipients, same_org).await?;
    Ok(SharedFile { name: item["name"].as_str().unwrap_or(&att.name).to_string(), size: bytes.len(), url })
}

fn human_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Blocco HTML in stile "allegato cloud" di Outlook, con stili inline per i client di posta.
pub fn links_html(files: &[SharedFile]) -> String {
    let cards: String = files
        .iter()
        .map(|f| {
            format!(
                r#"<a href="{url}" target="_blank" style="display:block;margin:0 0 6px;padding:10px 14px;border:1px solid #c9d6f5;border-radius:8px;background:#f4f7ff;color:#1a3fb8;text-decoration:none;font-family:-apple-system,'Segoe UI',sans-serif;font-size:14px;max-width:420px"><span style="font-weight:600">{name}</span><br><span style="color:#5b6b92;font-size:12px">{size} · OneDrive</span></a>"#,
                url = escape(&f.url),
                name = escape(&f.name),
                size = human_size(f.size),
            )
        })
        .collect();
    format!(r#"<div style="margin:16px 0">{cards}</div>"#)
}

pub fn links_text(files: &[SharedFile]) -> String {
    let lines: Vec<String> = files
        .iter()
        .map(|f| format!("• {} ({}): {}", f.name, human_size(f.size), f.url))
        .collect();
    format!("File condivisi da OneDrive:\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_are_encoded() {
        assert_eq!(segment("Report Q3 (v2).pdf"), "Report%20Q3%20%28v2%29.pdf");
        assert_eq!(segment("città.xlsx"), "citt%C3%A0.xlsx");
    }

    #[test]
    fn link_block_escapes_names() {
        let files = [SharedFile { name: "a<b>.pdf".into(), size: 5 * 1024 * 1024, url: "https://x/y?a=1&b=2".into() }];
        let html = links_html(&files);
        assert!(html.contains("a&lt;b&gt;.pdf"));
        assert!(html.contains("a=1&amp;b=2"));
        assert!(html.contains("5.0 MB"));
        assert!(links_text(&files).contains("https://x/y?a=1&b=2"));
    }
}
