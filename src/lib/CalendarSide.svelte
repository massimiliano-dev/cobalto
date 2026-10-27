<script lang="ts">
  import { mail } from "./state.svelte";
  import { cal, addDays, sameDay, weekStart, accountHue } from "./calendar.svelte";

  const monthFmt = new Intl.DateTimeFormat("it-IT", { month: "long", year: "numeric" });
  const DOW = ["L", "M", "M", "G", "V", "S", "D"];

  let month = $state(new Date(cal.anchor.getFullYear(), cal.anchor.getMonth(), 1));
  let today = $derived(new Date(cal.now));
  let cells = $derived(Array.from({ length: 42 }, (_, i) => addDays(weekStart(month), i)));
  let shownStart = $derived(cal.anchor.getTime());
  let shownEnd = $derived(cal.range.end);

  function statusOf(id: string) {
    return cal.status.find((s) => s.accountId === id);
  }
</script>

<div class="mini">
  <div class="mini-head">
    <span class="mini-title">{monthFmt.format(month)}</span>
    <button aria-label="Mese precedente" onclick={() => (month = new Date(month.getFullYear(), month.getMonth() - 1, 1))}>‹</button>
    <button aria-label="Mese successivo" onclick={() => (month = new Date(month.getFullYear(), month.getMonth() + 1, 1))}>›</button>
  </div>
  <div class="mini-grid">
    {#each DOW as d, i (i)}<span class="dow">{d}</span>{/each}
    {#each cells as d (d.getTime())}
      <button
        class="cell"
        class:other={d.getMonth() !== month.getMonth()}
        class:today={sameDay(d, today)}
        class:shown={d.getTime() >= shownStart && d.getTime() < shownEnd}
        onclick={() => cal.goTo(d)}>{d.getDate()}</button
      >
    {/each}
  </div>
</div>

<div class="section">Calendari</div>
{#each mail.accounts as a, i (a.id)}
  {@const st = statusOf(a.id)}
  <button
    class="cal-row"
    class:off={cal.hidden.includes(a.id) || st?.status === "unsupported"}
    disabled={st?.status === "unsupported"}
    title={st?.status === "unsupported"
      ? "Calendario disponibile per gli account collegati con Microsoft o Google"
      : (st?.message ?? "Mostra o nascondi")}
    onclick={() => cal.toggle(a.id)}
  >
    <span class="swatch" style:--h={accountHue(i)}></span>
    <span class="cal-name">{a.email}</span>
    {#if st?.status === "error"}<span class="err">!</span>{/if}
  </button>
{/each}

<style>
  .mini {
    margin: 2px 4px 14px;
    padding: 10px 8px 8px;
    border-radius: var(--r-md);
    background: oklch(1 0 0 / 0.06);
  }
  .mini-head {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 4px 6px;
  }
  .mini-title {
    flex: 1;
    font-family: var(--font-serif);
    font-size: 1.08rem;
    text-transform: capitalize;
    color: var(--on-deep);
  }
  .mini-head button {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    color: var(--on-deep-2);
    font-size: 1rem;
  }
  .mini-head button:hover {
    background: oklch(1 0 0 / 0.1);
    color: var(--on-deep);
  }
  .mini-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 1px;
    text-align: center;
  }
  .dow {
    font-size: 0.62rem;
    font-weight: 700;
    color: var(--on-deep-3);
    padding-bottom: 3px;
  }
  .cell {
    height: 24px;
    border-radius: 6px;
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    color: var(--on-deep-2);
    transition: background var(--dur-fast), color var(--dur-fast);
  }
  .cell:hover {
    background: oklch(1 0 0 / 0.1);
    color: var(--on-deep);
  }
  .cell.other {
    color: var(--on-deep-3);
    opacity: 0.6;
  }
  .cell.shown {
    background: oklch(1 0 0 / 0.1);
    color: var(--on-deep);
  }
  .cell.today {
    background: var(--paper);
    color: var(--cobalt-deep);
    font-weight: 700;
  }

  .section {
    margin: 6px 12px 4px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1rem;
    color: var(--on-deep-3);
  }
  .cal-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    font-size: var(--t-sm);
    color: var(--on-deep);
    transition: background var(--dur-fast), opacity var(--dur);
  }
  .cal-row:hover:not(:disabled) {
    background: oklch(1 0 0 / 0.07);
  }
  .cal-row.off {
    opacity: 0.5;
  }
  .swatch {
    flex: none;
    width: 12px;
    height: 12px;
    border-radius: 4px;
    background: oklch(0.72 0.14 var(--h));
    box-shadow: inset 0 0 0 1px oklch(1 0 0 / 0.25);
    transition: background var(--dur);
  }
  .off .swatch {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px oklch(0.72 0.14 var(--h));
  }
  .cal-name {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .err {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    font-size: 0.65rem;
    font-weight: 800;
    background: var(--on-deep);
    color: var(--cobalt-deep);
  }
</style>
