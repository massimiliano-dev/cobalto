// Anteprima nel browser (solo `npm run dev` fuori da Tauri): dati finti per lavorare sulla grafica.
// Non entra mai nella build di produzione.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

const now = Math.floor(Date.now() / 1000);
const h = 3600;
const d = 86400;

const account = {
  id: "demo",
  name: "Mario Rossi",
  email: "mario.rossi@acme.example",
  username: "mario.rossi@acme.example",
  imapHost: "outlook.office365.com",
  imapPort: 993,
  smtpHost: "smtp.office365.com",
  smtpPort: 587,
  smtpSecurity: "starttls",
  signature: '<p><strong>Mario Rossi</strong><br>Acme Srl · <a href="https://acme.example">acme.example</a></p>',
  auth: "oauth-microsoft",
};

const gmail = {
  ...account,
  id: "gmail",
  name: "Max",
  email: "mario.rossi@example.com",
  username: "mario.rossi@example.com",
  imapHost: "imap.gmail.com",
  smtpHost: "smtp.gmail.com",
  auth: "oauth-google",
  signature: "",
};

function calendarEvents(start: number, end: number) {
  const day0 = new Date();
  day0.setHours(0, 0, 0, 0);
  const at = (dayOffset: number, h: number, m = 0) => {
    const d = new Date(day0);
    d.setDate(d.getDate() + dayOffset);
    d.setHours(h, m, 0, 0);
    return d.getTime();
  };
  const ymd = (off: number) => {
    const d = new Date(day0);
    d.setDate(d.getDate() + off);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };
  const soon = Date.now() + 8 * 60_000;
  const base = { location: "", organizer: "", attendees: [], joinUrl: null, joinKind: null, webLink: "https://outlook.office.com", preview: "", showAs: "busy", cancelled: false, allDay: false, startDate: null, endDate: null };
  const evs = [
    { ...base, id: "e1", accountId: "demo", title: "Revisione preventivo Rossi Srl", start: soon, end: soon + 30 * 60_000, location: "Microsoft Teams", organizer: "Giulia Bianchi", attendees: ["Giulia Bianchi", "Elena Conti", "Mario Rossi"], joinUrl: "https://teams.microsoft.com/l/meetup-join/demo", joinKind: "teams", preview: "Rivediamo insieme le voci del preventivo 2027 e lo sconto valido fino a venerdì." },
    { ...base, id: "e2", accountId: "demo", title: "Standup tecnico", start: at(0, 9), end: at(0, 9, 30), joinUrl: "https://teams.microsoft.com/l/meetup-join/standup", joinKind: "teams", location: "Microsoft Teams" },
    { ...base, id: "e3", accountId: "demo", title: "Sopralluogo cliente ABC", start: at(1, 10), end: at(1, 12, 30), location: "Via Roma 12, Milano" },
    { ...base, id: "e4", accountId: "demo", title: "Call fornitore switch", start: at(1, 11), end: at(1, 12), joinUrl: "https://us02web.zoom.us/j/881234", joinKind: "zoom", showAs: "tentative" },
    { ...base, id: "e5", accountId: "gmail", title: "Palestra", start: at(1, 18, 30), end: at(1, 19, 45), webLink: "https://calendar.google.com", showAs: "free" },
    { ...base, id: "e6", accountId: "gmail", title: "Cena con Sara e Luca", start: at(2, 20), end: at(2, 22, 30), location: "Trattoria da Mario", webLink: "https://calendar.google.com" },
    { ...base, id: "e7", accountId: "demo", title: "Riunione di giovedì · nuovo gestionale", start: at(3, 15), end: at(3, 16, 30), location: "Sala riunioni", organizer: "Marco Ferrari" },
    { ...base, id: "e8", accountId: "gmail", title: "Call con commercialista", start: at(3, 17), end: at(3, 17, 30), joinUrl: "https://meet.google.com/abc-defg-hij", joinKind: "meet", webLink: "https://calendar.google.com" },
    { ...base, id: "e9", accountId: "demo", title: "Scadenza conferma preventivo", allDay: true, start: 0, end: 0, startDate: ymd(4), endDate: ymd(5) },
    { ...base, id: "e10", accountId: "gmail", title: "Ponte di Ognissanti", allDay: true, start: 0, end: 0, startDate: ymd(5), endDate: ymd(7), showAs: "free", webLink: "https://calendar.google.com" },
    { ...base, id: "e11", accountId: "demo", title: "Formazione Microsoft 365", start: at(-1, 14), end: at(-1, 17), location: "Online", joinUrl: "https://teams.microsoft.com/l/meetup-join/corso", joinKind: "teams" },
    { ...base, id: "e12", accountId: "demo", title: "Pranzo con Elena", start: at(0, 13), end: at(0, 14) },
  ];
  return {
    events: evs.filter((e) => (e.allDay ? true : e.end > start && e.start < end)),
    accounts: [
      { accountId: "demo", status: "ok", message: null },
      { accountId: "gmail", status: "ok", message: null },
    ],
  };
}

