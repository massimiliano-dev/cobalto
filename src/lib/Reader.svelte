<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import MailFrame from "./MailFrame.svelte";
  import AiMargin from "./AiMargin.svelte";
  import { folderLabel } from "./folders";
  import { displayName, fileSize, fullDate, initials, senderHue } from "./format";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { fadeOut } from "./motion";

  const TRUST_KEY = "cobalto:trusted-senders";

  function trusted(): string[] {
    try {
      return JSON.parse(localStorage.getItem(TRUST_KEY) ?? "[]");
    } catch {
      return [];
    }
  }

  let m = $derived(mail.message);
  let allowOnce = $state(false);
  let blocked = $state(0);
  let showMove = $state(false);
  let showDetails = $state(false);

  let allowRemote = $derived(allowOnce || (m ? trusted().includes(m.from.email.toLowerCase()) : false));
  let files = $derived(m?.attachments.filter((a) => !a.inline) ?? []);
  let moveTargets = $derived(mail.currentFolders.filter((f) => f.path !== mail.folder && f.role !== "all"));

  $effect(() => {
    // Nuovo messaggio: si riparte con le immagini bloccate
    void mail.selectedUid;
    allowOnce = false;
    showMove = false;
    showDetails = false;
  });

  function trustSender() {
    if (!m) return;
    const list = new Set(trusted());
    list.add(m.from.email.toLowerCase());
    try {
      localStorage.setItem(TRUST_KEY, JSON.stringify([...list]));
    } catch {}
    allowOnce = true;
  }

  function unsubscribe() {
    const raw = m?.listUnsubscribe ?? "";
    const http = raw.match(/<(https?:[^>]+)>/)?.[1];
    const mailto = raw.match(/<mailto:([^>?]+)/)?.[1];
    if (http) openUrl(http);
    else if (mailto) {
      mail.startCompose();
      if (mail.compose) {
        mail.compose.to = mailto;
        mail.compose.subject = "unsubscribe";
      }
    }
  }
</script>

