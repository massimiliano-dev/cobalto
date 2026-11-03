// node --test tests/  (Node ≥ 23 esegue TypeScript direttamente)
import { test } from "node:test";
import assert from "node:assert/strict";
import { buildThreads, baseSubject } from "../src/lib/threads.ts";

const env = (uid: number, subject: string, messageId = "", parents: string[] = [], date = uid) => ({
  uid,
  subject,
  from: { name: "", email: "a@x.it" },
  to: [],
  date,
  preview: "",
  seen: uid !== 3,
  flagged: false,
  answered: false,
  hasAttachments: false,
  size: 0,
  messageId,
  parents,
});

test("le risposte si uniscono con References e In-Reply-To", () => {
  const t = buildThreads([
    env(1, "Preventivo", "a@x"),
    env(2, "Re: Preventivo", "b@x", ["a@x"]),
    env(3, "Re: Preventivo", "c@x", ["a@x", "b@x"]),
    env(4, "Altro", "d@x"),
  ]);
  assert.equal(t.length, 2);
  assert.deepEqual(t[0].messages.map((m) => m.uid), [4]);
  assert.deepEqual(t[1].messages.map((m) => m.uid), [1, 2, 3]);
  assert.equal(t[1].unread, 1);
  assert.equal(t[1].latest.uid, 3);
});

test("senza identificativi, una risposta si aggancia allo stesso oggetto", () => {
  const t = buildThreads([env(1, "Riunione giovedì"), env(2, "R: Riunione giovedì")]);
  assert.equal(t.length, 1);
});

test("due messaggi con lo stesso oggetto ma non risposte restano separati", () => {
  const t = buildThreads([env(1, "Report mensile"), env(2, "Report mensile")]);
  assert.equal(t.length, 2);
});

test("prefissi ripetuti e in altre lingue", () => {
  assert.equal(baseSubject("Re: R: AW: Fwd: Ordine 12"), "ordine 12");
});