const folders = [
  { path: "INBOX", name: "Inbox", depth: 0, role: "inbox", unseen: 4 },
  { path: "Drafts", name: "Drafts", depth: 0, role: "drafts", unseen: 0 },
  { path: "Sent Items", name: "Sent Items", depth: 0, role: "sent", unseen: 0 },
  { path: "Archive", name: "Archive", depth: 0, role: "archive", unseen: 0 },
  { path: "Junk Email", name: "Junk Email", depth: 0, role: "junk", unseen: 0 },
  { path: "Deleted Items", name: "Deleted Items", depth: 0, role: "trash", unseen: 0 },
  { path: "Clienti", name: "Clienti", depth: 0, role: "", unseen: 2 },
  { path: "Clienti/Rossi Srl", name: "Rossi Srl", depth: 1, role: "", unseen: 0 },
  { path: "Fornitori", name: "Fornitori", depth: 0, role: "", unseen: 0 },
];

const people = [
  ["Giulia Bianchi", "g.bianchi@rossisrl.it"],
  ["Marco Ferrari", "marco.ferrari@studioferrari.it"],
  ["Aruba", "comunicazioni@staff.aruba.it"],
  ["Elena Conti", "elena.conti@acme.example"],
  ["Luca Romano", "l.romano@fornitore.it"],
  ["Microsoft 365", "no-reply@microsoft.com"],
  ["Sara Greco", "sara.greco@gmail.com"],
  ["Davide Esposito", "d.esposito@clienteabc.it"],
];

const rows: [number, string, string, number, boolean, boolean, boolean][] = [
  [0, "Preventivo rinnovo infrastruttura 2027", "Ciao Massimiliano, come d'accordo ti mando in allegato il preventivo aggiornato per il rinnovo dei server. Avremmo bisogno di una conferma entro venerdì per bloccare i prezzi.", 0.4 * h, false, false, true],
  [1, "Re: Riunione di giovedì", "Perfetto, allora ci vediamo giovedì alle 15 in sala riunioni. Porto io le slide sul nuovo gestionale.", 2 * h, false, true, false],
  [3, "Ferie di dicembre — calendario team", "Ho preparato il calendario delle ferie: manca solo la tua conferma per la settimana tra Natale e Capodanno.", 5 * h, false, false, false],
  [2, "Il tuo dominio acme.example scade tra 30 giorni", "Rinnova ora per evitare interruzioni del servizio di posta e del sito web.", 9 * h, true, false, false],
  [4, "Consegna switch rimandata", "Purtroppo il corriere ha segnalato un ritardo: la consegna slitta a martedì prossimo. Ci scusiamo per il disagio.", 1 * d + 2 * h, true, false, false],
  [5, "Nuovo accesso al tuo account", "Abbiamo rilevato un nuovo accesso da Mac (Milano, Italia). Se sei stato tu, puoi ignorare questo messaggio.", 1 * d + 7 * h, true, false, false],
  [6, "Foto della cena 📸", "Eccole finalmente! Che bella serata, dobbiamo rifarlo prima di Natale.", 2 * d, true, true, true],
  [7, "Richiesta assistenza VPN", "Buongiorno, da stamattina due colleghi non riescono a collegarsi alla VPN aziendale. Potete verificare?", 3 * d, true, false, false],
  [0, "Contratto di manutenzione firmato", "Vi rimandiamo il contratto controfirmato. Restiamo in attesa del calendario degli interventi.", 6 * d, true, false, true],
  [1, "Proposta collaborazione", "Ti scrivo per proporti una collaborazione sul progetto di digitalizzazione dello studio.", 12 * d, true, false, false],
  [3, "Report mensile ottobre", "In allegato il report mensile con i ticket chiusi e i tempi medi di risposta.", 20 * d, true, false, true],
  [4, "Listino 2027", "Vi inviamo il nuovo listino in vigore dal 1° gennaio.", 34 * d, true, false, true],
];

let messages = rows.map(([p, subject, preview, ago, seen, answered, att], i) => ({
  uid: 500 - i,
  subject,
  from: { name: people[p][0], email: people[p][1] },
  to: [{ name: "Mario Rossi", email: "mario.rossi@acme.example" }],
  date: now - ago,
  preview,
  seen,
  flagged: i === 2,
  answered,
  hasAttachments: att,
  size: 20000,
}));

