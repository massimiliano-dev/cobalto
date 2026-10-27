import { api, errorText, type CalEvent, type CalendarResult } from "./api";
import { notify } from "./sounds";

const HIDDEN_KEY = "cobalto:cal-hidden";
const DAY = 86_400_000;

/** Lunedì 00:00 locale della settimana che contiene `d`. */
export function weekStart(d: Date): Date {
  const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const dow = (x.getDay() + 6) % 7;
  x.setDate(x.getDate() - dow);
  return x;
}

export function addDays(d: Date, n: number): Date {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
}

export function sameDay(a: Date, b: Date) {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** "AAAA-MM-GG" → mezzanotte locale (per gli eventi di tutto il giorno). */
export function localDate(s: string): Date {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/** Inizio e fine (esclusa) in ms locali, anche per gli eventi di tutto il giorno. */
export function span(e: CalEvent): [number, number] {
  if (e.allDay && e.startDate) {
    const s = localDate(e.startDate).getTime();
    const end = e.endDate ? localDate(e.endDate).getTime() : s + DAY;
    return [s, Math.max(end, s + DAY)];
  }
  return [e.start, Math.max(e.end, e.start + 15 * 60_000)];
}

/** Tonalità di blu per account: restiamo sempre nel blu, cambiando sfumatura. */
const HUES = [262, 232, 250, 218, 244, 226];
export function accountHue(index: number) {
  return HUES[index % HUES.length];
}

class CalendarState {
  mode = $state<"week" | "agenda">("week");
  anchor = $state(weekStart(new Date()));
  events = $state<CalEvent[]>([]);
  status = $state<CalendarResult["accounts"]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  hidden = $state<string[]>(readHidden());
  selected = $state<CalEvent | null>(null);
  /** Prossimi giorni, sempre aggiornati: avvisi riunione e contesto dell'assistente */
  upcoming = $state<CalEvent[]>([]);
  now = $state(Date.now());

  visible = $derived(this.events.filter((e) => !this.hidden.includes(e.accountId) && !e.cancelled));
  range = $derived.by(() => {
    const start = this.anchor.getTime();
    const days = this.mode === "week" ? 7 : 14;
    return { start, end: addDays(this.anchor, days).getTime(), days };
  });

  private token = 0;
  private notified = new Set<string>();
  private started = false;

  start() {
    if (this.started) return;
    this.started = true;
    this.refreshUpcoming();
    setInterval(() => this.refreshUpcoming(), 5 * 60_000);
    setInterval(() => {
      this.now = Date.now();
      this.remind();
    }, 30_000);
  }

  async load() {
    const { start, end } = this.range;
    const token = ++this.token;
    this.loading = true;
    this.error = null;
    try {
      const r = await api.calendar(start, end);
      if (token !== this.token) return;
      this.events = r.events;
      this.status = r.accounts;
    } catch (e) {
      if (token === this.token) this.error = errorText(e);
    } finally {
      if (token === this.token) this.loading = false;
    }
  }

  async refreshUpcoming() {
    try {
      const now = Date.now();
      const r = await api.calendar(now - 12 * 3600_000, now + 8 * DAY);
      this.upcoming = r.events.filter((e) => !e.cancelled);
      if (!this.status.length) this.status = r.accounts;
      this.remind();
    } catch {
      // Il calendario è un di più: se manca, la posta funziona lo stesso
    }
  }

  /** La riunione in corso o che inizia entro 15 minuti (quella con il link per prima). */
  next = $derived.by(() => {
    const soon = this.upcoming
      .filter((e) => !e.allDay && !this.hidden.includes(e.accountId))
      .filter((e) => e.end > this.now && e.start - this.now < 15 * 60_000)
      .sort((a, b) => Number(!!b.joinUrl) - Number(!!a.joinUrl) || a.start - b.start);
    return soon[0] ?? null;
  });

  private async remind() {
    const now = Date.now();
    for (const e of this.upcoming) {
      if (e.allDay || this.notified.has(e.id) || this.hidden.includes(e.accountId)) continue;
      const lead = e.start - now;
      if (lead > 0 && lead <= 5 * 60_000) {
        this.notified.add(e.id);
        const mins = Math.max(1, Math.round(lead / 60_000));
        // Le riunioni suonano anche con Cobalto in primo piano: sono a tempo
        await notify(
          "meeting",
          `Tra ${mins} min · ${e.title}`,
          e.joinUrl ? "Apri Cobalto per partecipare alla chiamata" : e.location || "In calendario",
        );
      }
    }
  }

  go(delta: number) {
    this.anchor = addDays(this.anchor, delta * (this.mode === "week" ? 7 : 14));
    this.load();
  }

  private home(): Date {
    return this.mode === "week" ? weekStart(new Date()) : new Date(new Date().setHours(0, 0, 0, 0));
  }

  today() {
    this.anchor = this.home();
    this.load();
  }

  goTo(d: Date) {
    this.anchor = this.mode === "week" ? weekStart(d) : new Date(d.getFullYear(), d.getMonth(), d.getDate());
    this.load();
  }

  setMode(m: "week" | "agenda") {
    this.mode = m;
    this.anchor = this.home();
    this.load();
  }

  toggle(accountId: string) {
    this.hidden = this.hidden.includes(accountId) ? this.hidden.filter((x) => x !== accountId) : [...this.hidden, accountId];
    try {
      localStorage.setItem(HIDDEN_KEY, JSON.stringify(this.hidden));
    } catch {}
  }
}

function readHidden(): string[] {
  try {
    return JSON.parse(localStorage.getItem(HIDDEN_KEY) ?? "[]");
  } catch {
    return [];
  }
}

export const cal = new CalendarState();

export const joinLabel: Record<string, string> = {
  teams: "Partecipa su Teams",
  meet: "Partecipa su Meet",
  zoom: "Partecipa su Zoom",
  webex: "Partecipa su Webex",
  altro: "Partecipa alla chiamata",
};
