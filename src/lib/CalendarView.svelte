<script lang="ts">
  import { onMount, tick } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { mail } from "./state.svelte";
  import { cal, addDays, sameDay, span, accountHue, joinLabel } from "./calendar.svelte";
  import Icon from "./Icon.svelte";
  import { sheet } from "./motion";
  import type { CalEvent } from "./api";

  const H = 54; // altezza di un'ora in px
  const DAY = 86_400_000;
  const timeFmt = new Intl.DateTimeFormat("it-IT", { hour: "2-digit", minute: "2-digit" });
  const dowFmt = new Intl.DateTimeFormat("it-IT", { weekday: "short" });
  const longDay = new Intl.DateTimeFormat("it-IT", { weekday: "long", day: "numeric", month: "long" });

  let grid: HTMLElement | undefined = $state();

  let days = $derived(Array.from({ length: cal.range.days }, (_, i) => addDays(cal.anchor, i)));
  let weekDays = $derived(days.slice(0, 7));
  let today = $derived(new Date(cal.now));

  let title = $derived.by(() => {
    const a = cal.anchor;
    const b = addDays(a, cal.range.days - 1);
    const m = (d: Date, y = false) =>
      new Intl.DateTimeFormat("it-IT", { month: "long", year: y ? "numeric" : undefined }).format(d);
    const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
    if (a.getMonth() === b.getMonth()) return cap(m(a, true));
    return `${cap(m(a))} – ${m(b, true)}`;
  });

  function hueOf(accountId: string) {
    return accountHue(Math.max(0, mail.accounts.findIndex((a) => a.id === accountId)));
  }
  function accountName(accountId: string) {
    return mail.accounts.find((a) => a.id === accountId)?.email ?? "";
  }

  interface Placed {
    ev: CalEvent;
    a: number;
    b: number;
    col: number;
    cols: number;
  }

  /** Disposizione degli eventi di un giorno: quelli sovrapposti si affiancano in colonne. */
  function layout(day: Date): Placed[] {
    const s = day.getTime();
    const e = s + DAY;
    const items: Placed[] = cal.visible
      .filter((ev) => !ev.allDay)
      .map((ev) => {
        const [a, b] = span(ev);
        return { ev, a: Math.max(a, s), b: Math.min(b, e), col: 0, cols: 1 };
      })
      .filter((x) => x.b > x.a)
      .sort((x, y) => x.a - y.a || y.b - y.a - (x.b - x.a));
    let cols: number[] = [];
    let cluster: Placed[] = [];
    let clusterEnd = 0;
    const close = () => {
      cluster.forEach((p) => (p.cols = cols.length));
      cols = [];
      cluster = [];
    };
    for (const it of items) {
      if (cluster.length && it.a >= clusterEnd) close();
      let c = cols.findIndex((end) => end <= it.a);
      if (c < 0) {
        c = cols.length;
        cols.push(it.b);
      } else cols[c] = it.b;
      it.col = c;
      cluster.push(it);
      clusterEnd = Math.max(clusterEnd, it.b);
    }
    close();
    return items;
  }

  function allDayOn(day: Date) {
    const s = day.getTime();
    return cal.visible.filter((ev) => {
      if (!ev.allDay) return false;
      const [a, b] = span(ev);
      return a < s + DAY && b > s;
    });
  }

  function dayEvents(day: Date) {
    const s = day.getTime();
    return cal.visible
      .filter((ev) => {
        const [a, b] = span(ev);
        return a < s + DAY && b > s;
      })
      .sort((x, y) => Number(y.allDay) - Number(x.allDay) || span(x)[0] - span(y)[0]);
  }

  function when(ev: CalEvent) {
    if (ev.allDay) return "Tutto il giorno";
    return `${timeFmt.format(ev.start)} – ${timeFmt.format(ev.end)}`;
  }

  function isLive(ev: CalEvent) {
    const [a, b] = span(ev);
    return !ev.allDay && a <= cal.now && b > cal.now;
  }

  let nowTop = $derived(((today.getHours() * 60 + today.getMinutes()) / 60) * H);

  async function scrollToMorning() {
    await tick();
    if (!grid) return;
    const showsToday = weekDays.some((d) => sameDay(d, today));
    const target = showsToday ? Math.max(0, nowTop - H * 2) : 7.5 * H;
    grid.scrollTo({ top: target });
  }

  onMount(() => {
    cal.start();
    cal.load();
    scrollToMorning();
  });

  // Solo la prima lettera: in italiano i mesi restano minuscoli
  const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  function join(url: string) {
    openUrl(url);
  }
