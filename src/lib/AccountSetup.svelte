<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import SignatureEditor from "./SignatureEditor.svelte";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorText, type Account, type OAuthProvider } from "./api";
  import { providers, providerForEmail, type Provider } from "./providers";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { fadeOut } from "./motion";

  let { welcome = false }: { welcome?: boolean } = $props();

  const editing = mail.editAccount;

  let form = $state<Account>(
    editing
      ? { ...editing }
      : {
          id: "",
          name: "",
          email: "",
          username: "",
          imapHost: "",
          imapPort: 993,
          smtpHost: "",
          smtpPort: 465,
          smtpSecurity: "ssl",
          signature: "",
        },
  );
  let password = $state("");
  let providerId = $state<string>(editing ? "other" : "");
  let advanced = $state(!!editing);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let usernameTouched = $state(!!editing);

  let provider = $derived(providers.find((p) => p.id === providerId));
  let oauthReady = $state<OAuthProvider[]>([]);
  let oauthBusy = $state(false);
  let isOauthAccount = $derived(!!editing?.auth?.startsWith("oauth-"));
  // Microsoft accetta solo OAuth: niente campi password
  let passwordForm = $derived(!isOauthAccount && !provider?.oauthOnly);
  let oauthTarget = $derived<OAuthProvider | undefined>(
    isOauthAccount ? (editing!.auth!.slice(6) as OAuthProvider) : provider?.oauth,
  );
  const oauthLabel: Record<OAuthProvider, string> = { microsoft: "Microsoft", google: "Google" };

  let oauthUrl = $state<string | null>(null);

  onMount(() => {
    api.oauthAvailable().then((r) => (oauthReady = r)).catch(() => {});
    const un = listen<string>("oauth-url", (e) => (oauthUrl = e.payload));
    return () => {
      un.then((f) => f());
      // Chiudendo questa schermata non si lascia un accesso in sospeso
      if (oauthBusy) api.oauthCancel();
    };
  });

  function cancelOAuth() {
    api.oauthCancel();
    oauthBusy = false;
    oauthUrl = null;
  }

  async function connectOAuth() {
    if (!oauthTarget) return;
    error = null;
    oauthBusy = true;
    try {
      const saved = await api.oauthConnect(oauthTarget);
      if (form.signature && !saved.signature) {
        saved.signature = form.signature;
        await api.saveAccount(saved, null);
      }
      await mail.accountSaved(saved);
    } catch (err) {
      const text = errorText(err);
      // L'annullamento è una scelta, non un errore da mostrare
      if (text !== "Accesso annullato") error = text;
    } finally {
      oauthBusy = false;
      oauthUrl = null;
    }
  }

  function applyProvider(p: Provider) {
    providerId = p.id;
    form.imapHost = p.imapHost;
    form.imapPort = p.imapPort;
    form.smtpHost = p.smtpHost;
    form.smtpPort = p.smtpPort;
    form.smtpSecurity = p.smtpSecurity;
    if (p.id === "other") {
      advanced = true;
      guessHosts();
    }
  }

  function guessHosts() {
    const domain = form.email.split("@")[1];
    if (!domain) return;
    if (!form.imapHost) form.imapHost = `imap.${domain}`;
    if (!form.smtpHost) form.smtpHost = `smtp.${domain}`;
  }

  function onEmail() {
    if (!usernameTouched) form.username = form.email.trim();
    if (editing) return;
    const p = providerForEmail(form.email);
    if (p && p.id !== providerId) applyProvider(p);
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    if (isOauthAccount) {
      busy = true;
      try {
        await mail.accountSaved(await api.saveAccount($state.snapshot(form), null));
      } catch (err) {
        error = errorText(err);
      } finally {
        busy = false;
      }
      return;
    }
    if (!form.email.includes("@")) return (error = "Inserisci un indirizzo email valido");
    if (!editing && !password) return (error = "Inserisci la password");
    if (!form.imapHost || !form.smtpHost) {
      advanced = true;
      return (error = "Completa i server IMAP e SMTP");
    }
    form.email = form.email.trim();
    form.username = (form.username || form.email).trim();
    if (!form.name.trim()) form.name = form.email.split("@")[0];
    busy = true;
    try {
      const saved = await api.saveAccount($state.snapshot(form), password || null);
      await mail.accountSaved(saved);
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  function close() {
    mail.showSetup = false;
    mail.editAccount = null;
  }
</script>

<div class="overlay" class:welcome out:fadeOut={{ duration: 160 }}>
  {#if welcome}
    <section class="hero" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <svg width="34" height="34" viewBox="0 0 1024 1024" aria-hidden="true">
          <rect x="64" y="64" width="896" height="896" rx="208" fill="oklch(0.98 0.012 255)" />
          <path d="M262 352 L512 540 L762 352" fill="none" stroke="oklch(0.42 0.2 263)" stroke-width="64" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        Cobalto
      </div>
      <h1 data-tauri-drag-region>La posta,<br /><em>senza il peso.</em></h1>
      <ul data-tauri-drag-region>
        <li><strong>Leggero.</strong> Pochi megabyte, si apre all'istante.</li>
        <li><strong>Assistente a scelta.</strong> Claude o GPT: riassume, risponde, riscrive.</li>
        <li><strong>Riservato.</strong> Password nel portachiavi di sistema, immagini traccianti bloccate.</li>
      </ul>
    </section>
  {/if}

  <form class="card" onsubmit={save}>
    <div class="card-head">
      <h2>{editing ? "Modifica account" : welcome ? "Collega la tua casella" : "Aggiungi un account"}</h2>
      {#if !welcome}
        <button type="button" class="icon-btn" onclick={close} aria-label="Chiudi"><Icon name="x" /></button>
      {/if}
    </div>

    {#if !editing}
      <div class="providers">
        {#each providers as p (p.id)}
          <button type="button" class="prov" class:active={p.id === providerId} onclick={() => applyProvider(p)}>{p.label}</button>
        {/each}
      </div>
    {/if}

    {#if oauthTarget}
      <div class="oauth">
        {#if oauthReady.includes(oauthTarget)}
          <button type="button" class="btn btn-primary big oauth-btn" disabled={oauthBusy} onclick={connectOAuth}>
            {#if oauthBusy}
              Completa l'accesso nel browser…
            {:else}
              {isOauthAccount ? "Ricollega" : "Accedi con"} {oauthLabel[oauthTarget]}
            {/if}
          </button>
          {#if oauthBusy}
            <div class="oauth-wait">
              {#if oauthUrl}
                <button type="button" class="btn btn-quiet" onclick={() => openUrl(oauthUrl!)}>Riapri la pagina</button>
              {/if}
              <button type="button" class="btn btn-quiet" onclick={cancelOAuth}>Annulla</button>
            </div>
          {/if}
          <small class="note">
            {#if oauthBusy}
              Si è aperta una pagina di {oauthLabel[oauthTarget]}: dopo l'accesso torna qui.
            {:else if provider?.note && provider.oauthOnly}
              {provider.note}
            {:else}
              Nessuna password salvata: Cobalto riceve solo un accesso revocabile alla posta.
            {/if}
          </small>
        {:else}
          <p class="warn">
            L'accesso con {oauthLabel[oauthTarget]} non è ancora configurato in questa build: manca l'ID applicazione
            (vedi README → OAuth).{provider?.oauthOnly ? "" : " Nel frattempo puoi usare la password qui sotto."}
          </p>
        {/if}
      </div>
      {#if passwordForm}
        <div class="or"><span>oppure con password</span></div>
      {/if}
    {/if}

    {#if passwordForm || isOauthAccount}
    <div class="grid">
      <label class="field" class:span={isOauthAccount}>
        <span>Il tuo nome</span>
        <input class="input" bind:value={form.name} placeholder="Mario Rossi" />
      </label>
      {#if isOauthAccount}
        <div class="field span">
          <span>Firma</span>
          <SignatureEditor bind:value={form.signature} />
        </div>
      {:else}
      <label class="field">
        <span>Email</span>
        <input class="input" type="email" bind:value={form.email} oninput={onEmail} onblur={() => providerId === "other" && guessHosts()} placeholder="mario@esempio.it" spellcheck="false" />
      </label>
      <label class="field span">
        <span>Password {editing ? "(lascia vuoto per non cambiarla)" : ""}</span>
        <input class="input" type="password" bind:value={password} autocomplete="current-password" />
        {#if provider?.note}
          <small class="note">
            {provider.note}
            {#if provider.helpUrl}<button type="button" onclick={() => openUrl(provider!.helpUrl!)}>Apri la pagina</button>{/if}
          </small>
        {/if}
      </label>
      {/if}
    </div>
    {/if}

    {#if passwordForm}
    <button type="button" class="adv-toggle" onclick={() => (advanced = !advanced)}>
      <span class:open={advanced}><Icon name="chevron" size={13} /></span> Server e firma
    </button>

    {#if advanced}
      <div class="grid adv">
        <label class="field span">
          <span>Nome utente</span>
          <input class="input" bind:value={form.username} oninput={() => (usernameTouched = true)} spellcheck="false" />
        </label>
        <label class="field">
          <span>Server IMAP</span>
          <input class="input" bind:value={form.imapHost} placeholder="imap.esempio.it" spellcheck="false" />
        </label>
        <label class="field narrow">
          <span>Porta</span>
          <input class="input" type="number" bind:value={form.imapPort} />
        </label>
        <label class="field">
          <span>Server SMTP</span>
          <input class="input" bind:value={form.smtpHost} placeholder="smtp.esempio.it" spellcheck="false" />
        </label>
        <label class="field narrow">
          <span>Porta</span>
          <input
            class="input"
            type="number"
            bind:value={form.smtpPort}
            oninput={() => (form.smtpSecurity = form.smtpPort === 587 ? "starttls" : "ssl")}
          />
        </label>
        <div class="field span">
          <span>Firma</span>
          <SignatureEditor bind:value={form.signature} />
        </div>
      </div>
    {/if}
    {/if}

    {#if error}<p class="error">{error}</p>{/if}

    <div class="actions">
      {#if editing}
        <button
          type="button"
          class="btn btn-quiet danger"
          onclick={async () => {
            await mail.removeAccount(editing.id);
            close();
          }}>Rimuovi account</button
        >
      {/if}
      <span class="spacer"></span>
      {#if passwordForm || isOauthAccount}
        <button class="btn {passwordForm && oauthTarget && !isOauthAccount ? 'btn-line' : 'btn-primary'} big" disabled={busy}>
          {busy ? "Verifico la connessione…" : editing ? "Salva" : oauthTarget ? "Collega con password" : "Collega"}
        </button>
      {/if}
    </div>
  </form>
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: oklch(0.25 0.1 265 / 0.35);
    animation: fade 0.2s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .overlay.welcome {
    grid-column: 1 / -1;
    position: relative;
    grid-template-columns: minmax(0, 1fr) minmax(420px, 520px);
    gap: clamp(24px, 5vw, 80px);
    padding: 48px clamp(28px, 6vw, 96px);
    background:
      radial-gradient(80% 70% at 10% 0%, oklch(0.45 0.2 262) 0%, transparent 60%),
      linear-gradient(160deg, var(--cobalt-deep), var(--cobalt-night));
  }
  @media (max-width: 960px) {
    .overlay.welcome {
      grid-template-columns: 1fr;
      overflow-y: auto;
    }
  }

  .hero {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    justify-content: center;
    color: var(--on-deep);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 650;
    font-size: 1.05rem;
    letter-spacing: 0.01em;
  }
  .hero h1 {
    margin: 36px 0 28px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(2.8rem, 2rem + 3vw, 4.6rem);
    line-height: 0.98;
    letter-spacing: -0.02em;
  }
  .hero em {
    color: var(--on-deep-2);
  }
  .hero ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 12px;
    max-width: 420px;
    color: var(--on-deep-2);
    font-size: var(--t-md);
    line-height: 1.45;
  }
  .hero li strong {
    color: var(--on-deep);
    font-weight: 600;
  }

  .card {
    width: min(540px, 100%);
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    padding: 24px 26px 22px;
    border-radius: 18px;
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    animation: settle 0.3s var(--ease-out);
  }
  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }
  h2 {
    margin: 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.75rem;
    line-height: 1.1;
  }

  .providers {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 18px;
  }
  .prov {
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    font-size: var(--t-sm);
    font-weight: 550;
    color: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: background 0.12s, color 0.12s, box-shadow 0.12s;
  }
  .prov:hover {
    background: var(--cobalt-mist);
  }
  .prov.active {
    background: var(--cobalt);
    color: var(--on-deep);
    box-shadow: none;
  }

  .warn {
    margin: -6px 0 16px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--cobalt-mist);
    color: var(--ink-2);
    font-size: var(--t-sm);
    line-height: 1.45;
  }

  .oauth {
    display: grid;
    gap: 8px;
    margin-bottom: 6px;
  }
  .oauth .warn {
    margin: 0;
  }
  .oauth-wait {
    display: flex;
    justify-content: center;
    gap: 4px;
    animation: settle var(--dur) var(--ease-ink);
  }
  .oauth-btn {
    justify-content: center;
    width: 100%;
    height: 44px;
  }
  .or {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 18px 0 14px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .or::before,
  .or::after {
    content: "";
    flex: 1;
    border-top: 1px solid var(--rule-soft);
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .grid.adv {
    grid-template-columns: 1fr 96px;
    margin-top: 12px;
    animation: settle 0.2s var(--ease-out);
  }
  .span {
    grid-column: 1 / -1;
  }
  .note {
    font-size: var(--t-xs);
    color: var(--ink-3);
    line-height: 1.4;
  }
  .note button {
    color: var(--cobalt);
    font-weight: 600;
    font-size: inherit;
  }

  .adv-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink-2);
  }
  .adv-toggle span {
    display: inline-flex;
    transition: transform 0.2s var(--ease-out);
  }
  .adv-toggle span.open {
    transform: rotate(90deg);
  }
  .adv-toggle:hover {
    color: var(--cobalt);
  }

  .error {
    margin: 16px 0 0;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: oklch(0.96 0.03 25);
    color: oklch(0.45 0.16 25);
    font-size: var(--t-sm);
    line-height: 1.45;
    user-select: text;
    -webkit-user-select: text;
  }

  .actions {
    display: flex;
    align-items: center;
    margin-top: 22px;
  }
  .spacer {
    flex: 1;
  }
  .big {
    height: 40px;
    padding: 0 22px;
    font-size: var(--t-md);
  }
  .danger {
    color: var(--danger);
  }
</style>
