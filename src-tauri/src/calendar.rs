//! Calendario multi-account: Microsoft (Graph) e Google (Calendar API), in sola lettura.
//! Ogni evento porta con sé, se c'è, il link per entrare nella chiamata.

use crate::error::{AppError, Result};
use crate::store::Account;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalEvent {
    pub id: String,
    pub account_id: String,
    pub title: String,
    /// Millisecondi UTC (0 per gli eventi di tutto il giorno)
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    /// Solo per gli eventi di tutto il giorno: "AAAA-MM-GG", fine esclusa
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub location: String,
    pub organizer: String,
    pub attendees: Vec<String>,
    pub join_url: Option<String>,
    /// teams | meet | zoom | webex | altro
    pub join_kind: Option<String>,
    pub web_link: Option<String>,
    pub preview: String,
    /// busy | tentative | free | oof
    pub show_as: String,
    pub cancelled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCalendar {
    pub account_id: String,
    /// ok | unsupported | error
    pub status: String,
    pub message: Option<String>,
}

/// Riconosce i link delle videochiamate più diffuse dentro un testo qualsiasi.
pub fn find_join_link(text: &str) -> Option<(String, String)> {
    const KINDS: [(&str, &str); 10] = [
        ("teams.microsoft.com/l/meetup-join", "teams"),
        ("teams.live.com/meet", "teams"),
        ("teams.microsoft.com/meet", "teams"),
        ("meet.google.com/", "meet"),
        ("zoom.us/j/", "zoom"),
        ("zoom.us/my/", "zoom"),
        ("webex.com/", "webex"),
        ("whereby.com/", "altro"),
        ("meet.jit.si/", "altro"),
        ("gotomeet", "altro"),
    ];
    let mut rest = text;
    while let Some(pos) = rest.find("https://") {
        let tail = &rest[pos..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | ']'))
            .unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';']);
        if let Some((_, kind)) = KINDS.iter().find(|(needle, _)| url.contains(needle)) {
            return Some((url.to_string(), kind.to_string()));
        }
        rest = &tail[end.max(8)..];
    }
    None
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                out.push(' ');
            }
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn preview(s: &str) -> String {
    let text = strip_html(s);
    // Le note delle riunioni Teams iniziano con una riga di trattini: non è contenuto utile
    let text = text.trim_start_matches(['_', '-', ' ']);
    text.chars().take(280).collect()
}

// ---------- Microsoft Graph ----------

fn graph_time(v: &Value) -> Option<i64> {
    let s = v["dateTime"].as_str()?;
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|t| Utc.from_utc_datetime(&t).timestamp_millis())
}

fn graph_event(account_id: &str, e: &Value) -> CalEvent {
    let all_day = e["isAllDay"].as_bool().unwrap_or(false);
    let location = e["location"]["displayName"].as_str().unwrap_or_default().to_string();
    let body = e["bodyPreview"].as_str().unwrap_or_default();
    let join = e["onlineMeeting"]["joinUrl"]
        .as_str()
        .or_else(|| e["onlineMeetingUrl"].as_str())
        .map(|u| (u.to_string(), find_join_link(u).map(|(_, k)| k).unwrap_or_else(|| "teams".into())))
        .or_else(|| find_join_link(&location))
        .or_else(|| find_join_link(body));
    let date = |k: &str| e[k]["dateTime"].as_str().map(|s| s.chars().take(10).collect::<String>());
    CalEvent {
        id: e["id"].as_str().unwrap_or_default().to_string(),
        account_id: account_id.to_string(),
        title: e["subject"].as_str().filter(|s| !s.is_empty()).unwrap_or("(senza titolo)").to_string(),
        start: if all_day { 0 } else { graph_time(&e["start"]).unwrap_or(0) },
        end: if all_day { 0 } else { graph_time(&e["end"]).unwrap_or(0) },
        all_day,
        start_date: if all_day { date("start") } else { None },
        end_date: if all_day { date("end") } else { None },
        location,
        organizer: e["organizer"]["emailAddress"]["name"].as_str().unwrap_or_default().to_string(),
        attendees: e["attendees"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let addr = &x["emailAddress"];
                        addr["name"].as_str().or_else(|| addr["address"].as_str()).map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        join_url: join.as_ref().map(|j| j.0.clone()),
        join_kind: join.map(|j| j.1),
        web_link: e["webLink"].as_str().map(str::to_string),
        preview: preview(body),
        show_as: e["showAs"].as_str().unwrap_or("busy").to_lowercase(),
        cancelled: e["isCancelled"].as_bool().unwrap_or(false),
    }
}

async fn microsoft(acc: &Account, token: &str, start: i64, end: i64) -> Result<Vec<CalEvent>> {
    let iso = |ms: i64| {
        DateTime::<Utc>::from_timestamp_millis(ms)
            .unwrap_or_default()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    };
    let client = reqwest::Client::new();
    let mut url = format!(
        "https://graph.microsoft.com/v1.0/me/calendarView?startDateTime={}&endDateTime={}&$top=250\
         &$select=id,subject,start,end,isAllDay,location,organizer,attendees,onlineMeeting,onlineMeetingUrl,bodyPreview,webLink,showAs,isCancelled",
        iso(start),
        iso(end)
    );
    let mut events = Vec::new();
    // Le pagine successive arrivano con @odata.nextLink
    for _ in 0..10 {
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .header("Prefer", "outlook.timezone=\"UTC\"")
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let v: Value = resp.json().await.unwrap_or_default();
            let msg = v["error"]["message"].as_str().unwrap_or("errore sconosciuto");
            return Err(AppError::msg(format!("Calendario Microsoft ({status}): {msg}")));
        }
        let v: Value = resp.json().await?;
        if let Some(items) = v["value"].as_array() {
            events.extend(items.iter().map(|e| graph_event(&acc.id, e)));
        }
        match v["@odata.nextLink"].as_str() {
            Some(next) => url = next.to_string(),
            None => break,
        }
    }
    Ok(events)
}