</script>

<section class="calendar">
  <header data-tauri-drag-region>
    <div class="title-row" data-tauri-drag-region>
      <h1 data-tauri-drag-region>{title}</h1>
      <div class="nav">
        <button class="btn btn-line" onclick={() => (cal.today(), scrollToMorning())}>Oggi</button>
        <button class="icon-btn" title="Precedente" onclick={() => cal.go(-1)}><Icon name="left" size={16} /></button>
        <button class="icon-btn" title="Successivo" onclick={() => cal.go(1)}><Icon name="chevron" size={16} /></button>
        <button class="icon-btn" class:spin={cal.loading} title="Aggiorna" onclick={() => cal.load()}><Icon name="refresh" size={15} /></button>
      </div>
      <span class="spacer"></span>
      <div class="seg" role="radiogroup" aria-label="Vista">
        <button role="radio" aria-checked={cal.mode === "week"} class:on={cal.mode === "week"} onclick={() => (cal.setMode("week"), scrollToMorning())}>Settimana</button>
        <button role="radio" aria-checked={cal.mode === "agenda"} class:on={cal.mode === "agenda"} onclick={() => cal.setMode("agenda")}>Agenda</button>
      </div>
    </div>
    {#each cal.status.filter((s) => s.status === "error") as s (s.accountId)}
      <p class="warn"><strong>{accountName(s.accountId)}</strong> · {s.message}</p>
    {/each}
    {#if cal.error}<p class="warn">{cal.error}</p>{/if}
  </header>

  {#if cal.mode === "week"}
    <div class="week">
      <div class="head">
        <span class="gutter"></span>
        {#each weekDays as d (d.getTime())}
          <button class="day-head" class:today={sameDay(d, today)} onclick={() => (cal.mode = "agenda", cal.goTo(d))}>
            <span class="dow">{dowFmt.format(d).replace(".", "")}</span>
            <span class="num">{d.getDate()}</span>
          </button>
        {/each}
      </div>
      {#if weekDays.some((d) => allDayOn(d).length)}
        <div class="allday">
          <span class="gutter label">tutto il giorno</span>
          {#each weekDays as d (d.getTime())}
            <div class="allday-col">
              {#each allDayOn(d) as ev (ev.id + d.getTime())}
                <button class="chip" style:--h={hueOf(ev.accountId)} class:free={ev.showAs === "free"} onclick={() => (cal.selected = ev)}>{ev.title}</button>
              {/each}
            </div>
          {/each}
        </div>
      {/if}
      <div class="grid-scroll" bind:this={grid}>
        <div class="grid" style:height="{24 * H}px" style:--hh="{H}px">
          <div class="hours">
            {#each Array(24) as _, h (h)}
              {#if h > 0}<span style:top="{h * H}px">{String(h).padStart(2, "0")}:00</span>{/if}
            {/each}
          </div>
          {#each weekDays as d, di (d.getTime())}
            <div class="col" class:today={sameDay(d, today)} class:weekend={di >= 5}>
              {#each layout(d) as p (p.ev.id)}
                {@const short = (p.b - p.a) / 3_600_000 < 0.75}
                <button
                  class="event"
                  class:short
                  class:live={isLive(p.ev)}
                  class:tentative={p.ev.showAs === "tentative"}
                  class:free={p.ev.showAs === "free"}
                  class:sel={cal.selected?.id === p.ev.id}
                  style:--h={hueOf(p.ev.accountId)}
                  style:top="{((p.a - d.getTime()) / 3_600_000) * H + 1}px"
                  style:height="{Math.max(((p.b - p.a) / 3_600_000) * H - 3, 20)}px"
                  style:left="calc({(p.col / p.cols) * 100}% + 2px)"
                  style:width="calc({100 / p.cols}% - 5px)"
                  onclick={() => (cal.selected = p.ev)}
                >
                  <span class="ev-title">
                    {#if p.ev.joinUrl}<Icon name="video" size={12} />{/if}
                    {p.ev.title}
                  </span>
                  {#if !short}<span class="ev-time">{timeFmt.format(p.ev.start)}{p.ev.location ? ` · ${p.ev.location}` : ""}</span>{/if}
                </button>
              {/each}
              {#if sameDay(d, today)}
                <div class="now" style:top="{nowTop}px" aria-hidden="true"></div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    </div>
  {:else}
    <div class="agenda">
      <div class="agenda-inner">
      {#each days as d (d.getTime())}
        {@const list = dayEvents(d)}
        {#if list.length || sameDay(d, today)}
          <section class="a-day" class:today={sameDay(d, today)}>
            <h2>{sameDay(d, today) ? "Oggi" : sameDay(d, addDays(today, 1)) ? "Domani" : capitalize(longDay.format(d))}</h2>
            {#if !list.length}
              <p class="a-empty">Niente in calendario.</p>
            {/if}
            {#each list as ev (ev.id)}
              <div class="a-row" class:live={isLive(ev)} style:--h={hueOf(ev.accountId)}>
                <button class="a-main" onclick={() => (cal.selected = ev)}>
                  <span class="a-time">{ev.allDay ? "Tutto il giorno" : timeFmt.format(ev.start)}</span>
                  <span class="a-dot"></span>
                  <span class="a-text">
                    <strong>{ev.title}</strong>
                    {#if ev.location || ev.organizer}<small>{[ev.location, ev.organizer].filter(Boolean).join(" · ")}</small>{/if}
                  </span>
                </button>
                {#if ev.joinUrl}
                  <button class="btn {isLive(ev) ? 'btn-primary' : 'btn-line'} join" onclick={() => join(ev.joinUrl!)}>
                    <Icon name="video" size={14} /> Partecipa
                  </button>
                {/if}
              </div>
            {/each}
          </section>
        {/if}
      {/each}
      {#if !cal.loading && !cal.visible.length}
        <p class="a-empty big">Nessun appuntamento nei prossimi 14 giorni.</p>
      {/if}
      </div>
    </div>
  {/if}

  {#if cal.selected}
    {@const ev = cal.selected}
    <aside class="detail" style:--h={hueOf(ev.accountId)} out:sheet={{ x: 24, duration: 160 }}>
      <div class="d-head">
        <span class="d-swatch"></span>
        <span class="d-account">{accountName(ev.accountId)}</span>
        <button class="icon-btn" aria-label="Chiudi" onclick={() => (cal.selected = null)}><Icon name="x" size={16} /></button>
      </div>
      <h2 class="d-title">{ev.title}</h2>
      <p class="d-when">
        <Icon name="clock" size={15} />
        <span>
          {ev.allDay && ev.startDate ? longDay.format(span(ev)[0]) : longDay.format(ev.start)}<br />
          {when(ev)}
          {#if isLive(ev)}<em class="live-tag">in corso</em>{/if}
        </span>
      </p>
      {#if ev.joinUrl}
        <button class="btn btn-primary d-join" onclick={() => join(ev.joinUrl!)}>
          <Icon name="video" size={16} /> {joinLabel[ev.joinKind ?? "altro"]}
        </button>
      {/if}
      {#if ev.location}
        <p class="d-row"><Icon name="pin" size={15} /><span>{ev.location}</span></p>
      {/if}
      {#if ev.organizer || ev.attendees.length}
        <p class="d-row">
          <Icon name="users" size={15} />
          <span>
            {#if ev.organizer}<strong>{ev.organizer}</strong> organizza{/if}
            {#if ev.attendees.length}
              <br />{ev.attendees.slice(0, 8).join(", ")}{ev.attendees.length > 8 ? ` e altri ${ev.attendees.length - 8}` : ""}
            {/if}
          </span>
        </p>
      {/if}
      {#if ev.preview}<p class="d-preview">{ev.preview}</p>{/if}
      {#if ev.webLink}
        <button class="d-link" onclick={() => openUrl(ev.webLink!)}>
          Apri in {mail.accounts.find((a) => a.id === ev.accountId)?.auth === "oauth-google" ? "Google Calendar" : "Outlook"}
          <Icon name="external" size={13} />
        </button>
      {/if}
    </aside>
  {/if}
</section>

<style>
  .calendar {
    position: relative;
    grid-column: 2 / -1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--paper);
    animation: settle var(--dur) var(--ease-ink);
  }

  header {
    padding: 40px 24px 10px;
    border-bottom: 1px solid var(--rule);
  }
  :global(.shell:not(.mac)) header {
    padding-top: 16px;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    min-width: 7ch;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.95rem;
    line-height: 1.1;
    letter-spacing: -0.01em;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .nav .btn {
    margin-right: 6px;
  }
  .spin :global(svg) {
    animation: rot 0.9s linear infinite;
  }
  @keyframes rot {
    to {
      transform: rotate(360deg);
    }
  }
  .spacer {
    flex: 1;
  }
  .seg {
    display: flex;
    padding: 3px;
    border-radius: var(--r-md);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .seg button {
    height: 28px;
    padding: 0 12px;
    border-radius: 7px;
    font-size: var(--t-sm);
    font-weight: 550;
    color: var(--ink-2);
    transition: background var(--dur-fast), color var(--dur-fast), box-shadow var(--dur-fast);
  }
  .seg button.on {
    background: var(--paper);
    color: var(--cobalt);
    box-shadow: 0 1px 2px oklch(0.4 0.12 262 / 0.18), inset 0 0 0 1px var(--rule);
  }
  .warn {
    margin: 10px 0 0;
    padding: 8px 12px;
    border-radius: var(--r-md);
    background: var(--cobalt-mist);
    font-size: var(--t-sm);
    color: var(--ink-2);
    line-height: 1.4;
  }

  /* ---------- Settimana ---------- */
  .week {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .head,
  .allday {
    display: grid;
    grid-template-columns: 56px repeat(7, minmax(0, 1fr));
    padding-right: 10px;
  }
  .head {
    border-bottom: 1px solid var(--rule-soft);
  }
  .day-head {
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 6px;
    padding: 10px 0 8px;
    color: var(--ink-3);
    border-radius: var(--r-sm);
    transition: background var(--dur-fast);
  }
  .day-head:hover {
    background: var(--cobalt-mist);
  }
  .dow {
    font-size: var(--t-xs);
    font-weight: 600;
    text-transform: capitalize;
  }
  .num {
    font-family: var(--font-serif);
    font-size: 1.35rem;
    line-height: 1;
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
  }
  .day-head.today .dow {
    color: var(--cobalt);
  }
  .day-head.today .num {
    display: inline-grid;
    place-items: center;
    min-width: 1.6em;
    height: 1.6em;
    border-radius: 50%;
    background: var(--cobalt);
    color: var(--on-deep);
    font-size: 1.05rem;
  }
  .allday {
    border-bottom: 1px solid var(--rule-soft);
    padding-block: 4px;
  }
  .gutter.label {
    align-self: center;
    font-size: 0.66rem;
    color: var(--ink-3);
    text-align: right;
    padding-right: 8px;
    line-height: 1.1;
  }
  .allday-col {
    display: grid;
    gap: 2px;
    padding: 0 2px;
  }
  .chip {
    height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
    font-size: var(--t-xs);
    font-weight: 600;
    color: oklch(0.3 0.13 var(--h));
    background: oklch(0.92 0.05 var(--h));
  }
  .chip.free {
    background: transparent;
    box-shadow: inset 0 0 0 1px oklch(0.8 0.07 var(--h));
  }

  .grid-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-right: 10px;
  }
  .grid {
    position: relative;
    display: grid;
    grid-template-columns: 56px repeat(7, minmax(0, 1fr));
    background-image: linear-gradient(var(--rule-soft) 1px, transparent 1px);
    background-size: 100% var(--hh);
  }
  .hours {
    position: relative;
  }
  .hours span {
    position: absolute;
    right: 10px;
    transform: translateY(-50%);
    font-size: 0.68rem;
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
    background: var(--paper);
    padding: 0 2px;
  }
  .col {
    position: relative;
    border-left: 1px solid var(--rule-soft);
  }
  .col.weekend {
    background: oklch(0.977 0.011 255 / 0.6);
  }
  .col.today {
    background: oklch(0.965 0.022 258 / 0.55);
  }

  .event {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 4px 7px;
    border-radius: 7px;
    overflow: hidden;
    text-align: left;
    color: oklch(0.28 0.13 var(--h));
    background: oklch(0.93 0.045 var(--h));
    box-shadow: inset 0 0 0 1px oklch(0.86 0.07 var(--h) / 0.9);
    transition: box-shadow var(--dur-fast), transform var(--dur) var(--ease-ink), background var(--dur-fast);
    animation: settle var(--dur) var(--ease-ink) backwards;
  }
  .event:hover {
    z-index: 2;
    background: oklch(0.9 0.06 var(--h));
  }
  .event.sel {
    z-index: 3;
    box-shadow: inset 0 0 0 1.5px oklch(0.5 0.2 var(--h)), 0 8px 18px -10px oklch(0.35 0.15 var(--h) / 0.6);
  }
  .event.live {
    background: oklch(0.52 0.19 var(--h));
    color: var(--on-deep);
    box-shadow: 0 6px 16px -8px oklch(0.35 0.18 var(--h) / 0.7);
  }
  .event.tentative {
    background: repeating-linear-gradient(-45deg, oklch(0.95 0.035 var(--h)) 0 6px, oklch(0.92 0.05 var(--h)) 6px 12px);
  }
  .event.free {
    background: var(--paper);
  }
  .event.short {
    flex-direction: row;
    align-items: center;
    padding-block: 2px;
  }
  .ev-title {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    font-weight: 650;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ev-title :global(svg) {
    flex: none;
  }
  .ev-time {
    font-size: 0.68rem;
    opacity: 0.8;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .now {
    position: absolute;
    left: -1px;
    right: 0;
    height: 2px;
    background: var(--cobalt);
    z-index: 4;
    pointer-events: none;
  }
  .now::before {
    content: "";
    position: absolute;
    left: -5px;
    top: -4px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--cobalt);
    box-shadow: 0 0 0 3px var(--paper);
  }

  /* ---------- Agenda ---------- */
  .agenda {
    flex: 1;
    overflow-y: auto;
  }
  .agenda-inner {
    max-width: 880px;
    margin-inline: auto;
    padding: 8px clamp(20px, 5vw, 64px) 48px;
  }
  .a-day {
    padding: 18px 0 6px;
    border-bottom: 1px solid var(--rule-soft);
    animation: settle var(--dur-slow) var(--ease-ink) backwards;
  }
  .a-day h2 {
    margin: 0 0 8px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.45rem;
    color: var(--ink-2);
  }
  .a-day.today h2 {
    color: var(--cobalt);
  }
  .a-row {
    display: flex;
    align-items: center;
    gap: 12px;
    border-radius: var(--r-md);
    transition: background var(--dur-fast);
  }
  .a-row:hover {
    background: var(--paper-2);
  }
  .a-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 10px;
    text-align: left;
  }
  .a-time {
    width: 7.5rem;
    flex: none;
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }
  .a-dot {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: oklch(0.55 0.18 var(--h));
  }
  .live .a-dot {
    box-shadow: 0 0 0 4px oklch(0.55 0.18 var(--h) / 0.2);
  }
  .a-text {
    min-width: 0;
    display: grid;
  }
  .a-text strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .a-text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .join {
    margin-right: 8px;
    height: 30px;
  }
  .a-empty {
    margin: 0 0 10px;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .a-empty.big {
    margin-top: 12vh;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.8rem;
    color: var(--ink-2);
  }

  /* ---------- Dettaglio ---------- */
  .detail {
    position: absolute;
    z-index: 10;
    top: 96px;
    right: 18px;
    width: min(360px, 40%);
    max-height: calc(100% - 116px);
    overflow-y: auto;
    padding: 14px 18px 18px;
    border-radius: var(--r-lg);
    background: var(--paper);
    box-shadow: var(--shadow-sheet), 0 0 0 1px var(--rule);
    animation: settle var(--dur) var(--ease-ink);
  }
  .d-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .d-swatch {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    background: oklch(0.55 0.18 var(--h));
  }
  .d-account {
    flex: 1;
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .d-title {
    margin: 6px 0 12px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.6rem;
    line-height: 1.12;
    text-wrap: balance;
    user-select: text;
    -webkit-user-select: text;
  }
  .d-when,
  .d-row {
    display: flex;
    gap: 10px;
    margin: 0 0 12px;
    font-size: var(--t-sm);
    line-height: 1.45;
    color: var(--ink-2);
  }
  .d-when :global(svg),
  .d-row :global(svg) {
    flex: none;
    margin-top: 2px;
    color: oklch(0.5 0.18 var(--h));
  }
  .d-when span::first-line {
    color: var(--ink);
    font-weight: 600;
  }
  .live-tag {
    margin-left: 6px;
    font-style: normal;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--cobalt);
  }
  .d-join {
    width: 100%;
    justify-content: center;
    height: 42px;
    margin: 2px 0 16px;
    font-size: var(--t-md);
  }
  .d-row span {
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .d-preview {
    margin: 0 0 12px;
    padding-top: 12px;
    border-top: 1px solid var(--rule-soft);
    font-size: var(--t-sm);
    line-height: 1.5;
    color: var(--ink-2);
    user-select: text;
    -webkit-user-select: text;
  }
  .d-link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--cobalt);
  }
  .d-link:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
