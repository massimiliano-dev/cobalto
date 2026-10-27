<script lang="ts">
  import { onMount } from "svelte";
  import { mail } from "./state.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import { folderLabel, roleIcon } from "./folders";
  import { fadeOut } from "./motion";

  interface Command {
    label: string;
    hint?: string;
    icon: IconName;
    keys?: string;
    run: () => void;
  }

  let q = $state("");
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();

  let commands = $derived.by<Command[]>(() => {
    const m = mail.message;
    const list: Command[] = [
      { label: "Nuovo messaggio", icon: "pen", keys: "C", run: () => mail.startCompose() },
      { label: "Chiedi all'assistente", hint: "chat sulla tua posta", icon: "spark", keys: "⌘J", run: () => (mail.showAssistant = true) },
      { label: "Apri il calendario", icon: "calendar", keys: "⌘2", run: () => (mail.view = "calendar") },
      { label: "Torna alla posta", icon: "mail", keys: "⌘1", run: () => (mail.view = "mail") },
    ];
    if (m) {
      list.push(
        { label: "Rispondi", icon: "reply", keys: "R", run: () => mail.startCompose("reply", m) },
        { label: "Rispondi con l'AI", hint: "bozza automatica", icon: "spark", run: () => {
            mail.startCompose("reply", m);
            if (mail.compose) mail.compose.aiIntent = "";
          } },
        { label: "Rispondi a tutti", icon: "replyAll", keys: "A", run: () => mail.startCompose("replyAll", m) },
        { label: "Inoltra", icon: "forward", keys: "F", run: () => mail.startCompose("forward", m) },
        { label: "Archivia", icon: "archive", keys: "E", run: () => mail.archive() },
        { label: "Elimina", icon: "trash", keys: "#", run: () => mail.trash() },
        { label: m.flagged ? "Togli contrassegno" : "Contrassegna", icon: "star", keys: "S", run: () => mail.toggleFlag() },
      );
      for (const f of mail.currentFolders) {
        if (f.path !== mail.folder && f.role !== "all") {
          list.push({ label: `Sposta in ${folderLabel(f)}`, icon: "folder", run: () => mail.moveTo(f.path) });
        }
      }
    }
    for (const f of mail.currentFolders) {
      list.push({ label: `Vai a ${folderLabel(f)}`, hint: f.unseen ? `${f.unseen} da leggere` : undefined, icon: roleIcon[f.role] ?? "folder", run: () => mail.selectFolder(f.path) });
    }
    for (const a of mail.accounts) {
      if (a.id !== mail.accountId) list.push({ label: `Passa a ${a.email}`, icon: "user", run: () => mail.selectAccount(a.id) });
    }
    list.push(
      { label: "Aggiorna", icon: "refresh", run: () => mail.refresh() },
      { label: "Cerca", icon: "search", keys: "/", run: () => setTimeout(() => document.getElementById("search")?.focus()) },
      { label: "Impostazioni", icon: "settings", run: () => (mail.showSettings = true) },
      { label: "Aggiungi account", icon: "plus", run: () => { mail.editAccount = null; mail.showSetup = true; } },
    );
    return list;
  });

  // Ricerca fuzzy leggera: tutte le lettere in ordine, con bonus per le iniziali di parola
  function score(label: string, query: string): number {
    if (!query) return 1;
    const l = label.toLowerCase();
    const qq = query.toLowerCase();
    if (l.includes(qq)) return 100 - l.indexOf(qq);
    let i = 0;
    let s = 0;
    for (const ch of qq) {
      const j = l.indexOf(ch, i);
      if (j < 0) return 0;
      s += j === 0 || l[j - 1] === " " ? 3 : 1;
      i = j + 1;
    }
    return s;
  }

  let results = $derived(
    commands
      .map((c) => ({ c, s: score(c.label, q.trim()) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 12)
      .map((x) => x.c),
  );

  $effect(() => {
    void q;
    active = 0;
  });

  function exec(c: Command | undefined) {
    if (!c) return;
    mail.showPalette = false;
    c.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active = (active + 1) % Math.max(1, results.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = (active - 1 + results.length) % Math.max(1, results.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      exec(results[active]);
    }
  }

  onMount(() => input?.focus());
</script>

<div class="scrim" role="presentation" onclick={() => (mail.showPalette = false)} out:fadeOut={{ duration: 120 }}></div>
<div class="palette" role="dialog" aria-label="Comandi" out:fadeOut={{ duration: 120 }}>
  <div class="search">
    <Icon name="command" size={16} />
    <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder="Cosa vuoi fare?" />
  </div>
  <ul>
    {#each results as c, i (c.label)}
      <li>
        <button class:active={i === active} onmouseenter={() => (active = i)} onclick={() => exec(c)}>
          <Icon name={c.icon} size={16} />
          <span class="label">{c.label}</span>
          {#if c.hint}<small>{c.hint}</small>{/if}
          {#if c.keys}<kbd>{c.keys}</kbd>{/if}
        </button>
      </li>
    {:else}
      <li class="none">Nessun comando</li>
    {/each}
  </ul>
</div>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 80;
    background: oklch(0.25 0.1 265 / 0.25);
  }
  .palette {
    position: absolute;
    z-index: 81;
    top: 14vh;
    left: 50%;
    width: min(560px, calc(100vw - 32px));
    transform: translateX(-50%);
    border-radius: 16px;
    background: var(--paper);
    box-shadow: var(--shadow-sheet), 0 0 0 1px var(--rule);
    overflow: hidden;
    animation: pop 0.2s var(--ease-out);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(-8px) scale(0.98);
    }
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 18px;
    height: 56px;
    color: var(--cobalt);
    border-bottom: 1px solid var(--rule-soft);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: 0;
    background: none;
    font-size: 1.05rem;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 6px;
    max-height: 420px;
    overflow-y: auto;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    color: var(--ink-2);
    font-size: var(--t-md);
    text-align: left;
  }
  li button.active {
    background: var(--cobalt-mist);
    color: var(--cobalt);
  }
  .label {
    flex: 1;
    color: var(--ink);
  }
  small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .none {
    padding: 16px;
    color: var(--ink-3);
    font-size: var(--t-sm);
  }
</style>
