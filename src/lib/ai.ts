import { listen } from "@tauri-apps/api/event";
import { api, errorText, type ChatMsg, type FullMessage, type Settings } from "./api";
import { displayName, formatAddr, fullDate } from "./format";

type Handlers = {
  onDelta: (text: string) => void;
  onDone: (error: string | null) => void;
};

const running = new Map<string, Handlers>();
let listening: Promise<void> | null = null;

function ensureListeners() {
  listening ??= (async () => {
    await listen<{ id: string; delta: string }>("ai-chunk", (e) => {
      running.get(e.payload.id)?.onDelta(e.payload.delta);
    });
    await listen<{ id: string; error: string | null }>("ai-done", (e) => {
      const h = running.get(e.payload.id);
      running.delete(e.payload.id);
      h?.onDone(e.payload.error);
    });
  })();
  return listening;
}

export interface AiJob {
  cancel: () => void;
}

/** Avvia una generazione in streaming. `onText` riceve il testo accumulato. */
export async function generate(
  system: string,
  prompt: string | ChatMsg[],
  onText: (full: string) => void,
  onDone: (error: string | null) => void,
): Promise<AiJob> {
  await ensureListeners();
  const id = crypto.randomUUID();
  let full = "";
  running.set(id, {
    onDelta: (d) => {
      full += d;
      onText(full);
    },
    onDone,
  });
  try {
    const messages: ChatMsg[] = typeof prompt === "string" ? [{ role: "user", content: prompt }] : prompt;
    await api.aiRun(id, system, messages);
  } catch (e) {
    running.delete(id);
    onDone(errorText(e));
  }
  return {
    cancel: () => {
      running.delete(id);
      api.aiCancel(id);
      onDone(null);
    },
  };
}

// ---------- Prompt ----------

const MAX_BODY = 14000;

export function messageAsText(m: FullMessage): string {
  let body = m.text ?? "";
  if (!body && m.html) {
    const doc = new DOMParser().parseFromString(m.html, "text/html");
    doc.querySelectorAll("style,script,head").forEach((n) => n.remove());
    body = doc.body.innerText ?? doc.body.textContent ?? "";
  }
  body = body.replace(/\n{3,}/g, "\n\n").trim();
  if (body.length > MAX_BODY) body = body.slice(0, MAX_BODY) + "\n[…testo troncato…]";
  return [
    `Da: ${formatAddr(m.from)}`,
    `A: ${m.to.map(formatAddr).join(", ")}`,
    m.cc.length ? `Cc: ${m.cc.map(formatAddr).join(", ")}` : "",
    `Data: ${fullDate(m.date)}`,
    `Oggetto: ${m.subject}`,
    m.attachments.filter((a) => !a.inline).length
      ? `Allegati: ${m.attachments.filter((a) => !a.inline).map((a) => a.name).join(", ")}`
      : "",
    "",
    body,
  ]
    .filter((l, i) => l !== "" || i > 5)
    .join("\n");
}

export const prompts = {
  system: (lang: string, me: string) =>
    `Sei l'assistente di posta integrato in Cobalto, il client email di ${me}. ` +
    `Scrivi in ${lang} salvo diversa indicazione. Sei conciso, concreto e non inventi fatti che non sono nell'email. ` +
    `Non usare preamboli come "Ecco il riassunto": vai dritto al contenuto.`,

  summary: (m: FullMessage) =>
    `Riassumi questa email in 2-4 punti elenco brevi. Se ci sono richieste, scadenze o cose da fare per me, ` +
    `aggiungi una sezione finale "**Da fare**" con un elenco puntato (con le date se presenti). ` +
    `Se non c'è nulla da fare, ometti la sezione.\n\n---\n${messageAsText(m)}`,

  reply: (m: FullMessage, intent: string, me: string, signature: string) =>
    `Scrivi la risposta a questa email per conto di ${me}.\n` +
    (intent ? `Cosa voglio dire: ${intent}\n` : `Proponi una risposta appropriata e cordiale.\n`) +
    `Rispondi nella stessa lingua dell'email originale. Scrivi solo il corpo del messaggio (saluto iniziale incluso, ` +
    `nessun oggetto, nessuna citazione dell'originale). ` +
    (signature ? `Chiudi con un saluto ma NON aggiungere la firma: verrà aggiunta automaticamente.` : `Chiudi con un saluto e il nome "${me}".`) +
    `\n\n---\nEmail a cui rispondere:\n${messageAsText(m)}`,

  ask: (m: FullMessage, question: string) =>
    `Rispondi alla domanda usando solo le informazioni nell'email. Se l'informazione non c'è, dillo.\n` +
    `Domanda: ${question}\n\n---\n${messageAsText(m)}`,

  compose: (intent: string, to: string, subject: string, me: string) =>
    `Scrivi un'email per conto di ${me}.\n` +
    (to ? `Destinatario: ${to}\n` : "") +
    (subject ? `Oggetto: ${subject}\n` : "") +
    `Cosa deve dire: ${intent}\n\n` +
    `Prima riga: "Oggetto: …" con un oggetto efficace. Poi una riga vuota e il corpo (saluto iniziale incluso). ` +
    `Non aggiungere firme.`,

  rewrite: (text: string, how: string) =>
    `Riscrivi il testo seguente: ${how}. Mantieni il significato, la lingua (salvo richiesta di traduzione) ` +
    `e i dati concreti (nomi, date, cifre). Restituisci solo il testo riscritto.\n\n---\n${text}`,
};

export const replyIntents = [
  { label: "Accetta", intent: "Accetto/confermo con piacere quanto proposto." },
  { label: "Declina", intent: "Declino gentilmente, ringraziando." },
  { label: "Chiedi dettagli", intent: "Chiedo i dettagli mancanti necessari per procedere." },
  { label: "Grazie", intent: "Ringrazio brevemente, confermando di aver ricevuto." },
];

export const rewriteModes = [
  { label: "Più professionale", how: "con tono più professionale e formale" },
  { label: "Più cordiale", how: "con tono più caldo e cordiale" },
  { label: "Più breve", how: "rendendolo più breve e diretto, al massimo la metà della lunghezza" },
  { label: "Correggi", how: "correggendo solo grammatica, ortografia e punteggiatura" },
  { label: "In inglese", how: "traducendolo in inglese, con tono naturale" },
];

export function providerLabel(s: Settings | undefined): string {
  if (s?.aiProvider === "openai") return "GPT";
  if (s?.aiProvider === "local") return "AI locale";
  return "Claude";
}

export function senderLabel(m: FullMessage) {
  return displayName(m.from);
}
