// Suoni delle notifiche: "Cobalto" è generato con WebAudio (funziona su ogni sistema),
// gli altri sono i suoni di sistema di macOS passati alla notifica.
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";

export type SoundKind = "mail" | "meeting";

const KEY = "cobalto:sounds";
const isMac = navigator.userAgent.includes("Mac");

const MAC_SOUNDS = ["Glass", "Ping", "Pop", "Tink", "Purr", "Hero", "Submarine", "Funk", "Bottle", "Blow", "Frog", "Morse", "Sosumi", "Basso"];

export const SOUND_OPTIONS: { id: string; label: string }[] = [
  { id: "cobalto", label: "Cobalto" },
  ...(isMac ? MAC_SOUNDS.map((s) => ({ id: s, label: s })) : []),
  { id: "none", label: "Nessun suono" },
];

const DEFAULTS: Record<SoundKind, string> = { mail: "cobalto", meeting: "cobalto" };

export function getSound(kind: SoundKind): string {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}")[kind];
    if (SOUND_OPTIONS.some((o) => o.id === saved)) return saved;
  } catch {}
  return DEFAULTS[kind];
}

export function setSound(kind: SoundKind, id: string) {
  try {
    const all = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    all[kind] = id;
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {}
}

let ctx: AudioContext | null = null;

/** Una nota con attacco morbido e coda naturale, come un colpo su un bicchiere. */
function note(ac: AudioContext, freq: number, at: number, dur: number, gain: number) {
  const osc = ac.createOscillator();
  const overtone = ac.createOscillator();
  const g = ac.createGain();
  osc.type = "sine";
  overtone.type = "sine";
  osc.frequency.value = freq;
  overtone.frequency.value = freq * 2.01;
  const og = ac.createGain();
  og.gain.value = 0.18;
  overtone.connect(og).connect(g);
  osc.connect(g).connect(ac.destination);
  g.gain.setValueAtTime(0.0001, at);
  g.gain.exponentialRampToValueAtTime(gain, at + 0.012);
  g.gain.exponentialRampToValueAtTime(0.0001, at + dur);
  osc.start(at);
  overtone.start(at);
  osc.stop(at + dur + 0.05);
  overtone.stop(at + dur + 0.05);
}

/** Il suono "Cobalto": due note per la posta, un arpeggio ripetuto per le riunioni. */
export function playCobalto(kind: SoundKind) {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") void ctx.resume();
    const t = ctx.currentTime + 0.02;
    if (kind === "mail") {
      note(ctx, 880, t, 0.55, 0.16); // La5
      note(ctx, 1318.5, t + 0.11, 0.8, 0.13); // Mi6
    } else {
      for (const offset of [0, 0.95]) {
        note(ctx, 659.3, t + offset, 0.6, 0.15); // Mi5
        note(ctx, 830.6, t + offset + 0.13, 0.6, 0.15); // Sol#5
        note(ctx, 987.8, t + offset + 0.26, 1.0, 0.14); // Si5
      }
    }
  } catch {
    // Audio non disponibile: la notifica arriva comunque
  }
}

let allowed: boolean | null = null;

async function canNotify() {
  if (allowed === null) {
    allowed = (await isPermissionGranted().catch(() => false)) || (await requestPermission().catch(() => "denied")) === "granted";
  }
  return allowed;
}

/** Notifica di sistema con il suono scelto per quel tipo di evento. */
export async function notify(kind: SoundKind, title: string, body: string, sound = getSound(kind)) {
  if (sound === "cobalto") playCobalto(kind);
  if (!(await canNotify())) return;
  sendNotification({
    title,
    body,
    ...(sound !== "cobalto" && sound !== "none" ? { sound } : {}),
  });
}

/** Anteprima dalle impostazioni. */
export function preview(kind: SoundKind, id: string) {
  if (id === "cobalto") playCobalto(kind);
  else if (id !== "none") notify(kind, "Prova suono", kind === "mail" ? "Così suona la nuova posta" : "Così suona l'avviso riunione", id);
}
