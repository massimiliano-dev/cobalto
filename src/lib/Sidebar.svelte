<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { folderLabel, roleIcon } from "./folders";
  import { initials } from "./format";
  import CalendarSide from "./CalendarSide.svelte";
  import NextMeeting from "./NextMeeting.svelte";

  let system = $derived(mail.currentFolders.filter((f) => f.role));
  let custom = $derived(mail.currentFolders.filter((f) => !f.role));
  let accountsOpen = $state(false);
  // Ogni account seguito dalle sue caselle condivise
  let ordered = $derived(
    mail.accounts.filter((a) => !a.sharedOf).flatMap((a) => [a, ...mail.accounts.filter((x) => x.sharedOf === a.id)]),
  );
  // Gli altri account hanno posta nuova? Il badge mostra solo quello aperto, qui si vede il resto
  let othersUnread = $derived(mail.accounts.some((a) => a.id !== mail.accountId && mail.unreadFor(a.id) > 0));
  let nav: HTMLElement | undefined = $state();
  // Un solo evidenziatore che scorre tra le cartelle, invece di uno sfondo che salta
  let mark = $state<{ y: number; h: number } | null>(null);

  $effect(() => {
    void mail.folder;
    void mail.currentFolders.length;
    const el = nav?.querySelector<HTMLElement>(`[data-path="${CSS.escape(mail.folder)}"]`);
    mark = el ? { y: el.offsetTop, h: el.offsetHeight } : null;
  });
</script>

