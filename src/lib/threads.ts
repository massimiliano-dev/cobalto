import type { Envelope } from "./api";

export interface Thread {
  /** Id stabile: l'uid del messaggio più recente */
  id: number;
  /** Dal più vecchio al più recente */
  messages: Envelope[];
  latest: Envelope;
  unread: number;
  flagged: boolean;
  hasAttachments: boolean;
}

const PREFIX = /^\s*((re|r|aw|sv|antw|rif|fw|fwd|i|inoltra|tr|wg)\s*(\[\d+\])?\s*:\s*)+/i;

/** Oggetto senza "Re:", "R:", "I:", "Fwd:"… ripetuti. */
export function baseSubject(s: string): string {
  return s.replace(PREFIX, "").trim().toLowerCase();
}

const isReply = (s: string) => /^\s*(re|r|aw|sv|antw|rif)\s*(\[\d+\])?\s*:/i.test(s);

/**
 * Raggruppa i messaggi in conversazioni. Prima con gli identificativi standard
 * (Message-ID ↔ In-Reply-To/References); poi, per i client che non li inviano,
 * una risposta ("Re: …") si aggancia al messaggio con lo stesso oggetto.
 */
export function buildThreads(messages: Envelope[]): Thread[] {
  const parent = messages.map((_, i) => i);
  const find = (i: number): number => (parent[i] === i ? i : (parent[i] = find(parent[i])));
  const join = (a: number, b: number) => {
    const ra = find(a);
    const rb = find(b);
    if (ra !== rb) parent[ra] = rb;
  };

  const byId = new Map<string, number>();
  messages.forEach((m, i) => m.messageId && byId.set(m.messageId, i));
  messages.forEach((m, i) => {
    for (const p of m.parents ?? []) {
      const j = byId.get(p);
      if (j !== undefined) join(i, j);
    }
  });

  // Ripiego sull'oggetto, solo per le risposte: due "Report mensile" diversi restano separati
  const bySubject = new Map<string, number>();
  messages.forEach((m, i) => {
    const key = baseSubject(m.subject);
    if (key.length < 3) return;
    if (!isReply(m.subject) && !bySubject.has(key)) bySubject.set(key, i);
  });
  messages.forEach((m, i) => {
    if (!isReply(m.subject)) return;
    const j = bySubject.get(baseSubject(m.subject));
    if (j !== undefined) join(i, j);
    else bySubject.set(baseSubject(m.subject), i);
  });

  const groups = new Map<number, Envelope[]>();
  messages.forEach((m, i) => {
    const root = find(i);
    const g = groups.get(root);
    if (g) g.push(m);
    else groups.set(root, [m]);
  });

  return [...groups.values()]
    .map((list) => {
      list.sort((a, b) => a.date - b.date || a.uid - b.uid);
      const latest = list[list.length - 1];
      return {
        id: latest.uid,
        messages: list,
        latest,
        unread: list.filter((m) => !m.seen).length,
        flagged: list.some((m) => m.flagged),
        hasAttachments: list.some((m) => m.hasAttachments),
      };
    })
    .sort((a, b) => b.latest.date - a.latest.date || b.latest.uid - a.latest.uid);
}
