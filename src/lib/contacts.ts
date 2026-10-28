// Rubrica "imparata" per ogni account: chi ti scrive e a chi scrivi, con un punteggio
// che premia frequenza e recenza. Risponde all'istante, anche per gli account IMAP.
import type { Addr } from "./api";

export interface Suggestion {
  name: string;
  email: string;
  /** recenti | rubrica | directory | google */
  source: string;
}

interface Entry {
  name: string;
  email: string;
  score: number;
  last: number;
}

const MAX = 2000;
const DAY = 86_400_000;
// Indirizzi automatici: non sono persone a cui si risponde
const ROBOT = /(no-?reply|do-?not-?reply|notifications?|mailer-daemon|postmaster|bounce|newsletter|noreply)/i;

const books = new Map<string, Map<string, Entry>>();
const timers = new Map<string, ReturnType<typeof setTimeout>>();

function book(accountId: string): Map<string, Entry> {
  let b = books.get(accountId);
  if (!b) {
    b = new Map();
    try {
      for (const e of JSON.parse(localStorage.getItem(`cobalto:contacts:${accountId}`) ?? "[]") as Entry[]) {
        b.set(e.email.toLowerCase(), e);
      }
    } catch {}
    books.set(accountId, b);
  }
  return b;
}

function save(accountId: string) {
  clearTimeout(timers.get(accountId));
  timers.set(
    accountId,
    setTimeout(() => {
      const entries = [...book(accountId).values()].sort((a, b) => rank(b) - rank(a)).slice(0, MAX);
      try {
        localStorage.setItem(`cobalto:contacts:${accountId}`, JSON.stringify(entries));
      } catch {}
    }, 1500),
  );
}

/** Punteggio che decade: chi non senti da mesi scende, chi senti spesso resta in alto. */
function rank(e: Entry, now = Date.now()) {
  return e.score / (1 + (now - e.last) / (30 * DAY));
}

/** Parse di "Nome <email>" o "email" come scritti nei campi del composer. */
export function parseAddress(s: string): Addr {
  const m = s.match(/^\s*"?([^"<]*?)"?\s*<([^>]+)>\s*$/);
  if (m) return { name: m[1].trim(), email: m[2].trim() };
  return { name: "", email: s.trim() };
}

export function learn(accountId: string, addrs: Addr[], weight: number, myEmail = "", when = Date.now()) {
  const b = book(accountId);
  const me = myEmail.toLowerCase();
  let changed = false;
  for (const a of addrs) {
    const email = a.email.trim();
    const key = email.toLowerCase();
    if (!email.includes("@") || key === me || ROBOT.test(email)) continue;
    const prev = b.get(key);
    const name = (a.name ?? "").trim();
    b.set(key, {
      email,
      name: name.length > (prev?.name.length ?? 0) ? name : (prev?.name ?? name),
      score: (prev?.score ?? 0) + weight,
      last: Math.max(prev?.last ?? 0, when),
    });
    changed = true;
  }
  if (changed) save(accountId);
}

/** Suggerimenti locali: prefisso di nome, cognome o indirizzo, ordinati per punteggio. */
export function suggestLocal(accountId: string, query: string, exclude: Set<string>, limit = 6): Suggestion[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const now = Date.now();
  return [...book(accountId).values()]
    .filter((e) => !exclude.has(e.email.toLowerCase()))
    .map((e) => {
      const words = [e.email.toLowerCase(), ...e.name.toLowerCase().split(/\s+/)];
      const prefix = words.some((w) => w.startsWith(q));
      const inside = !prefix && (e.email.toLowerCase().includes(q) || e.name.toLowerCase().includes(q));
      return { e, s: prefix ? rank(e, now) + 1000 : inside ? rank(e, now) : -1 };
    })
    .filter((x) => x.s >= 0)
    .sort((a, b) => b.s - a.s)
    .slice(0, limit)
    .map(({ e }) => ({ name: e.name, email: e.email, source: "recenti" }));
}