<aside class="sidebar">
  <div class="drag" data-tauri-drag-region></div>

  <button class="compose" onclick={() => mail.startCompose()}>
    <Icon name="pen" size={16} />
    <span>Scrivi</span>
    <kbd>C</kbd>
  </button>

  <div class="views" role="tablist" aria-label="Sezione">
    <button role="tab" aria-selected={mail.view === "mail"} class:on={mail.view === "mail"} onclick={() => (mail.view = "mail")} title="Posta (⌘1)">
      <Icon name="mail" size={15} /> Posta
    </button>
    <button role="tab" aria-selected={mail.view === "calendar"} class:on={mail.view === "calendar"} onclick={() => (mail.view = "calendar")} title="Calendario (⌘2)">
      <Icon name="calendar" size={15} /> Calendario
    </button>
  </div>

  {#if mail.account && mail.view === "mail"}
    <div class="account">
      <button class="account-current" onclick={() => (accountsOpen = !accountsOpen)} aria-expanded={accountsOpen}>
        <span class="avatar">
          {initials({ name: mail.account.name, email: mail.account.email })}
          {#if othersUnread}<span class="others" title="Posta nuova in un altro account"></span>{/if}
        </span>
        <span class="who">
          <strong>{mail.account.name || mail.account.email}{#if mail.account.sharedOf}<em class="shared-tag">condivisa</em>{/if}</strong>
          <small>{mail.account.email}</small>
        </span>
        <span class="caret" class:open={accountsOpen}><Icon name="down" size={14} /></span>
      </button>
      {#if accountsOpen}
        <div class="account-menu">
          {#each ordered as a (a.id)}
            <button
              class="account-row"
              class:shared={!!a.sharedOf}
              class:active={a.id === mail.accountId}
              onclick={() => {
                mail.selectAccount(a.id);
                accountsOpen = false;
              }}
            >
              <span class="avatar sm">{initials({ name: a.name, email: a.email })}</span>
              <span class="row-email">{a.email}{#if a.sharedOf}<em>condivisa</em>{/if}</span>
              {#if mail.unreadFor(a.id) > 0}<span class="row-count">{mail.unreadFor(a.id)}</span>{/if}
              {#if a.id === mail.accountId}<Icon name="check" size={14} />{/if}
            </button>
          {/each}
          <button
            class="account-row add"
            onclick={() => {
              mail.editAccount = null;
              mail.showSetup = true;
              accountsOpen = false;
            }}
          >
            <span class="avatar sm ghost"><Icon name="plus" size={13} /></span>
            <span>Aggiungi account</span>
          </button>
        </div>
      {/if}
    </div>
  {/if}

  {#if mail.view === "calendar"}
  <div class="folders">
    <CalendarSide />
  </div>
  {:else}
  <nav class="folders" bind:this={nav}>
    {#if mark}
      <span class="mark" style:transform="translateY({mark.y}px)" style:height="{mark.h}px" aria-hidden="true"></span>
    {/if}
    {#if !mail.currentFolders.length}
      {#each [0, 1, 2, 3, 4] as i (i)}
        <div class="folder-skel" style:width="{60 + ((i * 17) % 30)}%"></div>
      {/each}
    {/if}
    {#each system as f (f.path)}
      <button class="folder" data-path={f.path} class:active={f.path === mail.folder} onclick={() => mail.selectFolder(f.path)}>
        <Icon name={roleIcon[f.role] ?? "folder"} size={17} />
        <span class="label">{folderLabel(f)}</span>
        {#if f.unseen}<span class="count">{f.unseen > 999 ? "999+" : f.unseen}</span>{/if}
      </button>
    {/each}

    {#if custom.length}
      <div class="section">Cartelle</div>
      {#each custom as f (f.path)}
        <button
          class="folder"
          data-path={f.path}
          class:active={f.path === mail.folder}
          style:padding-left="{12 + Math.min(f.depth, 4) * 14}px"
          onclick={() => mail.selectFolder(f.path)}
        >
          <Icon name="folder" size={16} />
          <span class="label">{f.name}</span>
          {#if f.unseen}<span class="count">{f.unseen}</span>{/if}
        </button>
      {/each}
    {/if}
  </nav>
  {/if}

  <NextMeeting />

  <footer>
    <button class="foot-btn" class:on={mail.showAssistant} onclick={() => (mail.showAssistant = !mail.showAssistant)}>
      <Icon name="spark" size={16} />
      <span>Assistente</span>
      <kbd>⌘J</kbd>
    </button>
    <button class="foot-btn" onclick={() => (mail.showPalette = true)}>
      <Icon name="command" size={16} />
      <span>Comandi</span>
      <kbd>⌘K</kbd>
    </button>
    <button class="foot-btn" onclick={() => (mail.showSettings = true)}>
      <Icon name="settings" size={16} />
      <span>Impostazioni</span>
    </button>
  </footer>
</aside>

<style>
  .sidebar {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    color: var(--on-deep);
    background:
      radial-gradient(120% 60% at 0% 0%, oklch(0.42 0.19 262) 0%, transparent 60%),
      linear-gradient(180deg, var(--cobalt-deep), var(--cobalt-night));
    padding: 0 10px 10px;
    overflow: hidden;
  }
  /* Filigrana: righe sottilissime come carta da lettere */
  .sidebar::before {
    content: "";
    position: absolute;
    inset: 0;
    background: repeating-linear-gradient(180deg, transparent 0 27px, oklch(1 0 0 / 0.025) 27px 28px);
    pointer-events: none;
  }
  .sidebar > :global(*) {
    position: relative;
  }

  .drag {
    height: 44px;
    flex: none;
  }
  :global(.shell:not(.mac)) .drag {
    height: 14px;
  }

  .views {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
    margin: 0 2px 12px;
    padding: 3px;
    border-radius: var(--r-md);
    background: oklch(0.2 0.1 266 / 0.4);
  }
  .views button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 28px;
    border-radius: 7px;
    font-size: var(--t-sm);
    font-weight: 550;
    color: var(--on-deep-2);
    transition: background var(--dur) var(--ease-ink), color var(--dur-fast);
  }
  .views button:hover {
    color: var(--on-deep);
  }
  .views button.on {
    background: oklch(1 0 0 / 0.16);
    color: var(--on-deep);
  }
  .compose {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    height: 40px;
    padding: 0 12px;
    margin: 0 2px 14px;
    border-radius: var(--r-md);
    background: var(--paper);
    color: var(--cobalt-deep);
    font-weight: 650;
    font-size: var(--t-md);
    box-shadow: 0 1px 0 oklch(1 0 0 / 0.5) inset, 0 6px 16px -6px oklch(0.15 0.1 266 / 0.6);
    transition: transform 0.15s var(--ease-out), box-shadow 0.15s;
  }
  .compose:hover {
    transform: translateY(-1px);
    box-shadow: 0 1px 0 oklch(1 0 0 / 0.5) inset, 0 10px 20px -8px oklch(0.15 0.1 266 / 0.7);
  }
  .compose span {
    flex: 1;
    text-align: left;
  }
  .compose kbd {
    background: var(--cobalt-wash);
    box-shadow: none;
    color: var(--cobalt);
  }

  .account {
    margin-bottom: 10px;
  }
  .account-current {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 8px;
    border-radius: var(--r-md);
    text-align: left;
  }
  .account-current:hover {
    background: oklch(1 0 0 / 0.07);
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--cobalt-deep);
    background: var(--on-deep-2);
  }
  .avatar {
    position: relative;
  }
  .others {
    position: absolute;
    top: -1px;
    right: -1px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--on-deep);
    box-shadow: 0 0 0 2px var(--cobalt-deep);
    animation: stamp 0.32s var(--ease-ink);
  }
  .account-row.shared {
    padding-left: 20px;
  }
  .row-email em {
    margin-left: 6px;
    font-style: normal;
    font-size: 0.66rem;
    font-weight: 650;
    color: var(--on-deep-3);
  }
  .row-count {
    font-size: var(--t-xs);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--on-deep);
  }
  .avatar.sm {
    width: 24px;
    height: 24px;
    font-size: 0.62rem;
  }
  .avatar.ghost {
    background: oklch(1 0 0 / 0.12);
    color: var(--on-deep);
  }
  .who {
    display: grid;
    min-width: 0;
    flex: 1;
  }
  .who strong,
  .shared-tag {
    margin-left: 6px;
    font-style: normal;
    font-size: 0.66rem;
    font-weight: 650;
    color: var(--on-deep-3);
  }
  .who small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who strong {
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .who small {
    font-size: var(--t-xs);
    color: var(--on-deep-3);
  }
  .caret {
    color: var(--on-deep-3);
    transition: transform 0.2s var(--ease-out);
  }
  .caret.open {
    transform: rotate(180deg);
  }
  .account-menu {
    transform-origin: top;
    margin: 4px 0 6px;
    padding: 4px;
    border-radius: var(--r-md);
    background: oklch(0.2 0.1 266 / 0.45);
    animation: settle 0.2s var(--ease-out);
  }
  .account-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 6px;
    border-radius: var(--r-sm);
    font-size: var(--t-sm);
    color: var(--on-deep-2);
  }
  .account-row:hover,
  .account-row.active {
    background: oklch(1 0 0 / 0.08);
    color: var(--on-deep);
  }
  .row-email {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folders {
    position: relative;
    flex: 1;
    overflow-y: auto;
    margin: 0 -4px;
    padding: 0 4px;
  }
  .mark {
    position: absolute;
    top: 0;
    left: 4px;
    right: 4px;
    border-radius: var(--r-sm);
    background: oklch(1 0 0 / 0.14);
    box-shadow: inset 0 1px 0 oklch(1 0 0 / 0.08);
    transition:
      transform var(--dur-slow) var(--ease-ink),
      height var(--dur) var(--ease-ink);
    pointer-events: none;
  }
  .folders::-webkit-scrollbar-thumb {
    background: oklch(1 0 0 / 0.15);
    background-clip: content-box;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    font-size: var(--t-md);
    color: var(--on-deep-2);
    transition: background 0.12s, color 0.12s;
  }
  .folder:hover {
    background: oklch(1 0 0 / 0.07);
    color: var(--on-deep);
  }
  .folder {
    position: relative;
  }
  .folder.active,
  .folder.active:hover {
    background: none;
    color: var(--on-deep);
    font-weight: 600;
  }
  .folder .label {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: var(--t-xs);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--on-deep);
  }
  .folder:not(.active) .count {
    color: var(--on-deep-2);
  }
  .section {
    margin: 20px 12px 4px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1rem;
    color: var(--on-deep-3);
  }
  .folder-skel {
    height: 12px;
    margin: 14px 12px;
    border-radius: 4px;
    background: oklch(1 0 0 / 0.08);
  }

  footer {
    display: grid;
    gap: 2px;
    padding-top: 8px;
    border-top: 1px solid oklch(1 0 0 / 0.08);
  }
  .foot-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    font-size: var(--t-sm);
    color: var(--on-deep-2);
  }
  .foot-btn span {
    flex: 1;
    text-align: left;
  }
  .foot-btn.on {
    background: oklch(1 0 0 / 0.14);
    color: var(--on-deep);
  }
  .foot-btn:hover {
    background: oklch(1 0 0 / 0.07);
    color: var(--on-deep);
  }
  .foot-btn kbd {
    background: oklch(1 0 0 / 0.1);
    color: var(--on-deep-2);
    box-shadow: none;
  }
</style>
