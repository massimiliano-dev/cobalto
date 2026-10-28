mod ai;
mod calendar;
mod contacts;
mod error;
mod imap;
mod oauth;
mod shared;
mod onedrive;
mod smtp;
mod store;

use error::{AppError, Result};
use serde::{Deserialize, Serialize};
use store::{Account, Settings, Store};
use tauri::{AppHandle, Manager, State};

const PAGE_SIZE: u32 = 50;

struct App {
    store: Store,
    pool: imap::Pool,
    jobs: ai::Jobs,
    /// Accesso OAuth in attesa del browser: chiudere il canale lo annulla
    oauth_pending: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

/// Esegue un'operazione IMAP sulla corsia indicata; se fallisce invalida la
/// connessione e ritenta una volta su una connessione nuova.
macro_rules! with_imap {
    ($app:expr, $acc:expr, $lane:expr, |$s:ident| $body:expr) => {{
        let mut conn = $app.pool.lane(&$acc.id, $lane).await;
        let first: Result<_> = async {
            let $s = conn.session(&$acc).await?;
            $body
        }
        .await;
        match first {
            Ok(v) => Ok(v),
            Err(AppError::Imap(_)) | Err(AppError::Io(_)) => {
                conn.invalidate();
                let $s = conn.session(&$acc).await?;
                $body
            }
            Err(e) => Err(e),
        }
    }};
}

// ---------- Account ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountView {
    #[serde(flatten)]
    account: Account,
}

#[tauri::command]
fn accounts_list(app: State<'_, App>) -> Vec<AccountView> {
    app.store.accounts().into_iter().map(|account| AccountView { account }).collect()
}

#[tauri::command]
async fn account_save(app: State<'_, App>, mut account: Account, password: Option<String>) -> Result<Account> {
    if account.id.is_empty() {
        account.id = uuid::Uuid::new_v4().to_string();
    }
    if account.is_oauth() {
        // Account OAuth: si modificano solo nome e firma, le credenziali restano i token
        let current = app.store.account(&account.id)?;
        let updated = Account { name: account.name, signature: account.signature, ..current };
        app.store.save_account(updated.clone())?;
        return Ok(updated);
    }
    let secret_key = format!("account:{}", account.id);
    let password = match password.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => store::get_secret(&secret_key)
            .map_err(|_| AppError::msg("Inserisci la password"))?,
    };
    // Verifica prima di salvare: niente account rotti in lista
    let cred = imap::Credential::Password(password.clone());
    let mut session = imap::connect(&account, &cred).await?;
    let _ = session.logout().await;
    smtp::test(&account, &cred)
        .await
        .map_err(|e| AppError::msg(format!("IMAP ok, ma SMTP non funziona: {e}")))?;

    store::set_secret(&secret_key, &password)?;
    app.pool.drop_account(&account.id).await;
    app.store.save_account(account.clone())?;
    Ok(account)
}

#[tauri::command]
fn oauth_available() -> Vec<&'static str> {
    oauth::available()
}

/// Apre il browser per il consenso, verifica IMAP e SMTP e salva l'account.
#[tauri::command]
async fn oauth_connect(handle: AppHandle, app: State<'_, App>, provider: String) -> Result<Account> {
    use tauri::Emitter;
    use tauri_plugin_opener::OpenerExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    // Un nuovo accesso sostituisce quello eventualmente rimasto in sospeso
    if let Ok(mut pending) = app.oauth_pending.lock() {
        *pending = Some(tx);
    }
    let auth = oauth::authorize(
        &provider,
        |url| {
            // Il frontend tiene l'indirizzo per "Riapri la pagina"
            let _ = handle.emit("oauth-url", url);
            handle
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|e| AppError::msg(format!("Impossibile aprire il browser: {e}")))
        },
        rx,
    )
    .await?;
    let mut account = auth.account;
    // Stesso indirizzo già presente: si aggiorna quello invece di duplicarlo
    if let Some(existing) = app.store.accounts().into_iter().find(|a| a.email.eq_ignore_ascii_case(&account.email)) {
        let tokens = store::get_secret(&format!("oauth:{}", account.id))?;
        oauth::forget(&account.id);
        store::set_secret(&format!("oauth:{}", existing.id), &tokens)?;
        account = Account { id: existing.id, name: existing.name, signature: existing.signature, ..account };
    }
    let cred = imap::Credential::Token(auth.access_token);
    let check: Result<()> = async {
        let mut session = imap::connect(&account, &cred).await?;
        let _ = session.logout().await;
        match smtp::test(&account, &cred).await {
            Ok(()) => Ok(()),
            // SMTP autenticato spento (tipico nei tenant aziendali): basta che funzioni Graph
            Err(e) if account.auth == "oauth-microsoft" => oauth::graph_send_token(&account)
                .await
                .map(|_| ())
                .map_err(|_| e),
            Err(e) => Err(e),
        }
    }
    .await;
    if let Err(e) = check {
        if app.store.account(&account.id).is_err() {
            oauth::forget(&account.id);
        }
        return Err(e);
    }
    app.pool.drop_account(&account.id).await;
    app.store.save_account(account.clone())?;
    Ok(account)
}