// ---------- Google Calendar ----------

fn google_time(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v["dateTime"].as_str()?)
        .ok()
        .map(|t| t.timestamp_millis())
}

fn google_event(account_id: &str, e: &Value) -> CalEvent {
    let all_day = e["start"]["date"].is_string();
    let location = e["location"].as_str().unwrap_or_default().to_string();
    let description = e["description"].as_str().unwrap_or_default();
    let video = e["conferenceData"]["entryPoints"]
        .as_array()
        .and_then(|eps| eps.iter().find(|p| p["entryPointType"] == "video"))
        .and_then(|p| p["uri"].as_str());
    let join = e["hangoutLink"]
        .as_str()
        .or(video)
        .map(|u| (u.to_string(), find_join_link(u).map(|(_, k)| k).unwrap_or_else(|| "meet".into())))
        .or_else(|| find_join_link(&location))
        .or_else(|| find_join_link(description));
    let me_declined = e["attendees"]
        .as_array()
        .map(|a| a.iter().any(|x| x["self"] == true && x["responseStatus"] == "declined"))
        .unwrap_or(false);
    CalEvent {
        id: e["id"].as_str().unwrap_or_default().to_string(),
        account_id: account_id.to_string(),
        title: e["summary"].as_str().filter(|s| !s.is_empty()).unwrap_or("(senza titolo)").to_string(),
        start: if all_day { 0 } else { google_time(&e["start"]).unwrap_or(0) },
        end: if all_day { 0 } else { google_time(&e["end"]).unwrap_or(0) },
        all_day,
        start_date: e["start"]["date"].as_str().map(str::to_string),
        end_date: e["end"]["date"].as_str().map(str::to_string),
        location,
        organizer: e["organizer"]["displayName"]
            .as_str()
            .or_else(|| e["organizer"]["email"].as_str())
            .unwrap_or_default()
            .to_string(),
        attendees: e["attendees"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| x["resource"] != true)
                    .filter_map(|x| x["displayName"].as_str().or_else(|| x["email"].as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        join_url: join.as_ref().map(|j| j.0.clone()),
        join_kind: join.map(|j| j.1),
        web_link: e["htmlLink"].as_str().map(str::to_string),
        preview: preview(description),
        show_as: if e["transparency"] == "transparent" { "free".into() } else { "busy".into() },
        cancelled: e["status"] == "cancelled" || me_declined,
    }
}

async fn google(acc: &Account, token: &str, start: i64, end: i64) -> Result<Vec<CalEvent>> {
    let rfc = |ms: i64| DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or_default().to_rfc3339();
    let url = reqwest::Url::parse_with_params(
        "https://www.googleapis.com/calendar/v3/calendars/primary/events",
        &[
            ("timeMin", rfc(start)),
            ("timeMax", rfc(end)),
            ("singleEvents", "true".into()),
            ("orderBy", "startTime".into()),
            ("maxResults", "500".into()),
        ],
    )
    .map_err(|e| AppError::msg(e.to_string()))?;
    let resp = reqwest::Client::new().get(url).bearer_auth(token).send().await?;
    if !resp.status().is_success() {
        let v: Value = resp.json().await.unwrap_or_default();
        let msg = v["error"]["message"].as_str().unwrap_or("errore sconosciuto");
        let reason = v["error"]["errors"][0]["reason"].as_str().unwrap_or_default();
        let text = v.to_string();
        return Err(AppError::msg(if text.contains("SERVICE_DISABLED") || reason == "accessNotConfigured" {
            "Il calendario Google non è attivo nel progetto Cobalto: abilita «Google Calendar API» su Google Cloud".to_string()
        } else if text.contains("insufficient") || text.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT") {
            "Ricollega l'account Google per dare accesso al calendario".to_string()
        } else {
            format!("Calendario Google: {msg}")
        }));
    }
    let v: Value = resp.json().await?;
    Ok(v["items"]
        .as_array()
        .map(|items| items.iter().map(|e| google_event(&acc.id, e)).collect())
        .unwrap_or_default())
}

/// Eventi di un account nell'intervallo; gli account senza OAuth non hanno un calendario leggibile.
pub async fn events(acc: &Account, start: i64, end: i64) -> Result<Vec<CalEvent>> {
    match acc.auth.as_str() {
        "oauth-microsoft" => {
            let token = crate::oauth::graph_send_token(acc).await.map_err(|_| {
                AppError::msg("Ricollega l'account Microsoft per dare accesso al calendario")
            })?;
            microsoft(acc, &token, start, end).await
        }
        "oauth-google" => {
            let token = crate::oauth::access_token(acc).await?;
            google(acc, &token, start, end).await
        }
        _ => Err(AppError::msg("unsupported")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn join_links_are_found_in_free_text() {
        let body = "Partecipa: https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc/0?context=x. Grazie";
        assert_eq!(find_join_link(body).unwrap().1, "teams");
        assert_eq!(
            find_join_link("<a href=\"https://us02web.zoom.us/j/8812345?pwd=xyz\">Zoom</a>").unwrap().0,
            "https://us02web.zoom.us/j/8812345?pwd=xyz"
        );
        assert!(find_join_link("Sala riunioni 2, https://example.com/mappa").is_none());
    }

    #[test]
    fn graph_event_parsing() {
        let e = json!({
            "id": "A1", "subject": "Revisione preventivo", "isAllDay": false,
            "start": { "dateTime": "2026-10-29T10:00:00.0000000", "timeZone": "UTC" },
            "end": { "dateTime": "2026-10-29T10:30:00.0000000", "timeZone": "UTC" },
            "location": { "displayName": "Microsoft Teams Meeting" },
            "organizer": { "emailAddress": { "name": "Giulia Bianchi" } },
            "onlineMeeting": { "joinUrl": "https://teams.microsoft.com/l/meetup-join/xyz" },
            "showAs": "busy", "isCancelled": false
        });
        let ev = graph_event("acc", &e);
        assert_eq!(ev.start, 1_793_268_000_000);
        assert_eq!(ev.end - ev.start, 30 * 60 * 1000);
        assert_eq!(ev.join_kind.as_deref(), Some("teams"));
        assert_eq!(ev.organizer, "Giulia Bianchi");
    }

    #[test]
    fn google_all_day_and_meet() {
        let e = json!({
            "id": "g1", "summary": "Ferie", "start": { "date": "2026-12-24" }, "end": { "date": "2026-12-27" },
            "transparency": "transparent"
        });
        let ev = google_event("acc", &e);
        assert!(ev.all_day);
        assert_eq!(ev.start_date.as_deref(), Some("2026-12-24"));
        assert_eq!(ev.show_as, "free");

        let e = json!({
            "id": "g2", "summary": "Call", "hangoutLink": "https://meet.google.com/abc-defg-hij",
            "start": { "dateTime": "2026-10-29T15:00:00+01:00" }, "end": { "dateTime": "2026-10-29T15:45:00+01:00" }
        });
        let ev = google_event("acc", &e);
        assert_eq!(ev.join_kind.as_deref(), Some("meet"));
        assert_eq!(ev.end - ev.start, 45 * 60 * 1000);
    }
}