<main class="reader">
  <div class="toolbar" data-tauri-drag-region>
    {#if m}
      <div class="group">
        <button class="icon-btn" title="Rispondi (R)" onclick={() => mail.startCompose("reply", m)}><Icon name="reply" /></button>
        <button class="icon-btn" title="Rispondi a tutti (A)" onclick={() => mail.startCompose("replyAll", m)}><Icon name="replyAll" /></button>
        <button class="icon-btn" title="Inoltra (F)" onclick={() => mail.startCompose("forward", m)}><Icon name="forward" /></button>
      </div>
      <span class="sep"></span>
      <div class="group">
        <button class="icon-btn" title="Archivia (E)" onclick={() => mail.archive()}><Icon name="archive" /></button>
        <button class="icon-btn" title="Elimina (#)" onclick={() => mail.trash()}><Icon name="trash" /></button>
        <div class="pop-anchor">
          <button class="icon-btn" title="Sposta in…" onclick={() => (showMove = !showMove)}><Icon name="folder" /></button>
          {#if showMove}
            <div class="popover" role="menu" out:fadeOut>
              {#each moveTargets as f (f.path)}
                <button
                  role="menuitem"
                  style:padding-left="{10 + f.depth * 12}px"
                  onclick={() => {
                    showMove = false;
                    mail.moveTo(f.path);
                  }}>{folderLabel(f)}</button
                >
              {/each}
            </div>
          {/if}
        </div>
      </div>
      <span class="sep"></span>
      <div class="group">
        <button class="icon-btn" class:on={m.flagged} title="Contrassegna (S)" onclick={() => mail.toggleFlag()}>
          {#key m.flagged}<span class="star"><Icon name="star" fill={m.flagged} /></span>{/key}
        </button>
        <button class="icon-btn" title="Segna come da leggere (U)" onclick={() => mail.toggleSeen()}><Icon name="mail" /></button>
      </div>
    {/if}
  </div>

  <div class="scroll">
    {#if m}
      {#key m.uid}
        <article class="sheet">
          <div class="main">
            <h2 class="subject">{m.subject || "(senza oggetto)"}</h2>

            <div class="sender">
              <span class="avatar" style:--h={senderHue(m.from.email)}>{initials(m.from)}</span>
              <div class="sender-text">
                <div class="sender-line">
                  <strong>{displayName(m.from)}</strong>
                  <span class="addr">{m.from.email}</span>
                </div>
                <button class="to-line" onclick={() => (showDetails = !showDetails)}>
                  a {m.to.map(displayName).join(", ") || "—"}{m.cc.length ? `, cc ${m.cc.map(displayName).join(", ")}` : ""}
                  <Icon name="down" size={12} />
                </button>
              </div>
              <time>{fullDate(m.date)}</time>
            </div>

            {#if showDetails}
              <dl class="details">
                <dt>Da</dt>
                <dd>{m.from.name} &lt;{m.from.email}&gt;</dd>
                <dt>A</dt>
                <dd>{m.to.map((a) => a.email).join(", ")}</dd>
                {#if m.cc.length}<dt>Cc</dt>
                  <dd>{m.cc.map((a) => a.email).join(", ")}</dd>{/if}
                {#if m.replyTo.length}<dt>Rispondi a</dt>
                  <dd>{m.replyTo.map((a) => a.email).join(", ")}</dd>{/if}
                {#if m.listUnsubscribe}<dt>Lista</dt>
                  <dd><button class="inline-link" onclick={unsubscribe}>Annulla iscrizione</button></dd>{/if}
              </dl>
            {/if}

            {#if files.length}
              <div class="files">
                {#each files as f (f.index)}
                  <button class="file" onclick={() => mail.saveAttachment(f.index)} title="Salva in Download e apri">
                    <span class="file-ico"><Icon name={f.mime.startsWith("image/") ? "image" : "clip"} size={15} /></span>
                    <span class="file-name">{f.name}</span>
                    <span class="file-size">{fileSize(f.size)}</span>
                    <span class="file-dl"><Icon name="download" size={14} /></span>
                  </button>
                {/each}
              </div>
            {/if}

            {#if blocked > 0 && !allowRemote}
              <div class="banner">
                <Icon name="image" size={15} />
                <span>Immagini remote bloccate per proteggerti dal tracciamento.</span>
                <button onclick={() => (allowOnce = true)}>Mostra</button>
                <button onclick={trustSender}>Sempre da {displayName(m.from)}</button>
              </div>
            {/if}

            <div class="body">
              <MailFrame
                html={m.html}
                text={m.text}
                {allowRemote}
                onblocked={(n) => (blocked = n)}
                onmailto={(addr) => {
                  mail.startCompose();
                  if (mail.compose) mail.compose.to = addr;
                }}
              />
            </div>

            <div class="reply-bar">
              <button class="btn btn-line" onclick={() => mail.startCompose("reply", m)}><Icon name="reply" size={15} /> Rispondi</button>
              <button class="btn btn-quiet" onclick={() => mail.startCompose("forward", m)}><Icon name="forward" size={15} /> Inoltra</button>
            </div>
          </div>

          <AiMargin message={m} />
        </article>
      {/key}
    {:else if mail.messageLoading}
      <div class="sheet loading">
        <div class="skeleton" style="width:60%;height:28px"></div>
        <div class="skeleton" style="width:35%;height:12px;margin-top:22px"></div>
        <div class="skeleton" style="width:90%;height:10px;margin-top:40px"></div>
        <div class="skeleton" style="width:84%;height:10px;margin-top:12px"></div>
        <div class="skeleton" style="width:70%;height:10px;margin-top:12px"></div>
      </div>
    {:else if mail.messageError}
      <div class="idle">
        <p class="idle-title">Non riesco ad aprirlo.</p>
        <p>{mail.messageError}</p>
        <button class="btn btn-line" onclick={() => mail.selectedUid && mail.open(mail.selectedUid)}>Riprova</button>
      </div>
    {:else}
      <div class="idle">
        <svg class="idle-mark" width="56" height="56" viewBox="0 0 56 56" aria-hidden="true">
          <rect x="6" y="14" width="44" height="30" rx="5" fill="none" stroke="currentColor" stroke-width="1.6" />
          <path d="M8 17 28 31 48 17" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <p class="idle-title">
          {#if mail.currentFolder?.unseen}
            {mail.currentFolder.unseen === 1 ? "Un messaggio ti aspetta." : `${mail.currentFolder.unseen} messaggi ti aspettano.`}
          {:else}
            Nessun messaggio aperto.
          {/if}
        </p>
        <dl class="keys">
          <dt><kbd>J</kbd> <kbd>K</kbd></dt>
          <dd>scorri</dd>
          <dt><kbd>R</kbd></dt>
          <dd>rispondi</dd>
          <dt><kbd>E</kbd></dt>
          <dd>archivia</dd>
          <dt><kbd>#</kbd></dt>
          <dd>elimina</dd>
          <dt><kbd>C</kbd></dt>
          <dd>scrivi</dd>
          <dt><kbd>⌘</kbd> <kbd>K</kbd></dt>
          <dd>tutti i comandi</dd>
        </dl>
      </div>
    {/if}
  </div>
</main>

<style>
  .reader {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--paper);
    container-type: inline-size;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 52px;
    padding: 8px 18px 0;
    flex: none;
  }
  .group {
    display: flex;
    gap: 2px;
  }
  .sep {
    width: 1px;
    height: 18px;
    margin: 0 6px;
    background: var(--rule);
  }
  .pop-anchor {
    position: relative;
  }
  .popover {
    position: absolute;
    z-index: 20;
    top: 38px;
    left: 0;
    min-width: 220px;
    max-height: 360px;
    overflow-y: auto;
    padding: 5px;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    animation: settle 0.18s var(--ease-out);
  }
  .popover button {
    display: block;
    width: 100%;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    font-size: var(--t-sm);
    text-align: left;
  }
  .popover button:hover {
    background: var(--cobalt-mist);
    color: var(--cobalt);
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
  }

  .sheet {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 28px;
    max-width: 1080px;
    margin-inline: auto;
    padding: 18px clamp(20px, 4cqi, 48px) 64px;
  }
  /* Il foglio si compone in sequenza: oggetto scritto a inchiostro, poi mittente, allegati e testo */
  .sheet .subject {
    animation: ink-write 560ms var(--ease-ink) both;
  }
  .sheet .sender,
  .sheet .files,
  .sheet .banner,
  .sheet .body,
  .sheet .reply-bar,
  .sheet > :global(.margin) {
    animation: settle var(--dur-slow) var(--ease-ink) both;
  }
  .sheet .sender {
    animation-delay: 90ms;
  }
  .sheet .files,
  .sheet .banner {
    animation-delay: 140ms;
  }
  .sheet .body {
    animation-delay: 170ms;
  }
  .sheet .reply-bar {
    animation-delay: 240ms;
  }
  .sheet > :global(.margin) {
    animation-delay: 220ms;
  }
  .icon-btn.on .star {
    animation: stamp 0.32s var(--ease-ink);
  }
  .star {
    display: inline-flex;
  }
  /* Finestra larga: l'AI vive a margine, come un appunto a fianco della lettera */
  @container (min-width: 860px) {
    .sheet {
      grid-template-columns: minmax(0, 1fr) 260px;
      gap: 44px;
    }
    .sheet > :global(.margin) {
      position: sticky;
      top: 18px;
      align-self: start;
      padding-top: 8px;
    }
  }
  @container (max-width: 859px) {
    .sheet > :global(.margin) {
      order: -1;
      padding: 14px 16px;
      border-radius: var(--r-lg);
      background: var(--cobalt-mist);
    }
  }

  .main {
    min-width: 0;
  }
  .subject {
    margin: 0 0 18px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: var(--t-display);
    line-height: 1.12;
    letter-spacing: -0.012em;
    color: var(--ink);
    text-wrap: balance;
    user-select: text;
    -webkit-user-select: text;
  }

  .sender {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    font-size: 0.78rem;
    font-weight: 700;
    color: oklch(0.32 0.14 var(--h));
    background: oklch(0.92 0.045 var(--h));
  }
  .sender-text {
    flex: 1;
    min-width: 0;
  }
  .sender-line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }
  .sender-line strong {
    font-weight: 650;
  }
  .addr {
    font-size: var(--t-sm);
    color: var(--ink-3);
    user-select: text;
    -webkit-user-select: text;
  }
  .to-line {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-top: 2px;
    max-width: 100%;
    font-size: var(--t-sm);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .to-line:hover {
    color: var(--cobalt);
  }
  .sender time {
    flex: none;
    font-size: var(--t-xs);
    color: var(--ink-3);
    padding-top: 3px;
  }
  @container (max-width: 640px) {
    .sender time {
      display: none;
    }
  }

  .details {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 14px;
    margin: 12px 0 0;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--paper-2);
    font-size: var(--t-sm);
    animation: settle 0.2s var(--ease-out);
  }
  .details dt {
    color: var(--ink-3);
  }
  .details dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .inline-link {
    color: var(--cobalt);
    font-weight: 600;
  }

  .files {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 16px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 280px;
    height: 40px;
    padding: 0 10px 0 6px;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
    font-size: var(--t-sm);
    transition: box-shadow 0.15s, background 0.15s;
  }
  .file:hover {
    background: var(--cobalt-mist);
    box-shadow: inset 0 0 0 1px oklch(0.82 0.08 260);
  }
  .file-ico {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 7px;
    background: var(--cobalt-wash);
    color: var(--cobalt);
  }
  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 550;
  }
  .file-size {
    color: var(--ink-3);
    font-size: var(--t-xs);
    white-space: nowrap;
  }
  .file-dl {
    color: var(--ink-3);
    display: inline-flex;
  }
  .file:hover .file-dl {
    color: var(--cobalt);
  }

  .banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 12px;
    margin-top: 16px;
    padding: 9px 12px;
    border-radius: var(--r-md);
    background: var(--cobalt-mist);
    color: var(--ink-2);
    font-size: var(--t-sm);
  }
  .banner span {
    flex: 1;
    min-width: 180px;
  }
  .banner :global(svg) {
    color: var(--cobalt);
  }
  .banner button {
    font-weight: 650;
    color: var(--cobalt);
  }
  .banner button:hover {
    text-decoration: underline;
  }

  .body {
    margin-top: 22px;
  }

  .reply-bar {
    display: flex;
    gap: 8px;
    margin-top: 32px;
    padding-top: 18px;
    border-top: 1px solid var(--rule-soft);
  }

  .loading {
    display: block;
    max-width: 720px;
    padding-top: 30px;
  }

  .idle {
    display: grid;
    place-content: center;
    justify-items: start;
    gap: 10px;
    animation: settle var(--dur-slow) var(--ease-ink) both;
    height: 100%;
    min-height: 360px;
    padding: 24px;
    color: var(--ink-3);
    font-size: var(--t-md);
  }
  .idle p {
    margin: 0;
    max-width: 380px;
  }
  .idle-mark {
    color: var(--cobalt);
    opacity: 0.5;
    margin-bottom: 6px;
  }
  .idle-title {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: clamp(2rem, 1.4rem + 1.6vw, 2.8rem);
    line-height: 1.05;
    letter-spacing: -0.01em;
    color: var(--ink);
    text-wrap: balance;
  }
  .keys {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 14px;
    margin: 18px 0 0;
    padding-top: 16px;
    border-top: 1px solid var(--rule-soft);
    font-size: var(--t-sm);
  }
  .keys dt {
    display: flex;
    gap: 3px;
    justify-content: flex-end;
  }
  .keys dd {
    margin: 0;
    align-self: center;
  }
</style>
