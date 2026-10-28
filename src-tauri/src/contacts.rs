//! Suggerimenti dei destinatari dalle rubriche del provider:
//! Microsoft (People API, poi directory dell'organizzazione) e Google (People API).
//! I contatti "imparati" dalla posta li gestisce il frontend, istantaneamente.

use crate::error::Result;
use crate::store::Account;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub name: String,
    pub email: String,
    /// rubrica | directory | google
    pub source: String,
}

fn odata_escape(s: &str) -> String {
    s.replace('\'', "''")
}

async fn get_json(url: &str, token: &str, extra: Option<(&str, &str)>) -> Option<Value> {
    let mut req = reqwest::Client::new().get(url).bearer_auth(token);
    if let Some((k, v)) = extra {
        req = req.header(k, v);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json().await.ok()
}

/// Persone rilevanti (rubrica, colleghi, chi ti scrive spesso), già ordinate da Microsoft.
async fn microsoft_people(token: &str, q: &str) -> Option<Vec<Contact>> {
    let url = reqwest::Url::parse_with_params(
        "https://graph.microsoft.com/v1.0/me/people",
        &[
            ("$search", format!("\"{}\"", q.replace('"', ""))),
            ("$top", "8".into()),
            ("$select", "displayName,scoredEmailAddresses".into()),
        ],
    )
    .ok()?;
    let v = get_json(url.as_str(), token, None).await?;
    Some(
        v["value"]
            .as_array()?
            .iter()
            .filter_map(|p| {
                let email = p["scoredEmailAddresses"][0]["address"].as_str()?;
                Some(Contact {
                    name: p["displayName"].as_str().unwrap_or_default().to_string(),
                    email: email.to_string(),
                    source: "rubrica".into(),
                })
            })
            .collect(),
    )
}

/// Ripiego senza People.Read: ricerca per prefisso nella directory dell'organizzazione.
async fn microsoft_directory(token: &str, q: &str) -> Option<Vec<Contact>> {
    let q = odata_escape(q);
    let filter = format!(
        "startswith(displayName,'{q}') or startswith(givenName,'{q}') or startswith(surname,'{q}') or startswith(mail,'{q}')"
    );
    let url = reqwest::Url::parse_with_params(
        "https://graph.microsoft.com/v1.0/users",
        &[("$filter", filter.as_str()), ("$select", "displayName,mail"), ("$top", "8")],
    )
    .ok()?;
    let v = get_json(url.as_str(), token, None).await?;
    Some(
        v["value"]
            .as_array()?
            .iter()
            .filter_map(|u| {
                Some(Contact {
                    name: u["displayName"].as_str().unwrap_or_default().to_string(),
                    email: u["mail"].as_str()?.to_string(),
                    source: "directory".into(),
                })
            })
            .collect(),
    )
}

fn google_people(v: &Value) -> Vec<Contact> {
    v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let p = &r["person"];
            Some(Contact {
                name: p["names"][0]["displayName"].as_str().unwrap_or_default().to_string(),
                email: p["emailAddresses"][0]["value"].as_str()?.to_string(),
                source: "google".into(),
            })
        })
        .collect()
}

async fn google(token: &str, q: &str) -> Vec<Contact> {
    let mut out = Vec::new();
    // Contatti salvati e "altri contatti" (le persone a cui hai scritto)
    for endpoint in ["people:searchContacts", "otherContacts:search"] {
        let Ok(url) = reqwest::Url::parse_with_params(
            &format!("https://people.googleapis.com/v1/{endpoint}"),
            &[("query", q), ("readMask", "names,emailAddresses"), ("pageSize", "8")],
        ) else {
            continue;
        };
        if let Some(v) = get_json(url.as_str(), token, None).await {
            out.extend(google_people(&v));
        }
    }
    out
}

pub async fn search(acc: &Account, q: &str) -> Result<Vec<Contact>> {
    let q = q.trim();
    if q.len() < 2 {
        return Ok(vec![]);
    }
    let found = match acc.auth.as_str() {
        "oauth-microsoft" => {
            let people = match crate::oauth::people_token(acc).await {
                Ok(t) => microsoft_people(&t, q).await,
                Err(_) => None,
            };
            match people {
                Some(p) if !p.is_empty() => p,
                _ => match crate::oauth::directory_token(acc).await {
                    Ok(t) => microsoft_directory(&t, q).await.unwrap_or_default(),
                    Err(_) => vec![],
                },
            }
        }
        "oauth-google" => match crate::oauth::access_token(acc).await {
            Ok(t) => google(&t, q).await,
            Err(_) => vec![],
        },
        _ => vec![],
    };
    // Niente doppioni (lo stesso indirizzo può arrivare da più fonti)
    let mut seen = std::collections::HashSet::new();
    Ok(found
        .into_iter()
        .filter(|c| seen.insert(c.email.to_lowercase()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn google_results_are_mapped() {
        let v = json!({ "results": [
            { "person": { "names": [{ "displayName": "Sara Greco" }], "emailAddresses": [{ "value": "sara@gmail.com" }] } },
            { "person": { "names": [{ "displayName": "Senza email" }] } }
        ]});
        assert_eq!(
            google_people(&v),
            vec![Contact { name: "Sara Greco".into(), email: "sara@gmail.com".into(), source: "google".into() }]
        );
    }

    #[test]
    fn odata_quotes_are_escaped() {
        assert_eq!(odata_escape("D'Amico"), "D''Amico");
    }
}
