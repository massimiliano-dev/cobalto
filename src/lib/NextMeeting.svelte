<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { cal, joinLabel } from "./calendar.svelte";
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { sheet } from "./motion";

  const timeFmt = new Intl.DateTimeFormat("it-IT", { hour: "2-digit", minute: "2-digit" });

  let ev = $derived(cal.next);
  let label = $derived.by(() => {
    if (!ev) return "";
    const mins = Math.round((ev.start - cal.now) / 60_000);
    if (mins <= 0) return "In corso";
    if (mins === 1) return "Tra 1 minuto";
    return `Tra ${mins} minuti`;
  });
</script>

{#if ev}
  <div class="next" class:live={label === "In corso"} out:sheet={{ y: 8, duration: 160 }}>
    <button
      class="next-main"
      onclick={() => {
        mail.view = "calendar";
        cal.selected = ev;
      }}
    >
      <span class="when">{label} · {timeFmt.format(ev.start)}</span>
      <span class="what">{ev.title}</span>
    </button>
    {#if ev.joinUrl}
      <button class="join" title={joinLabel[ev.joinKind ?? "altro"]} onclick={() => openUrl(ev.joinUrl!)}>
        <Icon name="video" size={15} /> Partecipa
      </button>
    {/if}
  </div>
{/if}

<style>
  .next {
    display: grid;
    gap: 8px;
    margin: 0 2px 10px;
    padding: 10px 10px 10px 12px;
    border-radius: var(--r-md);
    background: oklch(1 0 0 / 0.1);
    box-shadow: inset 0 0 0 1px oklch(1 0 0 / 0.12);
    animation: settle var(--dur-slow) var(--ease-ink);
  }
  .next.live {
    background: var(--paper);
  }
  .next-main {
    display: grid;
    gap: 1px;
    text-align: left;
    min-width: 0;
  }
  .when {
    font-size: var(--t-xs);
    font-weight: 650;
    color: var(--on-deep-2);
    font-variant-numeric: tabular-nums;
  }
  .what {
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--on-deep);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .live .when {
    color: var(--cobalt);
  }
  .live .what {
    color: var(--ink);
  }
  .join {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 32px;
    border-radius: var(--r-sm);
    background: var(--paper);
    color: var(--cobalt-deep);
    font-size: var(--t-sm);
    font-weight: 650;
    transition: transform var(--dur-fast) var(--ease-ink);
  }
  .live .join {
    background: var(--cobalt);
    color: var(--on-deep);
  }
  .join:hover {
    transform: translateY(-1px);
  }
</style>
