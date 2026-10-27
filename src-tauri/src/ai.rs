//! Assistente AI: Claude (Anthropic) oppure GPT (OpenAI o endpoint compatibile).
//! Le chiamate partono dal backend, così la chiave API non tocca mai la webview.

use crate::error::{AppError, Result};
use crate::store::Settings;
use futures::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tokio::task::AbortHandle;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub fn key_name(provider: &str) -> String {
    format!("ai:{provider}")
}

#[derive(Default)]
pub struct Jobs(pub Mutex<HashMap<String, AbortHandle>>);

#[derive(Clone, Serialize)]
struct Chunk<'a> {
    id: &'a str,
    delta: &'a str,
}

#[derive(Clone, Serialize)]
struct Done<'a> {
    id: &'a str,
    error: Option<String>,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

fn openai_base(s: &Settings) -> String {
    let b = s.openai_base_url.trim().trim_end_matches('/');
    if b.is_empty() { "https://api.openai.com/v1".into() } else { b.into() }
}

async fn api_error(resp: reqwest::Response) -> AppError {
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let detail = body["error"]["message"]
        .as_str()
        .unwrap_or("errore sconosciuto")
        .to_string();
    let hint = match status.as_u16() {
        401 | 403 => " — controlla la chiave API nelle impostazioni",
        404 => " — il modello scelto non esiste o non è abilitato",
        429 => " — limite di richieste raggiunto, riprova tra poco",
        _ => "",
    };
    AppError::msg(format!("{detail}{hint}"))
}

/// Elenco modelli disponibili per la chiave configurata.
pub async fn models(provider: &str, key: &str, s: &Settings) -> Result<Vec<String>> {
    let req = if provider == "anthropic" {
        client()
            .get(format!("{ANTHROPIC_URL}/models?limit=100"))
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION)
    } else {
        client().get(format!("{}/models", openai_base(s))).bearer_auth(key)
    };
    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let body: Value = resp.json().await?;
    let mut ids: Vec<String> = body["data"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if provider == "openai" {
        // Solo modelli di chat/ragionamento, niente embedding/immagini/audio
        ids.retain(|id| {
            (id.starts_with("gpt") || id.starts_with('o') || !s.openai_base_url.contains("openai.com"))
                && !["embedding", "tts", "whisper", "dall-e", "image", "audio", "realtime", "transcribe", "moderation", "search"]
                    .iter()
                    .any(|x| id.contains(x))
        });
        ids.sort_by(|a, b| b.cmp(a));
    }
    Ok(ids)
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

/// Avvia una generazione in streaming. I pezzi arrivano al frontend come eventi
/// `ai-chunk` e la fine come `ai-done`.
pub fn start(app: AppHandle, jobs: &Jobs, id: String, system: String, messages: Vec<ChatMsg>, s: Settings, key: String) {
    let job_id = id.clone();
    // Il runtime di Tauri, non tokio::spawn: questo comando gira sul thread principale,
    // fuori dal runtime asincrono, e tokio::spawn lì va in panic (e chiude l'app)
    let handle = tauri::async_runtime::spawn(async move {
        let result = run(&app, &id, &system, &messages, &s, &key).await;
        let _ = app.emit(
            "ai-done",
            Done { id: &id, error: result.err().map(|e| e.to_string()) },
        );
    });
    if let Ok(mut map) = jobs.0.lock() {
        map.retain(|_, h| !h.is_finished());
        map.insert(job_id, handle.inner().abort_handle());
    }
}

pub fn cancel(jobs: &Jobs, id: &str) {
    if let Some(h) = jobs.0.lock().ok().and_then(|mut m| m.remove(id)) {
        h.abort();
    }
}

/// Toglie i blocchi `<think>…</think>` che alcuni modelli locali emettono comunque.
#[derive(Default)]
struct ThinkFilter {
    inside: bool,
    pending: String,
}

impl ThinkFilter {
    fn push(&mut self, delta: &str) -> String {
        self.pending.push_str(delta);
        let mut out = String::new();
        loop {
            let tag = if self.inside { "</think>" } else { "<think>" };
            if let Some(pos) = self.pending.find(tag) {
                if !self.inside {
                    out.push_str(&self.pending[..pos]);
                }
                self.pending.drain(..pos + tag.len());
                self.inside = !self.inside;
                continue;
            }
            // Tiene da parte un possibile inizio di tag spezzato tra due pezzi
            let keep = (1..tag.len())
                .rev()
                .find(|&n| self.pending.ends_with(&tag[..n]))
                .unwrap_or(0);
            let cut = self.pending.len() - keep;
            if !self.inside {
                out.push_str(&self.pending[..cut]);
            }
            self.pending.drain(..cut);
            return out;
        }
    }
}

fn local_base(s: &Settings) -> String {
    let b = s.local_url.trim().trim_end_matches('/');
    if b.is_empty() { "http://localhost:11434".into() } else { b.into() }
}

async fn run(app: &AppHandle, id: &str, system: &str, messages: &[ChatMsg], s: &Settings, key: &str) -> Result<()> {
    let provider = s.ai_provider.as_str();
    let req = match provider {
        "anthropic" => client()
            .post(format!("{ANTHROPIC_URL}/messages"))
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&json!({
                "model": s.anthropic_model,
                "max_tokens": 4096,
                "system": system,
                "stream": true,
                "messages": messages,
            })),
        "local" => {
            let mut all = vec![ChatMsg { role: "system".into(), content: system.into() }];
            all.extend_from_slice(messages);
            client()
                .post(format!("{}/api/chat", local_base(s)))
                .json(&json!({
                    "model": s.local_model,
                    "stream": true,
                    // Per la posta servono risposte rapide, non catene di ragionamento
                    "think": false,
                    "keep_alive": "30m",
                    "options": { "num_ctx": 16384 },
                    "messages": all,
                }))
        }
        _ => {
            let mut all = vec![ChatMsg { role: "system".into(), content: system.into() }];
            all.extend_from_slice(messages);
            client()
                .post(format!("{}/chat/completions", openai_base(s)))
                .bearer_auth(key)
                .json(&json!({ "model": s.openai_model, "stream": true, "messages": all }))
        }
    };
    let resp = req.send().await.map_err(|e| {
        if provider == "local" && e.is_connect() {
            AppError::msg("Ollama non risponde: apri l'app Ollama e riprova")
        } else {
            e.into()
        }
    })?;
    if !resp.status().is_success() {
        if provider == "local" {
            let v: Value = resp.json().await.unwrap_or_default();
            let msg = v["error"].as_str().unwrap_or("errore del modello locale");
            return Err(AppError::msg(if msg.contains("not found") {
                format!("Il modello «{}» non è installato: scaricalo dalle Impostazioni", s.local_model)
            } else {
                msg.to_string()
            }));
        }
        return Err(api_error(resp).await);
    }

    let mut think = ThinkFilter::default();
    let mut stream = resp.bytes_stream();
    // Buffer in byte: un carattere UTF-8 può arrivare spezzato tra due pacchetti
    let mut buf: Vec<u8> = Vec::new();
    while let Some(bytes) = stream.next().await {
        buf.extend_from_slice(&bytes?);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            let delta = if provider == "local" {
                // Ollama: una riga JSON per pezzo (NDJSON)
                let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                if let Some(err) = v["error"].as_str() {
                    return Err(AppError::msg(err.to_string()));
                }
                if v["done"] == true {
                    return Ok(());
                }
                think.push(v["message"]["content"].as_str().unwrap_or_default())
            } else {
                // SSE: righe "data: {...}"
                let Some(data) = line.strip_prefix("data:") else { continue };
                let data = data.trim();
                if data == "[DONE]" {
                    return Ok(());
                }
                let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
                if v["type"] == "error" {
                    return Err(AppError::msg(
                        v["error"]["message"].as_str().unwrap_or("errore del modello").to_string(),
                    ));
                }
                let d = if provider == "anthropic" {
                    v["delta"]["text"].as_str()
                } else {
                    v["choices"][0]["delta"]["content"].as_str()
                };
                d.unwrap_or_default().to_string()
            };
            if !delta.is_empty() {
                let _ = app.emit("ai-chunk", Chunk { id, delta: &delta });
            }
        }
    }
    Ok(())
}

