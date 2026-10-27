# Cobalto

Client di posta desktop per macOS, Windows e Linux. Pesa pochi megabyte, è blu e bianco e ha un assistente AI integrato: **Claude** oppure **GPT**, a scelta.

## Avvio

```bash
npm install
npm run tauri dev        # sviluppo
npm run tauri build      # pacchetti di release (.app/.dmg, .msi/.exe, .deb/.AppImage)
```

Requisiti: Rust ≥ 1.88 e Node ≥ 20. Su Linux servono anche i [prerequisiti Tauri](https://tauri.app/start/prerequisites/) e `libssl-dev`.

## Cosa fa

- **Posta IMAP/SMTP**: Gmail, iCloud, Aruba, Libero, Yahoo e qualsiasi server con IMAP. I provider vengono riconosciuti dall'indirizzo email.
- **Si apre subito**: la prima pagina di ogni cartella viene salvata in cache e mostrata prima della sincronizzazione.
- **Due connessioni per account**: aprire una mail non aspetta mai la sincronizzazione della lista.
- **Annulla**: archivia ed elimina sono immediati; per 4,5 secondi puoi annullare.
- **Privacy**: immagini remote bloccate di default (pixel traccianti), con la possibilità di sbloccarle una volta o sempre per quel mittente. HTML ripulito con DOMPurify dentro un iframe senza script.
- **Firma ricca**: grassetto, link, immagini (ridimensionate in automatico), incolla da Outlook o Gmail. All'invio le immagini diventano allegati inline `cid:`, così si vedono anche in Gmail e Outlook.
- **Assistente AI** (nota a margine accanto all'email):
  - riassunto con la sezione "Da fare"
  - risposte in un clic (Accetta, Declina, Chiedi dettagli…) o proposte dall'AI
  - domande libere sull'email
  - nel composer: scrivi da una descrizione, riscrivi (più professionale, più breve, correggi, in inglese…), con "Ripristina"
  - Claude (Anthropic), GPT (OpenAI o compatibili) oppure **AI locale con Ollama**: gratis, i testi non lasciano il Mac
- **Assistente** (`⌘J`): chat sulla cartella aperta e sull'email che stai leggendo. Cita le email con numeri cliccabili e trasforma le risposte in bozze.
- **Calendario multi-account** (`⌘2`): settimana e agenda con gli eventi di tutti gli account Microsoft e Google, ognuno nella sua sfumatura di blu. Riconosce le chiamate Teams, Meet, Zoom e Webex con il pulsante **Partecipa**. Mostra "Tra 5 minuti…" nella sidebar e invia una notifica. L'assistente vede anche gli appuntamenti.
- **Caselle condivise** (Microsoft 365): in Impostazioni, sotto l'account, "Cerca" trova le caselle condivise a cui hai accesso e un interruttore le mostra in Cobalto, con il tuo stesso accesso. Si possono aggiungere anche per indirizzo. L'invio "come" la casella richiede il permesso Invia come.
- **Notifiche** di nuova posta su tutti gli account e numero dei non letti sull'icona.
- **Inoltro** con gli allegati originali.
- **Tastiera**: `J`/`K`, `R`, `A`, `F`, `E`, `#`, `S`, `U`, `C`, `/`, `⌘K` (palette dei comandi), `⌘↵` (invia).

## OAuth (Microsoft 365 / Outlook.com, Google)

Microsoft ha disattivato l'accesso IMAP/SMTP con password sia per gli account personali sia per quelli aziendali: serve OAuth. Cobalto usa PKCE con redirect su `http://localhost` (porta casuale) e salva i token nel portachiavi. Gli ID si mettono in `src-tauri/.cargo/config.toml` (copialo da `config.toml.example`; è escluso da git perché contiene il segreto client di Google) e poi si ricompila.

### Microsoft (anche aziendale, es. dominio proprio su Microsoft 365)

1. [portal.azure.com](https://portal.azure.com) → **Microsoft Entra ID** → **Registrazioni app** → **Nuova registrazione**
   - Nome: `Cobalto`
   - Tipi di account: *Account in qualsiasi directory organizzativa e account Microsoft personali*
   - URI di reindirizzamento: piattaforma **Client pubblico/nativo (per dispositivi mobili e desktop)** → `http://localhost`
2. Copia l'**ID applicazione (client)** in `COBALTO_MS_CLIENT_ID`.
3. I permessi `IMAP.AccessAsUser.All` e `SMTP.Send` vengono chiesti al primo accesso. Se nel tenant gli utenti non possono dare consenso alle app, un amministratore deve approvare Cobalto (Entra ID → Applicazioni aziendali → Cobalto → Autorizzazioni → *Concedi consenso amministratore*).
4. Sulla casella devono essere attivi **IMAP** e **SMTP autenticato**: Exchange admin center → Destinatari → Cassette postali → *utente* → **Gestisci le app di posta elettronica**. Se lo SMTP autenticato è disattivato a livello di organizzazione: `Set-TransportConfig -SmtpClientAuthenticationDisabled $false`.

Permessi delegati usati (Microsoft Graph): IMAP.AccessAsUser.All, SMTP.Send, Mail.Send, Files.ReadWrite, Calendars.Read, Mail.Read.Shared, Mail.Send.Shared, User.Read.All (per trovare le caselle condivise), offline_access, User.Read.

### Google

1. [console.cloud.google.com](https://console.cloud.google.com) → nuovo progetto → **Schermata consenso OAuth** (Esterno; aggiungi il tuo indirizzo tra gli utenti di test).
2. **Credenziali** → **Crea ID client OAuth** → tipo **App desktop**.
3. Copia ID client e secret in `COBALTO_GOOGLE_CLIENT_ID` e `COBALTO_GOOGLE_CLIENT_SECRET` (per le app desktop Google considera pubblico il secret).
4. Per il calendario: **API e servizi → Libreria → Google Calendar API → Abilita**, e in **Accesso ai dati** aggiungi l'ambito `https://www.googleapis.com/auth/calendar.readonly`. Gli account Google già collegati vanno ricollegati una volta.

Attenzione: finché l'app Google è in modalità *test*, l'accesso scade ogni 7 giorni. Per uso personale la **password per le app** di Gmail resta più comoda; la pubblicazione richiede la verifica di Google per lo scope `mail.google.com`.

## AI locale

Con Ollama installato, in Impostazioni → Assistente scegli **Locale**: Cobalto mostra i modelli installati e scarica quelli consigliati con un clic. Il "ragionamento" dei modelli è spento: per la posta servono risposte rapide.

Prova su un Mac M5 con 32 GB, con veri compiti di posta in italiano (riassunto con cose da fare e risposta su indicazione):

| Modello | Velocità | Qualità |
|---|---|---|
| `gemma4:26b` (consigliato) | 44 token/s, primo testo in 0,3 s | italiano naturale, coglie tutte le scadenze, rispetta le istruzioni |
| `qwen3.8:27b` | 14–16 token/s, primo testo in 1,5 s | eccellente (nota da solo chi è già in copia), ma 3 volte più lento |
| `qwen3:8b` | 23 token/s | buono, ma nelle risposte ricopia le istruzioni e aggiunge la firma |

## Allegati grandi (Microsoft 365)

Oltre circa 3 MB gli allegati vanno su OneDrive (cartella "Allegati Cobalto") e nel messaggio compare una scheda con il link. Il link è di sola lettura: prima si prova a crearlo solo per i destinatari, poi per l'organizzazione (se scrivi a colleghi), e solo come ultima scelta per chiunque abbia il link.

## Anteprima della grafica

`npm run dev` aperto nel browser (fuori da Tauri) mostra l'app con dati di prova (`src/dev/preview.ts`), utile per lavorare sull'interfaccia. Non entra nella build di produzione.

## Sicurezza

- Le password delle caselle e le chiavi API stanno nel **portachiavi di sistema** (Keychain, Credential Manager, Secret Service), mai in chiaro su disco.
- Le chiamate AI partono dal backend Rust: la chiave non entra mai nella webview.

## Struttura

```
src-tauri/src/
  lib.rs     comandi Tauri, pool di connessioni
  imap.rs    cartelle, liste, messaggi, flag, spostamenti (async-imap)
  smtp.rs    composizione e invio (lettre), immagini inline della firma
  ai.rs      Claude / GPT in streaming (SSE), elenco modelli
  store.rs   account e impostazioni (JSON), segreti (keyring), cache
src/
  lib/state.svelte.ts   stato dell'app (Svelte 5 runes)
  lib/ai.ts             prompt e streaming lato UI
  lib/*.svelte          Sidebar, MessageList, Reader, AiMargin, Composer, …
```

## Limiti noti / prossimi passi

- Backend **Microsoft Graph** per i tenant che non permettono IMAP/SMTP autenticato.
- Notifiche istantanee con IMAP IDLE (oggi il controllo è ogni minuto) e posta in arrivo unificata tra account.
- Bozze salvate sul server e conversazioni raggruppate (threading).
