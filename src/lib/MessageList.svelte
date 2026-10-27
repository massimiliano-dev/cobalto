<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { folderLabel } from "./folders";
  import { dateGroup, displayName, listDate } from "./format";
  import type { Envelope } from "./api";
  import { collapse } from "./motion";

  let query = $state("");
  let scroller: HTMLElement | undefined = $state();
  let sentinel: HTMLElement | undefined = $state();
  let searchTimer: ReturnType<typeof setTimeout>;

  // Nelle cartelle di invio mostriamo il destinatario, non il mittente
  let outgoing = $derived(["sent", "drafts"].includes(mail.currentFolder?.role ?? ""));

  let groups = $derived.by(() => {
    const out: { label: string; items: Envelope[] }[] = [];
    for (const m of mail.visible) {
      const label = mail.searchResults ? "Risultati" : dateGroup(m.date);
      const last = out.at(-1);
      if (last && last.label === label) last.items.push(m);
      else out.push({ label, items: [m] });
    }
    return out;
  });

  $effect(() => {
    if (!sentinel || !scroller) return;
    const io = new IntersectionObserver((entries) => entries[0].isIntersecting && mail.loadMore(), {
      root: scroller,
      rootMargin: "400px",
    });
    io.observe(sentinel);
    return () => io.disconnect();
  });

  // Tiene in vista la riga selezionata quando si naviga con j/k
  $effect(() => {
    const uid = mail.selectedUid;
    if (uid == null || !scroller) return;
    scroller.querySelector(`[data-uid="${uid}"]`)?.scrollIntoView({ block: "nearest" });
  });

  $effect(() => {
    if (!mail.searchResults && !mail.query) query = "";
  });

  function onSearchInput() {
    clearTimeout(searchTimer);
    if (!query.trim()) {
      mail.clearSearch();
      return;
    }
    searchTimer = setTimeout(() => mail.search(query), 380);
  }

  function who(m: Envelope) {
    if (outgoing) return m.to.length ? "A: " + m.to.map(displayName).join(", ") : "(nessun destinatario)";
    return displayName(m.from) || "(sconosciuto)";
  }
</script>