/// Caselle condivise a cui l'account Microsoft ha accesso.
#[tauri::command]
async fn shared_mailboxes(app: State<'_, App>, account_id: String) -> Result<Vec<shared::SharedMailbox>> {
    let owner = app.store.account(&account_id)?;
    if owner.auth != "oauth-microsoft" || owner.shared_of.is_some() {
        return Err(AppError::msg("Le caselle condivise sono disponibili per gli account Microsoft 365"));
    }
    let token = oauth::directory_token(&owner)
        .await
        .map_err(|_| AppError::msg("Ricollega l'account Microsoft per cercare le caselle condivise"))?;
    shared::discover(&owner, &token).await
}

/// Attiva una casella condivisa: prova ad aprirla via IMAP e la salva come account.
#[tauri::command]
async fn shared_add(app: State<'_, App>, account_id: String, email: String, name: String) -> Result<Account> {
    let owner = app.store.account(&account_id)?;
    let email = email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::msg("Indirizzo non valido"));
    }
    if let Some(existing) = app.store.accounts().into_iter().find(|a| a.email.eq_ignore_ascii_case(&email)) {
        return Ok(existing);
    }
    let account = shared::child(&owner, &email, name.trim());
    let cred = imap::credential(&account).await?;
    let mut session = imap::connect(&account, &cred).await.map_err(|_| {
        AppError::msg(format!("Non hai accesso a {email}: serve il permesso «Accesso completo» sulla casella"))
    })?;
    let _ = session.logout().await;
    app.store.save_account(account.clone())?;
    Ok(account)
}

#[tauri::command]
fn oauth_cancel(app: State<'_, App>) {
    if let Ok(mut pending) = app.oauth_pending.lock() {
        pending.take();
    }
}

#[tauri::command]
async fn account_remove(app: State<'_, App>, id: String) -> Result<()> {
    app.pool.drop_account(&id).await;
    app.store.remove_account(&id)
}

// ---------- Posta ----------

#[tauri::command]
async fn folders_list(app: State<'_, App>, account_id: String) -> Result<Vec<imap::Folder>> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "sync", |s| imap::folders(s).await)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessagePage {
    messages: Vec<imap::Envelope>,
    total: u32,
    has_more: bool,
}

#[tauri::command]
fn messages_cached(app: State<'_, App>, account_id: String, folder: String) -> Option<MessagePage> {
    app.store.read_cache(&account_id, &folder)
}

#[tauri::command]
async fn messages_list(app: State<'_, App>, account_id: String, folder: String, page: u32) -> Result<MessagePage> {
    let acc = app.store.account(&account_id)?;
    let p = with_imap!(app, acc, "sync", |s| imap::list(s, &folder, page, PAGE_SIZE).await)?;
    let result = MessagePage {
        has_more: (page + 1) * PAGE_SIZE < p.total,
        total: p.total,
        messages: p.messages,
    };
    if page == 0 {
        app.store.write_cache(&account_id, &folder, &result);
    }
    Ok(result)
}

#[tauri::command]
async fn messages_search(app: State<'_, App>, account_id: String, folder: String, query: String) -> Result<Vec<imap::Envelope>> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "sync", |s| imap::search(s, &acc, &folder, &query).await)
}

