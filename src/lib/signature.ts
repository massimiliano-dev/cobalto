import DOMPurify from "dompurify";
import { textToHtml } from "./format";

const ALLOWED = {
  ALLOWED_TAGS: ["a", "img", "b", "strong", "i", "em", "u", "br", "p", "div", "span", "small", "font", "table", "tbody", "tr", "td", "hr"],
  ALLOWED_ATTR: ["href", "src", "alt", "title", "width", "height", "style", "target", "color", "align", "valign", "cellpadding", "cellspacing", "border"],
  ALLOWED_URI_REGEXP: /^(?:https?:|mailto:|tel:|data:image\/(?:png|jpe?g|gif|webp);base64,)/i,
};

/** Le firme vecchie erano testo semplice: le trattiamo come tali se non contengono tag. */
export function isHtml(s: string): boolean {
  return /<[a-z][\s\S]*>/i.test(s);
}

export function sanitizeSignature(html: string): string {
  const clean = DOMPurify.sanitize(html, ALLOWED) as string;
  // I link della firma si aprono sempre fuori dal client di chi riceve
  return clean.replace(/<a\s/gi, '<a target="_blank" rel="noopener" ');
}

export function signatureHtml(sig: string): string {
  if (!sig.trim()) return "";
  return isHtml(sig) ? sanitizeSignature(sig) : textToHtml(sig);
}

/** Versione testuale per la parte text/plain: i link diventano "testo <url>". */
export function signatureText(sig: string): string {
  if (!isHtml(sig)) return sig.trim();
  const doc = new DOMParser().parseFromString(sanitizeSignature(sig), "text/html");
  doc.querySelectorAll("a").forEach((a) => {
    const href = a.getAttribute("href") ?? "";
    const label = a.textContent?.trim() ?? "";
    const url = href.replace(/^mailto:/, "");
    a.replaceWith(label && label !== url && href ? `${label} <${url}>` : label || url);
  });
  doc.querySelectorAll("img").forEach((img) => img.replaceWith(img.getAttribute("alt") ?? ""));
  doc.querySelectorAll("br").forEach((br) => br.replaceWith("\n"));
  doc.querySelectorAll("p,div,tr").forEach((el) => el.append("\n"));
  return (doc.body.textContent ?? "")
    .split("\n")
    .map((l) => l.trim())
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

/** Ridimensiona un'immagine caricata: una firma non deve pesare come una foto. */
export async function imageToDataUri(file: File, maxWidth = 480): Promise<string> {
  const original = await new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
  if (file.type === "image/gif" || file.type === "image/svg+xml") return original;
  const img = new Image();
  img.src = original;
  await img.decode();
  if (img.naturalWidth <= maxWidth && file.size < 150_000) return original;
  const scale = Math.min(1, maxWidth / img.naturalWidth);
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(img.naturalWidth * scale);
  canvas.height = Math.round(img.naturalHeight * scale);
  canvas.getContext("2d")!.drawImage(img, 0, 0, canvas.width, canvas.height);
  return file.type === "image/png" ? canvas.toDataURL("image/png") : canvas.toDataURL("image/jpeg", 0.88);
}
