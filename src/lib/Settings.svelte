<script lang="ts">
  import { onMount } from "svelte";
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorText, type LocalStatus, type PullProgress, type Settings } from "./api";
  import { fileSize } from "./format";
  import { fadeOut, sheet } from "./motion";
  import { SOUND_OPTIONS, getSound, setSound, preview, type SoundKind } from "./sounds";

  let sounds = $state<Record<SoundKind, string>>({ mail: getSound("mail"), meeting: getSound("meeting") });

  function chooseSound(kind: SoundKind, id: string) {
    sounds[kind] = id;
    setSound(kind, id);
    preview(kind, id);
  }

  const FALLBACK: Record<string, string[]> = {
    anthropic: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"],
    openai: [],
    local: [],
  };

  // Scelti provando i modelli su compiti di posta in italiano (vedi README → AI locale)
  const RECOMMENDED = [
    { name: "gemma4:26b", label: "Gemma 4 26B", note: "Consigliato: veloce e ottimo italiano", size: "18 GB" },
    { name: "qwen3.8:27b", label: "Qwen 3.8 27B", note: "Il più preciso, più lento", size: "18 GB" },
    { name: "gemma4:12b", label: "Gemma 4 12B", note: "Leggero, per Mac con 16 GB", size: "8 GB" },
  ];

  let local = $state<LocalStatus | null>(null);
  let pulls = $state<Record<string, PullProgress>>({});

  async function refreshLocal() {
    local = await api.localStatus().catch(() => null);
  }

  async function download(model: string) {
    pulls[model] = { model, status: "avvio", completed: 0, total: 0, done: false, error: null };
    await api.localPull(model);
  }

  onMount(() => {
    const un = listen<PullProgress>("ollama-pull", async (e) => {
      pulls[e.payload.model] = e.payload;
      if (e.payload.done) {
        await refreshLocal();
        if (!e.payload.error && !local?.models.some((m) => m.name === s.localModel)) setModel(e.payload.model);
      }
    });
    refreshLocal();
    return () => {
      un.then((f) => f());
    };
  });

  let s = $state<Settings>({ ...mail.settings!.settings });
  let key = $state("");
  let models = $state<string[]>([]);
  let modelsError = $state<string | null>(null);
  let loadingModels = $state(false);
  let savingKey = $state(false);

  let hasKey = $derived(s.aiProvider === "openai" ? mail.settings?.hasOpenaiKey : mail.settings?.hasAnthropicKey);
  let model = $derived(s.aiProvider === "openai" ? s.openaiModel : s.aiProvider === "local" ? s.localModel : s.anthropicModel);
  let options = $derived(Array.from(new Set([model, ...(models.length ? models : FALLBACK[s.aiProvider])])).filter(Boolean));

  async function persist() {
    try {
      await api.saveSettings($state.snapshot(s));
      if (mail.settings) mail.settings.settings = $state.snapshot(s);
    } catch (e) {
      mail.toast(errorText(e), "error");
    }
  }

  async function loadModels() {
    models = [];
    modelsError = null;
    if (s.aiProvider === "local") {
      await refreshLocal();
      models = local?.models.map((m) => m.name) ?? [];
      return;
    }
    if (!hasKey) return;
    loadingModels = true;
    try {
      models = await api.aiModels(s.aiProvider);
    } catch (e) {
      modelsError = errorText(e);
    } finally {
      loadingModels = false;
    }
  }

  async function setProvider(p: Settings["aiProvider"]) {
    s.aiProvider = p;
    key = "";
    await persist();
    loadModels();
  }

  async function saveKey() {
    savingKey = true;
    try {
      await api.setAiKey(s.aiProvider, key);
      mail.settings = await api.settings();
      key = "";
      await loadModels();
      if (!modelsError) mail.toast("Chiave salvata nel portachiavi");
    } catch (e) {
      mail.toast(errorText(e), "error");
    } finally {
      savingKey = false;
    }
  }

  async function removeKey() {
    await api.setAiKey(s.aiProvider, "");
    mail.settings = await api.settings();
    models = [];
  }

  function setModel(v: string) {
    if (s.aiProvider === "openai") s.openaiModel = v;
    else if (s.aiProvider === "local") s.localModel = v;
    else s.anthropicModel = v;
    persist();
  }

  onMount(loadModels);
