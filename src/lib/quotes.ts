// Nelle risposte, lo storico citato ("Il giorno… ha scritto:" con tutto sotto) si nasconde:
// resta a un clic di distanza. Gli inoltri non passano di qui.

/** Righe da cui inizia la citazione nelle email in solo testo. */
const TEXT_MARKERS = [
  /^>/,
  /^(Il|In data) .{4,120} ha scritto:\s*$/i,
  /^On .{4,160} wrote:\s*$/i,
  /^-{2,}\s*(Messaggio originale|Original Message)\s*-{2,}/i,
];

export function trimQuotedText(text: string): { text: string; trimmed: boolean } {
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    // Outlook: "Da: …" seguito a breve da "Inviato:" / "From:" seguito da "Sent:"
    const outlook =
      /^(Da|From):/i.test(line) && lines.slice(i + 1, i + 4).some((l) => /^(Inviato|Data|Sent|Date):/i.test(l.trim()));
    if (outlook || TEXT_MARKERS.some((re) => re.test(line))) {
      const kept = lines.slice(0, i).join("\n").trimEnd();
      // Se sopra non resta niente, la citazione È il contenuto: non si taglia
      if (kept.replace(/\s/g, "").length < 2) break;
      return { text: kept, trimmed: true };
    }
  }
  return { text, trimmed: false };
}

/** Dove iniziano le citazioni dei client più diffusi (Apple Mail, Gmail, Outlook, Thunderbird). */
const HTML_MARKERS = [
  "#appendonsend",
  "#divRplyFwdMsg",
  "div[id^='divRplyFwdMsg']",
  ".gmail_quote",
  ".moz-cite-prefix",
  "blockquote[type='cite']",
  "div[style*='border-top:solid #E1E1E1']",
  "div[style*='border-top: solid #E1E1E1']",
  "div[style*='border-top:solid #B5C4DF']",
];

export function trimQuotedHtml(html: string): { html: string; trimmed: boolean } {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  const marker = doc.body.querySelector(HTML_MARKERS.join(","));
  if (!marker) return { html, trimmed: false };
  // Via il marcatore e tutto ciò che viene dopo, risalendo fino al body
  let node: Element | null = marker;
  while (node && node !== doc.body) {
    while (node.nextSibling) node.nextSibling.remove();
    node = node.parentElement;
  }
  marker.remove();
  if ((doc.body.textContent ?? "").replace(/\s/g, "").length < 2 && !doc.body.querySelector("img")) {
    return { html, trimmed: false };
  }
  return { html: doc.body.innerHTML, trimmed: true };
}

/** Le risposte si accorciano; inoltri e messaggi nuovi restano interi. */
export function isReplySubject(subject: string): boolean {
  return /^\s*(re|r|aw|sv|antw|rif)\s*(\[\d+\])?\s*:/i.test(subject);
}