// ---------- Ollama: modelli installati e download ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalStatus {
    pub running: bool,
    pub models: Vec<LocalModel>,
}

pub async fn local_status(s: &Settings) -> LocalStatus {
    let resp = client()
        .get(format!("{}/api/tags", local_base(s)))
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await;
    let Ok(resp) = resp else {
        return LocalStatus { running: false, models: vec![] };
    };
    let v: Value = resp.json().await.unwrap_or_default();
    let models = v["models"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    Some(LocalModel {
                        name: m["name"].as_str()?.to_string(),
                        size: m["size"].as_u64().unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    LocalStatus { running: true, models }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PullProgress<'a> {
    model: &'a str,
    status: &'a str,
    completed: u64,
    total: u64,
    done: bool,
    error: Option<String>,
}

/// Scarica un modello con Ollama, emettendo `ollama-pull` con l'avanzamento.
pub async fn pull(app: AppHandle, s: Settings, model: String) {
    let emit = |status: &str, completed: u64, total: u64, done: bool, error: Option<String>| {
        let _ = app.emit("ollama-pull", PullProgress { model: &model, status, completed, total, done, error });
    };
    let resp = client()
        .post(format!("{}/api/pull", local_base(&s)))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await;
    let resp = match resp {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => return emit("errore", 0, 0, true, Some(format!("Ollama ha risposto {}", r.status()))),
        Err(_) => return emit("errore", 0, 0, true, Some("Ollama non risponde: apri l'app Ollama".into())),
    };
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(Ok(bytes)) = stream.next().await {
        buf.extend_from_slice(&bytes);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
            if let Some(err) = v["error"].as_str() {
                return emit("errore", 0, 0, true, Some(err.to_string()));
            }
            let status = v["status"].as_str().unwrap_or_default();
            emit(status, v["completed"].as_u64().unwrap_or(0), v["total"].as_u64().unwrap_or(0), status == "success", None);
            if status == "success" {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThinkFilter;

    /// Regressione: i comandi sincroni girano sul thread principale, fuori dal runtime.
    /// Lì tokio::spawn va in panic; il runtime di Tauri invece funziona.
    #[test]
    fn spawning_from_a_plain_thread_does_not_panic() {
        let outcome = std::thread::spawn(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = tauri::async_runtime::spawn(async move {
                tx.send(42).unwrap();
            });
            let _abort = handle.inner().abort_handle();
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        })
        .join();
        assert_eq!(outcome.unwrap(), 42);
        assert!(std::thread::spawn(|| { tokio::spawn(async {}); }).join().is_err(), "tokio::spawn fuori dal runtime va in panic");
    }

    #[test]
    fn think_blocks_are_removed_even_when_split() {
        let mut f = ThinkFilter::default();
        let mut out = String::new();
        for piece in ["Ciao <thi", "nk>ragiono…</th", "ink>Mario", ", a presto"] {
            out.push_str(&f.push(piece));
        }
        assert_eq!(out, "Ciao Mario, a presto");
    }
}