/// Tiene allineata la cache su disco, così al riavvio non ricompaiono stati vecchi.
fn patch_cache(app: &App, account_id: &str, folder: &str, uids: &[u32], f: impl Fn(&mut imap::Envelope)) {
    if let Some(mut page) = app.store.read_cache::<MessagePage>(account_id, folder) {
        page.messages.iter_mut().filter(|m| uids.contains(&m.uid)).for_each(&f);
        app.store.write_cache(account_id, folder, &page);
    }
}

#[tauri::command]
async fn message_get(app: State<'_, App>, account_id: String, folder: String, uid: u32) -> Result<imap::FullMessage> {
    let acc = app.store.account(&account_id)?;
    let msg = with_imap!(app, acc, "ui", |s| imap::message(s, &folder, uid).await)?;
    patch_cache(&app, &account_id, &folder, &[uid], |m| m.seen = true);
    Ok(msg)
}

/// Non letti di una cartella, contati sulla stessa connessione che ha appena
/// salvato letture e spostamenti: il numero riflette sempre l'ultima modifica.
#[tauri::command]
async fn folder_unseen(app: State<'_, App>, account_id: String, folder: String) -> Result<u32> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "ui", |s| imap::unseen_count(s, &folder).await)
}

#[tauri::command]
async fn message_flag(app: State<'_, App>, account_id: String, folder: String, uids: Vec<u32>, flag: String, value: bool) -> Result<()> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "ui", |s| imap::set_flag(s, &folder, &uids, &flag, value).await)?;
    patch_cache(&app, &account_id, &folder, &uids, |m| match flag.as_str() {
        "seen" => m.seen = value,
        "flagged" => m.flagged = value,
        _ => {}
    });
    Ok(())
}

#[tauri::command]
async fn message_move(app: State<'_, App>, account_id: String, folder: String, uids: Vec<u32>, dest: String) -> Result<()> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "ui", |s| imap::move_to(s, &folder, &uids, &dest).await)?;
    drop_from_cache(&app, &account_id, &folder, &uids);
    Ok(())
}

fn drop_from_cache(app: &App, account_id: &str, folder: &str, uids: &[u32]) {
    if let Some(mut page) = app.store.read_cache::<MessagePage>(account_id, folder) {
        page.messages.retain(|m| !uids.contains(&m.uid));
        app.store.write_cache(account_id, folder, &page);
    }
}

#[tauri::command]
async fn message_delete_forever(app: State<'_, App>, account_id: String, folder: String, uids: Vec<u32>) -> Result<()> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "ui", |s| imap::delete_forever(s, &folder, &uids).await)?;
    drop_from_cache(&app, &account_id, &folder, &uids);
    Ok(())
}