function fullMessage(uid: number) {
  const m = messages.find((x) => x.uid === uid) ?? messages[0];
  return {
    uid: m.uid,
    subject: m.subject,
    from: m.from,
    to: m.to,
    cc: m.uid === 500 ? [{ name: "Elena Conti", email: "elena.conti@acme.example" }] : [],
    replyTo: [],
    date: m.date,
    flagged: m.flagged,
    html:
      m.uid === 500
        ? `<p>Ciao Massimiliano,</p><p>come d'accordo ti mando in allegato il <strong>preventivo aggiornato</strong> per il rinnovo dei server e dello storage.</p><p>Rispetto alla versione precedente:</p><ul><li>abbiamo aggiunto il secondo nodo per l'alta affidabilità;</li><li>la garanzia passa a 5 anni on-site;</li><li>lo sconto del 12% è valido solo con conferma entro <strong>venerdì 31 ottobre</strong>.</li></ul><p>Se vuoi possiamo fare una call martedì o mercoledì per rivederlo insieme.</p><p>Un saluto,<br>Giulia</p><p style="color:#667">Giulia Bianchi<br>Ufficio acquisti · Rossi Srl<br><img src="https://rossisrl.it/logo.png" width="120"></p>`
        : null,
    text: m.preview + "\n\nUn saluto,\n" + m.from.name,
    attachments: m.hasAttachments
      ? [
          { index: 0, name: "Preventivo_2027_rev3.pdf", mime: "application/pdf", size: 482_000, inline: false },
          { index: 1, name: "Specifiche_tecniche.xlsx", mime: "application/vnd.ms-excel", size: 61_000, inline: false },
        ]
      : [],
    messageId: `<${m.uid}@demo>`,
    references: [],
    listUnsubscribe: null,
  };
}

const settings = {
  settings: {
    aiProvider: "local",
    anthropicModel: "claude-sonnet-5-5",
    openaiModel: "gpt-5",
    openaiBaseUrl: "https://api.openai.com/v1",
    aiLanguage: "italiano",
    localModel: "gemma4:26b",
    localUrl: "http://localhost:11434",
  },
  hasAnthropicKey: false,
  hasOpenaiKey: false,
};

const DEMO_REPLY =
  "- **Preventivo** per il rinnovo di server e storage, con secondo nodo per l'alta affidabilità.\n" +
  "- Garanzia estesa a **5 anni on-site**.\n" +
  "- Sconto del 12% valido solo con conferma entro venerdì.\n\n" +
  "**Da fare**\n- Confermare il preventivo entro **venerdì 31 ottobre** [1]\n- Fissare una call martedì o mercoledì";

function stream(id: string, text: string) {
  const parts = text.match(/.{1,6}/gs) ?? [];
  parts.forEach((p, i) => setTimeout(() => emit("ai-chunk", { id, delta: p }), 300 + i * 22));
  setTimeout(() => emit("ai-done", { id, error: null }), 300 + parts.length * 22 + 50);
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function installPreview() {
  mockWindows("main");
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, any>;
      switch (cmd) {
        case "accounts_list":
          return [account, gmail];
        case "calendar_events":
          await wait(250);
          return calendarEvents(a.start, a.end);
        case "settings_get":
          return settings;
        case "folders_list":
          await wait(300);
          return folders;
        case "messages_cached":
          return null;
        case "messages_list":
          await wait(500);
          return a.folder === "INBOX"
            ? { messages, total: 1284, hasMore: false }
            : { messages: messages.slice(4, 9), total: 5, hasMore: false };
        case "messages_search":
          await wait(400);
          return messages.filter((m) => (m.subject + m.preview).toLowerCase().includes(String(a.query).toLowerCase()));
        case "message_get":
          await wait(250);
          return fullMessage(a.uid);
        case "inbox_check":
          return { unseen: 4, lastUid: 500, new: [] };
        case "message_move":
        case "message_delete_forever":
          messages = messages.filter((m) => !a.uids.includes(m.uid));
          return null;
        case "ai_run":
          stream(a.id, DEMO_REPLY);
          return null;
        case "local_ai_status":
          return { running: true, models: [{ name: "gemma4:26b", size: 18e9 }, { name: "qwen3:8b", size: 5.2e9 }] };
        case "ai_models":
          return ["gemma4:26b", "qwen3:8b"];
        case "oauth_available":
          return ["microsoft", "google"];
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
