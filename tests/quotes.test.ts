import { test } from "node:test";
import assert from "node:assert/strict";
import { trimQuotedText, isReplySubject } from "../src/lib/quotes.ts";

test("risposta italiana: si taglia da 'ha scritto:'", () => {
  const r = trimQuotedText("Va bene, a mercoledì.\n\nIl giorno 27 ott 2026, alle 10:24, Giulia ha scritto:\n> Ciao\n> come d'accordo");
  assert.equal(r.text, "Va bene, a mercoledì.");
  assert.equal(r.trimmed, true);
});

test("Outlook: Da/Inviato", () => {
  const r = trimQuotedText("Confermo.\n\nDa: Giulia Bianchi\nInviato: martedì\nA: Max\nOggetto: Preventivo");
  assert.equal(r.text, "Confermo.");
});

test("se c'è solo la citazione, non si taglia niente", () => {
  const t = "> testo citato\n> altro";
  assert.deepEqual(trimQuotedText(t), { text: t, trimmed: false });
});

test("una riga 'Da:' nel testo senza intestazione Outlook resta", () => {
  const t = "Ciao,\nDa: domani si parte\nsaluti";
  assert.equal(trimQuotedText(t).trimmed, false);
});

test("solo le risposte si accorciano", () => {
  assert.equal(isReplySubject("R: Ordine"), true);
  assert.equal(isReplySubject("I: Ordine"), false);
});