/// Salva l'allegato in Download (senza sovrascrivere) e restituisce il percorso.
#[tauri::command]
async fn attachment_save(handle: AppHandle, app: State<'_, App>, account_id: String, folder: String, uid: u32, index: usize) -> Result<String> {
    let acc = app.store.account(&account_id)?;
    let (name, bytes) = with_imap!(app, acc, "ui", |s| imap::attachment(s, &folder, uid, index).await)?;
    let dir = handle
        .path()
        .download_dir()
        .map_err(|_| AppError::msg("Cartella Download non trovata"))?;
    let safe: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let path = std::path::Path::new(&safe);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("allegato").to_string();
    let ext = path.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let mut target = dir.join(&safe);
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    tokio::fs::write(&target, bytes).await?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
async fn mail_send(app: State<'_, App>, account_id: String, mut draft: smtp::Draft, sent_folder: Option<String>) -> Result<()> {
    let acc = app.store.account(&account_id)?;
    // Inoltro: gli allegati originali si prendono dal server, non passano dalla webview
    if let Some(fwd) = draft.forward_of.take() {
        let src = match fwd.account_id.as_deref() {
            Some(id) if id != acc.id => app.store.account(id)?,
            _ => acc.clone(),
        };
        let originals = with_imap!(app, src, "ui", |s| imap::all_attachments(s, &fwd.folder, fwd.uid).await)?;
        draft.attachments.extend(originals);
    }
    // Account Microsoft: invio via Graph, che funziona anche con l'SMTP autenticato spento.
    // Se manca il consenso a Mail.Send (es. account personali) si ripiega su SMTP.
    if acc.auth == "oauth-microsoft" {
        if let Ok(token) = oauth::graph_send_token(&acc).await {
            let (_, raw) = smtp::build(&acc, &draft, true)?;
            if raw.len() > smtp::GRAPH_MAX_MIME {
                share_large_attachments(&acc, &token, &mut draft, raw.len()).await?;
            }
            let (_, raw) = smtp::build(&acc, &draft, true)?;
            return smtp::graph_send(&token, &raw, acc.shared_of.as_ref().map(|_| acc.email.as_str())).await;
        }
    }
    let cred = imap::credential(&acc).await?;
    let (msg, raw) = smtp::build(&acc, &draft, false)?;
    smtp::send(&acc, &cred, msg).await?;
    // Gmail e Outlook.com salvano da soli la copia inviata
    let auto_saves = ["gmail", "googlemail", "outlook.office365", "office365"]
        .iter()
        .any(|h| acc.smtp_host.contains(h));
    if let (Some(folder), false) = (sent_folder, auto_saves) {
        let _ = with_imap!(app, acc, "ui", |s| imap::append(s, &folder, &raw).await);
    }
    Ok(())
}

#[tauri::command]
async fn inbox_check(app: State<'_, App>, account_id: String, after_uid: Option<u32>) -> Result<imap::InboxCheck> {
    let acc = app.store.account(&account_id)?;
    with_imap!(app, acc, "sync", |s| imap::inbox_check(s, after_uid).await)
}

/// Sposta su OneDrive gli allegati più grandi finché il messaggio rientra nel limite di Graph,
/// e mette i link nel corpo al posto del segnaposto.
async fn share_large_attachments(acc: &Account, token: &str, draft: &mut smtp::Draft, mut size: usize) -> Result<()> {
    let recipients: Vec<String> = draft
        .to
        .iter()
        .chain(&draft.cc)
        .chain(&draft.bcc)
        .filter_map(|r| r.trim().parse::<lettre::message::Mailbox>().ok())
        .map(|m| m.email.to_string())
        .collect();
    let my_domain = acc.email.rsplit('@').next().unwrap_or_default().to_lowercase();
    let same_org = !recipients.is_empty()
        && recipients.iter().all(|r| r.rsplit('@').next().unwrap_or_default().to_lowercase() == my_domain);

    let mut order: Vec<usize> = (0..draft.attachments.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(draft.attachments[i].data.len()));
    let mut moved = Vec::new();
    for i in order {
        if size <= smtp::GRAPH_MAX_MIME * 9 / 10 {
            break;
        }
        size = size.saturating_sub(draft.attachments[i].data.len());
        moved.push(i);
    }
    let mut shared = Vec::new();
    for &i in &moved {
        shared.push(onedrive::upload_and_share(token, &draft.attachments[i], &recipients, same_org).await?);
    }
    moved.sort_unstable_by(|a, b| b.cmp(a));
    for i in moved {
        draft.attachments.remove(i);
    }
    if !shared.is_empty() {
        let html = onedrive::links_html(&shared);
        draft.html = if draft.html.contains(smtp::LINKS_HTML_MARK) {
            draft.html.replacen(smtp::LINKS_HTML_MARK, &html, 1)
        } else {
            format!("{}{html}", draft.html)
        };
        let text = onedrive::links_text(&shared);
        draft.text = if draft.text.contains(smtp::LINKS_TEXT_MARK) {
            draft.text.replacen(smtp::LINKS_TEXT_MARK, &text, 1)
        } else {
            format!("{}\n\n{text}", draft.text)
        };
    }
    Ok(())
}

/// Destinatari suggeriti dalla rubrica dell'account da cui si invia.
#[tauri::command]
async fn contacts_search(app: State<'_, App>, account_id: String, query: String) -> Result<Vec<contacts::Contact>> {
    let acc = app.store.account(&account_id)?;
    contacts::search(&acc, &query).await
}

// ---------- Calendario ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CalendarResult {
    events: Vec<calendar::CalEvent>,
    accounts: Vec<calendar::AccountCalendar>,
}