<section class="list">
  <header data-tauri-drag-region>
    <div class="title-row" data-tauri-drag-region>
      <h1 data-tauri-drag-region>{folderLabel(mail.currentFolder)}</h1>
      <button class="icon-btn" class:spin={mail.syncing} onclick={() => mail.refresh()} title="Aggiorna">
        <Icon name="refresh" size={16} />
      </button>
    </div>
    <p class="meta" data-tauri-drag-region>
      {#if mail.searchResults}
        {mail.searchResults.length} risultati per «{mail.query}»
      {:else if mail.currentFolder?.unseen}
        <strong>{mail.currentFolder.unseen} da leggere</strong> · {mail.total} messaggi
      {:else if mail.total}
        {mail.total} messaggi
      {:else}
        &nbsp;
      {/if}
    </p>
    <label class="search">
      <Icon name="search" size={15} />
      <input
        id="search"
        type="search"
        placeholder="Cerca in questa cartella"
        bind:value={query}
        oninput={onSearchInput}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            clearTimeout(searchTimer);
            mail.search(query);
          }
          if (e.key === "Escape") {
            query = "";
            mail.clearSearch();
            (e.target as HTMLElement).blur();
          }
        }}
      />
      {#if mail.searching}<span class="dot-pulse"></span>{:else}<kbd>/</kbd>{/if}
    </label>
  </header>

  <div class="scroll" bind:this={scroller}>
    {#if mail.listLoading && !mail.visible.length}
      {#each Array(9) as _, i (i)}
        <div class="row skel">
          <div class="skeleton" style="width:{40 + ((i * 23) % 35)}%;height:11px"></div>
          <div class="skeleton" style="width:{55 + ((i * 13) % 40)}%;height:10px;margin-top:9px"></div>
          <div class="skeleton" style="width:85%;height:9px;margin-top:7px"></div>
        </div>
      {/each}
    {:else if !mail.visible.length}
      <div class="empty">
        {#if mail.searchResults}
          <p class="empty-title">Nessun risultato</p>
          <p>Prova con meno parole, oppure cerca un nome o un indirizzo.</p>
        {:else if mail.currentFolder?.role === "inbox"}
          <p class="empty-title">Tutto letto.</p>
          <p>Niente di nuovo in arrivo. Premi <kbd>C</kbd> per scrivere.</p>
        {:else}
          <p class="empty-title">Cartella vuota</p>
        {/if}
      </div>
    {/if}

    <!-- Cambio cartella: il blocco si ricrea e le righe arrivano in cascata, senza animare le uscite -->
    {#key `${mail.accountId}:${mail.folder}:${mail.searchResults ? mail.query : ""}`}
    {#each groups as g, gi (g.label)}
      <div class="group" style:--i={gi * 3}>{g.label}</div>
      {#each g.items as m, i (m.uid)}
        <div
          in:collapse={{ enabled: mail.fresh.has(m.uid) }}
          out:collapse
          style:--i={Math.min(gi * 3 + i, 12)}
          class="row"
          class:unread={!m.seen}
          class:selected={m.uid === mail.selectedUid}
          data-uid={m.uid}
          role="button"
          tabindex="-1"
          onclick={() => mail.open(m.uid)}
          onkeydown={(e) => e.key === "Enter" && mail.open(m.uid)}
        >
          <span class="ink" aria-hidden="true"></span>
          <div class="line1">
            <span class="from">{who(m)}</span>
            {#if m.hasAttachments}<span class="glyph"><Icon name="clip" size={13} /></span>{/if}
            {#if m.answered}<span class="glyph" title="Risposto"><Icon name="reply" size={13} /></span>{/if}
            <time>{listDate(m.date)}</time>
          </div>
          <div class="subject">{m.subject || "(senza oggetto)"}</div>
          {#if m.preview}<div class="preview">{m.preview}</div>{/if}

          <div class="quick">
            <button title="Archivia (E)" onclick={(e) => (e.stopPropagation(), mail.archive(m.uid))}><Icon name="archive" size={15} /></button>
            <button title="Elimina (#)" onclick={(e) => (e.stopPropagation(), mail.trash(m.uid))}><Icon name="trash" size={15} /></button>
            <button title={m.seen ? "Segna come da leggere (U)" : "Segna come letto (U)"} onclick={(e) => (e.stopPropagation(), mail.toggleSeen(m.uid))}>
              <Icon name={m.seen ? "mail" : "mailOpen"} size={15} />
            </button>
          </div>
          <button
            class="flag"
            class:on={m.flagged}
            title="Contrassegna (S)"
            onclick={(e) => (e.stopPropagation(), mail.toggleFlag(m.uid))}
          >
            {#key m.flagged}<span class="star"><Icon name="star" size={14} fill={m.flagged} /></span>{/key}
          </button>
        </div>
      {/each}
    {/each}
    {/key}

    <div bind:this={sentinel} class="sentinel">
      {#if mail.listLoading && mail.visible.length}<span class="dot-pulse"></span>{/if}
    </div>
  </div>
</section>

<style>
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--paper-2);
    border-right: 1px solid var(--rule);
  }

  header {
    padding: 40px 18px 12px 20px;
    border-bottom: 1px solid var(--rule);
  }
  :global(.shell:not(.mac)) header {
    padding-top: 18px;
  }
  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  h1 {
    margin: 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.85rem;
    line-height: 1.1;
    letter-spacing: -0.01em;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spin :global(svg) {
    animation: rot 0.9s linear infinite;
  }
  @keyframes rot {
    to {
      transform: rotate(360deg);
    }
  }
  .meta {
    margin: 2px 0 12px;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .meta strong {
    color: var(--cobalt);
    font-weight: 650;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 8px 0 10px;
    border-radius: var(--r-sm);
    background: var(--paper);
    color: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: box-shadow 0.15s;
  }
  .search:focus-within {
    box-shadow: inset 0 0 0 1px var(--cobalt), 0 0 0 3px oklch(0.5 0.215 263 / 0.12);
    color: var(--cobalt);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--t-md);
    color: var(--ink);
  }
  .search input::placeholder {
    color: var(--ink-3);
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  /* I giorni come in un'agenda: serif corsivo, non etichette maiuscole */
  .group {
    position: sticky;
    top: 0;
    z-index: 2;
    padding: 14px 20px 4px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.02rem;
    line-height: 1.2;
    color: var(--ink-3);
    background: linear-gradient(var(--paper-2) 72%, oklch(0.977 0.011 255 / 0));
    animation: settle var(--dur) var(--ease-ink) backwards;
    animation-delay: calc(var(--i) * 18ms);
  }

  .row {
    position: relative;
    margin: 0 8px;
    padding: 10px 12px 11px 22px;
    border-radius: var(--r-md);
    outline: none;
    transition: background var(--dur-fast), box-shadow var(--dur);
    animation: settle var(--dur) var(--ease-ink) backwards;
    animation-delay: calc(var(--i) * 18ms);
  }
  .row + .row::before {
    content: "";
    position: absolute;
    top: 0;
    left: 22px;
    right: 12px;
    border-top: 1px solid var(--rule-soft);
  }
  .row:hover {
    background: var(--paper);
  }
  .row:hover::before,
  .row:hover + .row::before,
  .row.selected::before,
  .row.selected + .row::before {
    border-color: transparent;
  }
  .row.selected {
    background: var(--paper);
    box-shadow: 0 0 0 1px var(--rule), 0 6px 18px -10px oklch(0.4 0.15 262 / 0.35);
  }
  /* Selezione: il punto d'inchiostro diventa un anello (pieno se il messaggio è ancora da leggere) */
  .row.selected .ink {
    transform: scale(1);
    background: transparent;
    box-shadow: inset 0 0 0 1.6px var(--cobalt);
  }
  .row.selected.unread .ink {
    background: var(--cobalt);
  }
  .row.selected .from {
    color: var(--cobalt-deep);
  }

  /* Il punto d'inchiostro dei non letti */
  .ink {
    position: absolute;
    left: 9px;
    top: 16px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--cobalt);
    transform: scale(0);
    transition:
      transform var(--dur) var(--ease-ink),
      background var(--dur-fast),
      box-shadow var(--dur-fast);
  }
  .unread .ink {
    transform: scale(1);
  }

  .line1 {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }
  .from {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-md);
    font-weight: 500;
    color: var(--ink-2);
  }
  .unread .from {
    font-weight: 680;
    color: var(--ink);
  }
  .glyph {
    color: var(--ink-3);
    align-self: center;
    display: inline-flex;
  }
  time {
    flex: none;
    font-size: var(--t-xs);
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
  }
  .unread time {
    color: var(--cobalt);
    font-weight: 600;
  }
  .subject {
    margin-top: 2px;
    padding-right: 22px;
    font-size: var(--t-sm);
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .unread .subject {
    color: var(--ink);
    font-weight: 600;
  }
  .preview {
    margin-top: 2px;
    font-size: var(--t-sm);
    line-height: 1.4;
    color: var(--ink-3);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .flag {
    position: absolute;
    right: 10px;
    top: 30px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    color: var(--ink-3);
    opacity: 0;
    transition: opacity 0.12s, color 0.12s;
  }
  .flag.on {
    opacity: 1;
    color: var(--cobalt);
  }
  .flag.on .star {
    animation: stamp 0.32s var(--ease-ink);
  }
  .star {
    display: inline-flex;
  }
  .row:hover .flag {
    opacity: 1;
  }
  .flag:hover {
    color: var(--cobalt);
  }

  .quick {
    position: absolute;
    right: 8px;
    top: 6px;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--paper);
    box-shadow: 0 0 0 1px var(--rule), 0 4px 12px -6px oklch(0.4 0.15 262 / 0.3);
    opacity: 0;
    transform: translateY(-2px);
    pointer-events: none;
    transition: opacity 0.12s, transform 0.15s var(--ease-out);
  }
  .row:hover .quick {
    opacity: 1;
    transform: none;
    pointer-events: auto;
  }
  .quick button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 24px;
    border-radius: 6px;
    color: var(--ink-2);
  }
  .quick button:hover {
    background: var(--cobalt-mist);
    color: var(--cobalt);
  }

  .skel {
    padding-block: 16px;
  }

  .empty {
    padding: 56px 28px;
    color: var(--ink-3);
    font-size: var(--t-md);
    line-height: 1.5;
  }
  .empty-title {
    margin: 0 0 6px;
    font-family: var(--font-serif);
    font-size: 1.6rem;
    font-style: italic;
    color: var(--ink-2);
  }
  .empty p {
    margin: 0;
  }

  .sentinel {
    display: grid;
    place-items: center;
    height: 48px;
  }

  .dot-pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--cobalt);
    animation: pulse 1s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.25;
      transform: scale(0.7);
    }
  }
</style>
