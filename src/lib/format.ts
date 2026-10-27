import type { Addr } from "./api";

const timeFmt = new Intl.DateTimeFormat("it-IT", { hour: "2-digit", minute: "2-digit" });
const dayFmt = new Intl.DateTimeFormat("it-IT", { weekday: "short" });
const shortFmt = new Intl.DateTimeFormat("it-IT", { day: "numeric", month: "short" });
const yearFmt = new Intl.DateTimeFormat("it-IT", { day: "numeric", month: "short", year: "numeric" });
const fullFmt = new Intl.DateTimeFormat("it-IT", {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

function startOfDay(d: Date) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Data compatta per la lista: ora se oggi, giorno se in settimana, altrimenti data. */
export function listDate(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  const today = startOfDay(new Date());
  const day = startOfDay(d);
  if (day === today) return timeFmt.format(d);
  if (today - day < 6 * 86400000) return dayFmt.format(d).replace(".", "");
  if (d.getFullYear() === new Date().getFullYear()) return shortFmt.format(d).replace(".", "");
  return yearFmt.format(d);
}

export function fullDate(ts: number): string {
  return ts ? fullFmt.format(new Date(ts * 1000)) : "";
}

/** Raggruppamento della lista: Oggi, Ieri, Questa settimana, mese… */
export function dateGroup(ts: number): string {
  const d = new Date(ts * 1000);
  const today = startOfDay(new Date());
  const day = startOfDay(d);
  if (day === today) return "Oggi";
  if (day === today - 86400000) return "Ieri";
  if (today - day < 7 * 86400000) return "Questa settimana";
  const m = new Intl.DateTimeFormat("it-IT", {
    month: "long",
    year: d.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
  }).format(d);
  return m.charAt(0).toUpperCase() + m.slice(1);
}

export function displayName(a: Addr | undefined): string {
  if (!a) return "";
  return a.name || a.email.split("@")[0] || a.email;
}

export function initials(a: Addr | undefined): string {
  const n = displayName(a).replace(/[^\p{L}\p{N} ]/gu, " ").trim();
  const parts = n.split(/\s+/).filter(Boolean);
  if (!parts.length) return "·";
  return (parts[0][0] + (parts.length > 1 ? parts[parts.length - 1][0] : "")).toUpperCase();
}

/** Tonalità di blu stabile per mittente: tutta la palette resta nel blu. */
export function senderHue(email: string): number {
  let h = 0;
  for (const c of email) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return 215 + (h % 50); // 215–265: dall'azzurro al blu oltremare
}

export function formatAddr(a: Addr): string {
  return a.name ? `${a.name} <${a.email}>` : a.email;
}

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Markdown minimo per le risposte dell'AI: elenchi, grassetto, paragrafi. */
export function miniMarkdown(src: string): string {
  const lines = escapeHtml(src.trim()).split("\n");
  let html = "";
  let inList = false;
  for (const raw of lines) {
    const line = raw.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
    const item = line.match(/^\s*(?:[-*•]|\d+[.)])\s+(.*)$/);
    if (item) {
      if (!inList) html += "<ul>";
      inList = true;
      html += `<li>${item[1]}</li>`;
      continue;
    }
    if (inList) {
      html += "</ul>";
      inList = false;
    }
    const heading = line.match(/^#{1,4}\s+(.*)$/);
    if (heading) html += `<h4>${heading[1]}</h4>`;
    else if (line.trim()) html += `<p>${line}</p>`;
  }
  if (inList) html += "</ul>";
  return html;
}

export function textToHtml(text: string): string {
  return escapeHtml(text)
    .split(/\n{2,}/)
    .map((p) => `<p>${p.replace(/\n/g, "<br>")}</p>`)
    .join("");
}