/// Eventi di tutti gli account nell'intervallo, chiesti in parallelo.
#[tauri::command]
async fn calendar_events(app: State<'_, App>, start: i64, end: i64) -> Result<CalendarResult> {
    let accounts = app.store.accounts();
    let results = futures::future::join_all(accounts.iter().map(|a| calendar::events(a, start, end))).await;
    let mut out = CalendarResult { events: vec![], accounts: vec![] };
    for (acc, r) in accounts.iter().zip(results) {
        let (status, message) = match r {
            Ok(evs) => {
                out.events.extend(evs);
                ("ok", None)
            }
            Err(e) if e.to_string() == "unsupported" => ("unsupported", None),
            Err(e) => ("error", Some(e.to_string())),
        };
        out.accounts.push(calendar::AccountCalendar { account_id: acc.id.clone(), status: status.into(), message });
    }
    out.events.sort_by_key(|e| e.start);
    Ok(out)
}

// ---------- AI ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiState {
    settings: Settings,
    has_anthropic_key: bool,
    has_openai_key: bool,
}

#[tauri::command]
fn settings_get(app: State<'_, App>) -> AiState {
    AiState {
        settings: app.store.settings(),
        has_anthropic_key: store::has_secret(&ai::key_name("anthropic")),
        has_openai_key: store::has_secret(&ai::key_name("openai")),
    }
}

#[tauri::command]
fn settings_save(app: State<'_, App>, settings: Settings) -> Result<()> {
    app.store.save_settings(&settings)
}

#[tauri::command]
fn ai_key_set(provider: String, key: String) -> Result<()> {
    let name = ai::key_name(&provider);
    if key.trim().is_empty() {
        let _ = store::delete_secret(&name);
        Ok(())
    } else {
        store::set_secret(&name, key.trim())
    }
}

#[tauri::command]
async fn ai_models(app: State<'_, App>, provider: String) -> Result<Vec<String>> {
    if provider == "local" {
        return Ok(ai::local_status(&app.store.settings()).await.models.into_iter().map(|m| m.name).collect());
    }
    let key = store::get_secret(&ai::key_name(&provider))
        .map_err(|_| AppError::msg("Prima inserisci la chiave API"))?;
    ai::models(&provider, &key, &app.store.settings()).await
}

#[tauri::command]
fn ai_run(
    handle: AppHandle,
    app: State<'_, App>,
    id: String,
    system: String,
    prompt: Option<String>,
    messages: Option<Vec<ai::ChatMsg>>,
) -> Result<()> {
    let settings = app.store.settings();
    // L'AI locale non ha chiavi
    let key = if settings.ai_provider == "local" {
        String::new()
    } else {
        store::get_secret(&ai::key_name(&settings.ai_provider)).map_err(|_| {
            AppError::msg("Nessuna chiave API: aprila nelle Impostazioni → Assistente")
        })?
    };
    let messages = messages.unwrap_or_else(|| {
        vec![ai::ChatMsg { role: "user".into(), content: prompt.unwrap_or_default() }]
    });
    ai::start(handle, &app.jobs, id, system, messages, settings, key);
    Ok(())
}

#[tauri::command]
async fn local_ai_status(app: State<'_, App>) -> Result<ai::LocalStatus> {
    Ok(ai::local_status(&app.store.settings()).await)
}

#[tauri::command]
fn local_ai_pull(handle: AppHandle, app: State<'_, App>, model: String) {
    let settings = app.store.settings();
    tauri::async_runtime::spawn(ai::pull(handle, settings, model));
}

#[tauri::command]
fn ai_cancel(app: State<'_, App>, id: String) {
    ai::cancel(&app.jobs, &id);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let config = app.path().app_config_dir()?;
            let cache = app.path().app_cache_dir()?;
            app.manage(App {
                store: Store::new(config, cache),
                pool: imap::Pool::default(),
                jobs: ai::Jobs::default(),
                oauth_pending: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            accounts_list,
            account_save,
            account_remove,
            oauth_available,
            oauth_connect,
            oauth_cancel,
            shared_mailboxes,
            shared_add,
            inbox_check,
            calendar_events,
            contacts_search,
            folders_list,
            messages_cached,
            messages_list,
            messages_search,
            message_get,
            message_flag,
            folder_unseen,
            message_move,
            message_delete_forever,
            attachment_save,
            mail_send,
            settings_get,
            settings_save,
            ai_key_set,
            ai_models,
            ai_run,
            ai_cancel,
            local_ai_status,
            local_ai_pull,
        ])
        .run(tauri::generate_context!())
        .expect("errore avvio Cobalto");
}
