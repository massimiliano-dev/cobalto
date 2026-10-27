//! Caselle condivise di Microsoft 365: si aprono via IMAP con il token dell'utente,
//! mettendo l'indirizzo della casella condivisa come utente XOAUTH2.

use crate::error::{AppError, Result};
use crate::store::Account;
use serde::Serialize;
use serde_json::Value;

const GRAPH: &str = "https://graph.microsoft.com/v1.0";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMailbox {
    pub email: String,
    pub name: String,
    pub unread: u64,
}

/// Le caselle condivise di Exchange sono utenti con l'accesso disattivato: le si elenca
/// e poi si prova a leggerne la Posta in arrivo, così restano solo quelle accessibili.
pub async fn discover(owner: &Account, token: &str) -> Result<Vec<SharedMailbox>> {
    let client = reqwest::Client::new();
    let mut url = format!(
        "{GRAPH}/users?$filter=accountEnabled%20eq%20false&$select=id,displayName,mail&$top=999"
    );
    let mut candidates: Vec<(String, String, String)> = Vec::new();
    for _ in 0..5 {
        let resp = client.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            let v: Value = resp.json().await.unwrap_or_default();
            let msg = v["error"]["message"].as_str().unwrap_or("errore sconosciuto");
            return Err(AppError::msg(format!("Impossibile leggere la directory: {msg}")));
        }
        let v: Value = resp.json().await?;
        for u in v["value"].as_array().into_iter().flatten() {
            let (Some(id), Some(mail)) = (u["id"].as_str(), u["mail"].as_str()) else { continue };
            if mail.eq_ignore_ascii_case(&owner.email) {
                continue;
            }
            let name = u["displayName"].as_str().unwrap_or(mail).to_string();
            candidates.push((id.to_string(), mail.to_string(), name));
        }
        match v["@odata.nextLink"].as_str() {
            Some(next) => url = next.to_string(),
            None => break,
        }
    }

    let checks = candidates.iter().map(|(id, mail, name)| {
        let client = client.clone();
        async move {
            let resp = client
                .get(format!("{GRAPH}/users/{id}/mailFolders/inbox?$select=unreadItemCount"))
                .bearer_auth(token)
                .send()
                .await
                .ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let v: Value = resp.json().await.ok()?;
            Some(SharedMailbox {
                email: mail.clone(),
                name: name.clone(),
                unread: v["unreadItemCount"].as_u64().unwrap_or(0),
            })
        }
    });
    let mut found: Vec<SharedMailbox> = futures::future::join_all(checks).await.into_iter().flatten().collect();
    found.sort_by_key(|m| m.name.to_lowercase());
    Ok(found)
}

/// Account "figlio" per una casella condivisa: stessi server, token dell'account principale.
pub fn child(owner: &Account, email: &str, name: &str) -> Account {
    Account {
        id: uuid::Uuid::new_v4().to_string(),
        name: if name.is_empty() { email.split('@').next().unwrap_or(email).to_string() } else { name.to_string() },
        email: email.to_string(),
        username: email.to_string(),
        imap_host: owner.imap_host.clone(),
        imap_port: owner.imap_port,
        smtp_host: owner.smtp_host.clone(),
        smtp_port: owner.smtp_port,
        smtp_security: owner.smtp_security.clone(),
        signature: owner.signature.clone(),
        auth: owner.auth.clone(),
        shared_of: Some(owner.id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::child;
    use crate::store::Account;

    #[test]
    fn shared_child_uses_owner_servers_and_tokens() {
        let owner: Account = serde_json::from_value(serde_json::json!({
            "id": "owner", "name": "Max", "email": "m.polce@keynet.it", "username": "m.polce@keynet.it",
            "imapHost": "outlook.office365.com", "imapPort": 993, "smtpHost": "smtp.office365.com",
            "smtpPort": 587, "smtpSecurity": "starttls", "auth": "oauth-microsoft", "signature": "<p>Max</p>"
        }))
        .unwrap();
        let c = child(&owner, "info@keynet.it", "Info Keynet");
        assert_eq!(c.username, "info@keynet.it");
        assert_eq!(c.shared_of.as_deref(), Some("owner"));
        assert_eq!(c.imap_host, owner.imap_host);
        assert!(c.is_oauth());
        // Il campo resta fuori dal JSON degli account normali
        assert!(!serde_json::to_string(&owner).unwrap().contains("sharedOf"));
    }
}