</script>

<div class="scrim" role="presentation" onclick={() => (mail.showSettings = false)} out:fadeOut></div>
<aside class="panel" aria-label="Impostazioni" out:sheet={{ x: 40, duration: 180 }}>
  <header>
    <h2>Impostazioni</h2>
    <button class="icon-btn" onclick={() => (mail.showSettings = false)} aria-label="Chiudi"><Icon name="x" /></button>
  </header>

  <section>
    <h3>Assistente</h3>
    <div class="seg" role="radiogroup">
      <button role="radio" aria-checked={s.aiProvider === "anthropic"} class:on={s.aiProvider === "anthropic"} onclick={() => setProvider("anthropic")}>
        <strong>Claude</strong><small>Anthropic</small>
      </button>
      <button role="radio" aria-checked={s.aiProvider === "openai"} class:on={s.aiProvider === "openai"} onclick={() => setProvider("openai")}>
        <strong>GPT</strong><small>OpenAI o compatibile</small>
      </button>
      <button role="radio" aria-checked={s.aiProvider === "local"} class:on={s.aiProvider === "local"} onclick={() => setProvider("local")}>
        <strong>Locale</strong><small>Ollama · gratis, privato</small>
      </button>
    </div>

    {#if s.aiProvider === "local"}
      <div class="local">
        <div class="local-status" class:ok={local?.running}>
          <span class="led"></span>
          {#if local?.running}
            Ollama attivo · {local.models.length} {local.models.length === 1 ? "modello installato" : "modelli installati"}
          {:else}
            Ollama non risponde: apri l'app Ollama
            <button class="link" onclick={refreshLocal}>Riprova</button>
          {/if}
        </div>
        <p class="help">I testi delle email non lasciano mai il tuo Mac e non consumi crediti.</p>
        <div class="recs">
          {#each RECOMMENDED as r (r.name)}
            {@const installed = local?.models.some((m) => m.name === r.name)}
            {@const p = pulls[r.name]}
            <div class="rec" class:active={s.localModel === r.name && installed}>
              <div class="rec-text">
                <strong>{r.label}</strong>
                <small>{r.note} · {r.size}</small>
                {#if p && !p.done}
                  <div class="bar"><span style:transform="scaleX({p.total ? p.completed / p.total : 0})"></span></div>
                  <small class="pull-status">{p.total ? `${fileSize(p.completed)} di ${fileSize(p.total)}` : p.status}</small>
                {:else if p?.error}
                  <small class="err">{p.error}</small>
                {/if}
              </div>
              {#if installed}
                {#if s.localModel === r.name}
                  <span class="in-use"><Icon name="check" size={14} /> In uso</span>
                {:else}
                  <button class="btn btn-line" onclick={() => setModel(r.name)}>Usa</button>
                {/if}
              {:else if !p || p.done}
                <button class="btn btn-line" disabled={!local?.running} onclick={() => download(r.name)}><Icon name="download" size={14} /> Scarica</button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if s.aiProvider !== "local"}
    <label class="field">
      <span>Chiave API</span>
      {#if hasKey && !key}
        <div class="key-ok">
          <Icon name="check" size={15} />
          <span>Salvata nel portachiavi di sistema</span>
          <button class="link" onclick={removeKey}>Rimuovi</button>
        </div>
      {/if}
      <div class="key-row">
        <input
          class="input"
          type="password"
          bind:value={key}
          placeholder={hasKey ? "Sostituisci la chiave…" : s.aiProvider === "openai" ? "sk-…" : "sk-ant-…"}
          spellcheck="false"
        />
        <button class="btn btn-primary" disabled={!key.trim() || savingKey} onclick={saveKey}>Salva</button>
      </div>
      <small class="help">
        {s.aiProvider === "openai" ? "platform.openai.com → API keys" : "console.anthropic.com → API Keys"}. La chiave resta
        sul tuo computer: le richieste partono direttamente dall'app.
      </small>
    </label>
    {/if}

    <label class="field">
      <span>Modello {#if loadingModels}<em class="loading">· carico…</em>{/if}</span>
      <select class="input" value={model} onchange={(e) => setModel((e.target as HTMLSelectElement).value)}>
        {#each options as m (m)}
          <option value={m}>{m}{s.aiProvider === "local" ? ` · ${fileSize(local?.models.find((x) => x.name === m)?.size ?? 0)}` : ""}</option>
        {/each}
      </select>
      {#if modelsError}<small class="err">{modelsError}</small>{/if}
    </label>

    {#if s.aiProvider === "openai"}
      <label class="field">
        <span>Endpoint</span>
        <input class="input" bind:value={s.openaiBaseUrl} onchange={persist} spellcheck="false" />
        <small class="help">Cambialo per Azure OpenAI, Ollama o LM Studio (es. http://localhost:11434/v1).</small>
      </label>
    {/if}

    <label class="field">
      <span>Lingua delle risposte</span>
      <select class="input" bind:value={s.aiLanguage} onchange={persist}>
        <option value="italiano">Italiano</option>
        <option value="inglese">English</option>
        <option value="spagnolo">Español</option>
        <option value="francese">Français</option>
        <option value="tedesco">Deutsch</option>
      </select>
    </label>
  </section>

  <section>
    <h3>Notifiche</h3>
    {#each [{ kind: "mail", label: "Nuova posta" }, { kind: "meeting", label: "Riunioni in arrivo" }] as row (row.kind)}
      {@const kind = row.kind as SoundKind}
      <div class="sound-row">
        <span class="sound-label">{row.label}</span>
        <select class="input" value={sounds[kind]} onchange={(e) => chooseSound(kind, (e.target as HTMLSelectElement).value)}>
          {#each SOUND_OPTIONS as o (o.id)}
            <option value={o.id}>{o.label}</option>
          {/each}
        </select>
        <button class="btn btn-line" disabled={sounds[kind] === "none"} onclick={() => preview(kind, sounds[kind])}>Prova</button>
      </div>
    {/each}
    <small class="help">Le riunioni suonano 5 minuti prima, anche con Cobalto aperto. La nuova posta suona solo quando Cobalto non è in primo piano.</small>
  </section>

  <section>
    <h3>Account</h3>
    {#each mail.accounts as a (a.id)}
      <button
        class="acc"
        onclick={() => {
          mail.editAccount = a;
          mail.showSettings = false;
          mail.showSetup = true;
        }}
      >
        <span class="acc-text"><strong>{a.name}</strong><small>{a.email}</small></span>
        <Icon name="chevron" size={14} />
      </button>
    {/each}
    <button
      class="btn btn-line"
      onclick={() => {
        mail.editAccount = null;
        mail.showSettings = false;
        mail.showSetup = true;
      }}><Icon name="plus" size={14} /> Aggiungi account</button
    >
  </section>

  <section>
    <h3>Scorciatoie</h3>
    <dl class="keys">
      <dt><kbd>J</kbd><kbd>K</kbd></dt><dd>Messaggio successivo / precedente</dd>
      <dt><kbd>R</kbd></dt><dd>Rispondi</dd>
      <dt><kbd>A</kbd></dt><dd>Rispondi a tutti</dd>
      <dt><kbd>F</kbd></dt><dd>Inoltra</dd>
      <dt><kbd>E</kbd></dt><dd>Archivia</dd>
      <dt><kbd>#</kbd></dt><dd>Elimina</dd>
      <dt><kbd>S</kbd></dt><dd>Contrassegna</dd>
      <dt><kbd>U</kbd></dt><dd>Segna come da leggere</dd>
      <dt><kbd>C</kbd></dt><dd>Nuovo messaggio</dd>
      <dt><kbd>/</kbd></dt><dd>Cerca</dd>
      <dt><kbd>⌘</kbd><kbd>K</kbd></dt><dd>Palette dei comandi</dd>
      <dt><kbd>⌘</kbd><kbd>J</kbd></dt><dd>Assistente</dd>
      <dt><kbd>⌘</kbd><kbd>↵</kbd></dt><dd>Invia</dd>
    </dl>
  </section>
</aside>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 60;
    background: oklch(0.25 0.1 265 / 0.22);
    animation: fade 0.2s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .panel {
    position: absolute;
    z-index: 61;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(440px, 100vw);
    overflow-y: auto;
    padding: 0 26px 32px;
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    animation: slide 0.32s var(--ease-out);
  }
  @keyframes slide {
    from {
      transform: translateX(40px);
      opacity: 0;
    }
  }
  header {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 40px 0 12px;
    background: linear-gradient(var(--paper) 80%, transparent);
  }
  h2 {
    margin: 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 2rem;
  }
  section {
    display: grid;
    gap: 16px;
    padding: 18px 0 26px;
    border-bottom: 1px solid var(--rule-soft);
  }
  h3 {
    margin: 0;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--cobalt);
  }

  .seg {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .seg button {
    display: grid;
    gap: 2px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    text-align: left;
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: background 0.15s, box-shadow 0.15s;
  }
  .seg button:hover {
    background: var(--cobalt-mist);
  }
  .seg button.on {
    background: var(--cobalt-mist);
    box-shadow: inset 0 0 0 2px var(--cobalt);
  }
  .seg strong {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.35rem;
    line-height: 1;
    color: var(--ink);
  }
  .seg small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }

  .local {
    display: grid;
    gap: 10px;
  }
  .local-status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--t-sm);
    color: var(--ink-2);
  }
  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ink-3);
  }
  .local-status.ok .led {
    background: var(--cobalt);
    box-shadow: 0 0 0 3px oklch(0.5 0.215 263 / 0.18);
  }
  .local .help {
    margin: 0;
  }
  .recs {
    display: grid;
    gap: 6px;
  }
  .rec {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    transition: box-shadow 0.2s, background 0.2s;
  }
  .rec.active {
    background: var(--cobalt-mist);
    box-shadow: inset 0 0 0 1px oklch(0.82 0.08 260);
  }
  .rec-text {
    flex: 1;
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .rec-text strong {
    font-size: var(--t-md);
    font-weight: 600;
  }
  .rec-text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .bar {
    height: 4px;
    margin-top: 6px;
    border-radius: 4px;
    background: var(--paper-3);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--cobalt);
    transform-origin: left;
    transition: transform 0.3s var(--ease-out);
  }
  .in-use {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    font-weight: 650;
    color: var(--cobalt);
  }
  .rec .btn {
    height: 30px;
  }

  .key-ok {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-sm);
    color: var(--cobalt);
  }
  .key-ok span {
    flex: 1;
  }
  .key-row {
    display: flex;
    gap: 8px;
  }
  .key-row .btn {
    height: 38px;
  }
  .help {
    font-size: var(--t-xs);
    line-height: 1.45;
    color: var(--ink-3);
  }
  .err {
    font-size: var(--t-xs);
    color: var(--danger);
  }
  .loading {
    font-style: normal;
    text-transform: none;
    letter-spacing: 0;
  }
  select.input {
    appearance: none;
    background-image: linear-gradient(45deg, transparent 50%, var(--ink-3) 50%), linear-gradient(135deg, var(--ink-3) 50%, transparent 50%);
    background-position:
      calc(100% - 17px) 16px,
      calc(100% - 12px) 16px;
    background-size: 5px 5px;
    background-repeat: no-repeat;
    padding-right: 32px;
  }
  .link {
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
  }
  .link:hover {
    color: var(--danger);
  }

  .sound-row {
    display: grid;
    grid-template-columns: 1fr 160px auto;
    align-items: center;
    gap: 8px;
  }
  .sound-label {
    font-size: var(--t-md);
    color: var(--ink);
  }
  .sound-row .btn {
    height: 38px;
  }
  .acc {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    text-align: left;
    color: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .acc:hover {
    background: var(--cobalt-mist);
    color: var(--cobalt);
  }
  .acc-text {
    flex: 1;
    display: grid;
  }
  .acc-text strong {
    color: var(--ink);
    font-weight: 600;
    font-size: var(--t-md);
  }
  .acc-text small {
    font-size: var(--t-xs);
  }
  section > .btn {
    justify-self: start;
  }

  .keys {
    display: grid;
    grid-template-columns: 72px 1fr;
    gap: 9px 12px;
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink-2);
  }
  .keys dt {
    display: flex;
    gap: 3px;
  }
  .keys dd {
    margin: 0;
  }
</style>
