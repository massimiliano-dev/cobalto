<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorText, type Account } from "./api";

  let { owner }: { owner: Account } = $props();

  type Found = { email: string; name: string; unread: number };

  let found = $state<Found[] | null>(null);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let manual = $state("");

  let active = $derived(mail.accounts.filter((a) => a.sharedOf === owner.id));
  // Le attive restano in elenco anche prima di una ricerca, o se la ricerca non le trova
  let rows = $derived.by(() => {
    const list: Found[] = [...(found ?? [])];
    for (const a of active) {
      if (!list.some((f) => f.email.toLowerCase() === a.email.toLowerCase())) list.push({ email: a.email, name: a.name, unread: 0 });
    }
    return list;
  });

  function activeFor(email: string) {
    return active.find((a) => a.email.toLowerCase() === email.toLowerCase());
  }

  async function search() {
    searching = true;
    error = null;
    try {
      found = await api.sharedMailboxes(owner.id);
    } catch (e) {
      error = errorText(e);
    } finally {
      searching = false;
    }
  }

  async function toggle(f: Found) {
    busy = f.email;
    error = null;
    try {
      const existing = activeFor(f.email);
      if (existing) {
        await mail.removeAccount(existing.id);
      } else {
        const acc = await api.sharedAdd(owner.id, f.email, f.name);
        await mail.sharedAdded(acc);
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function addManual(e: SubmitEvent) {
    e.preventDefault();
    const email = manual.trim();
    if (!email) return;
    await toggle({ email, name: "", unread: 0 });
    if (!error) manual = "";
  }
</script>

<div class="shared">
  <div class="shared-head">
    <span>Caselle condivise</span>
    <button class="link" disabled={searching} onclick={search}>
      {searching ? "Cerco…" : found ? "Aggiorna" : "Cerca"}
    </button>
  </div>

  {#if found && !found.length && !active.length}
    <p class="hint">Nessuna casella condivisa a cui hai accesso. Se ne conosci l'indirizzo, aggiungila qui sotto.</p>
  {/if}

  {#each rows as f (f.email)}
    {@const on = !!activeFor(f.email)}
    <div class="mbox">
      <span class="mbox-text">
        <strong>{f.name}</strong>
        <small>{f.email}{f.unread ? ` · ${f.unread} da leggere` : ""}</small>
      </span>
      <button
        class="switch"
        class:on
        role="switch"
        aria-checked={on}
        aria-label="Mostra {f.email}"
        disabled={busy === f.email}
        onclick={() => toggle(f)}
      >
        <span></span>
      </button>
    </div>
  {/each}

  <form class="manual" onsubmit={addManual}>
    <input class="input" bind:value={manual} placeholder="info@azienda.it" spellcheck="false" />
    <button class="btn btn-line" disabled={!manual.trim() || !!busy}><Icon name="plus" size={14} /> Aggiungi</button>
  </form>

  {#if error}<p class="err">{error}</p>{/if}
</div>

<style>
  .shared {
    display: grid;
    gap: 6px;
    margin: -6px 0 4px 12px;
    padding: 10px 0 4px 14px;
    border-left: 1px solid var(--rule);
  }
  .shared-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink-2);
  }
  .link {
    font-size: var(--t-xs);
    font-weight: 650;
    color: var(--cobalt);
  }
  .link:disabled {
    color: var(--ink-3);
  }
  .hint {
    margin: 0;
    font-size: var(--t-xs);
    line-height: 1.45;
    color: var(--ink-3);
  }
  .mbox {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
    animation: settle var(--dur) var(--ease-ink) backwards;
  }
  .mbox-text {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .mbox-text strong {
    font-size: var(--t-sm);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mbox-text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .switch {
    flex: none;
    position: relative;
    width: 34px;
    height: 20px;
    border-radius: 999px;
    background: var(--paper-3);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: background var(--dur) var(--ease-ink);
  }
  .switch span {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--paper);
    box-shadow: 0 1px 3px oklch(0.3 0.1 264 / 0.3);
    transition: transform var(--dur) var(--ease-ink);
  }
  .switch.on {
    background: var(--cobalt);
    box-shadow: none;
  }
  .switch.on span {
    transform: translateX(14px);
  }
  .switch:disabled {
    opacity: 0.6;
  }
  .manual {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }
  .manual .input {
    height: 32px;
    font-size: var(--t-sm);
  }
  .manual .btn {
    height: 32px;
  }
  .err {
    margin: 0;
    font-size: var(--t-xs);
    line-height: 1.45;
    color: var(--danger);
  }
</style>
