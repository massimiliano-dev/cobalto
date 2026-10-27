<script lang="ts">
  import { onMount, tick } from "svelte";
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { generate, prompts, rewriteModes, type AiJob } from "./ai";
  import { fileSize } from "./format";
  import { signatureHtml } from "./signature";
  import { sheet } from "./motion";

  const c = mail.compose!;
  // Gli account Microsoft condividono i file grandi via OneDrive, gli altri hanno il limite classico
  const viaOneDrive = mail.account?.auth === "oauth-microsoft";
  const MAX_ATTACH = (viaOneDrive ? 100 : 25) * 1024 * 1024;
  const ONEDRIVE_FROM = 2.8 * 1024 * 1024;

  let showCc = $state(!!c.cc || !!c.bcc);
  let showQuote = $state(false);
  let wide = $state(false);
  let instruction = $state("");
  let aiBusy = $state(false);
  let aiError = $state<string | null>(null);
  let undoBody = $state<string | null>(null);
  let job: AiJob | null = null;
  let bodyEl: HTMLTextAreaElement | undefined = $state();
  let toEl: HTMLInputElement | undefined = $state();
  let fileInput: HTMLInputElement | undefined = $state();

  let title = $derived(
    { new: "Nuovo messaggio", reply: "Risposta", replyAll: "Risposta a tutti", forward: "Inoltro" }[c.mode],
  );
  let attachSize = $derived(c.attachments.reduce((n, a) => n + (a.data.length * 3) / 4, 0));
  let provider = $derived(mail.aiLabel);

  function me() {
    return mail.account?.name || mail.account?.email || "";
  }
  function system() {
    return prompts.system(mail.settings?.settings.aiLanguage || "italiano", me());
  }

  async function runAi(prompt: string, apply: (text: string) => void) {
    job?.cancel();
    aiBusy = true;
    aiError = null;
    undoBody = c.body;
    job = await generate(system(), prompt, apply, (err) => {
      aiBusy = false;
      aiError = err;
      job = null;
    });
  }

  function writeWithAi() {
    const intent = instruction.trim();
    if (c.body.trim() && intent) {
      // C'è già del testo: l'istruzione diventa una riscrittura libera
      runAi(prompts.rewrite(c.body, intent), (t) => (c.body = t));
    } else if (c.source && c.mode !== "forward") {
      runAi(prompts.reply(c.source, intent, me(), mail.account?.signature ?? ""), (t) => (c.body = t));
    } else {
      if (!intent) return;
      runAi(prompts.compose(intent, c.to, c.subject, me()), (t) => {
        const match = t.match(/^\s*Oggetto:\s*(.*)\n?/i);
        if (match && (!c.subject || c.subject === undoSubject)) {
          c.subject = match[1].trim();
          undoSubject = c.subject;
          c.body = t.slice(match[0].length).replace(/^\s*\n/, "");
        } else if (!match) {
          c.body = t;
        }
      });
    }
    instruction = "";
  }
  let undoSubject = "";

  function rewrite(how: string) {
    if (!c.body.trim()) return;
    runAi(prompts.rewrite(c.body, how), (t) => (c.body = t));
  }

  async function addFiles(list: FileList | null) {
    if (!list) return;
    for (const file of Array.from(list)) {
      if (attachSize + file.size > MAX_ATTACH) {
        mail.toast(`Gli allegati superano ${MAX_ATTACH / 1024 / 1024} MB`, "error");
        break;
      }
      const data = await new Promise<string>((resolve, reject) => {
        const r = new FileReader();
        r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
        r.onerror = () => reject(r.error);
        r.readAsDataURL(file);
      });
      c.attachments.push({ name: file.name, mime: file.type || "application/octet-stream", data });
    }
  }

  function discard() {
    job?.cancel();
    mail.compose = null;
  }

  // Textarea che cresce con il contenuto
  $effect(() => {
    void c.body;
    if (!bodyEl) return;
    bodyEl.style.height = "auto";
    bodyEl.style.height = Math.max(180, bodyEl.scrollHeight) + "px";
  });

  onMount(() => {
    tick().then(() => {
      if (c.aiIntent !== null && c.source) {
        runAi(prompts.reply(c.source, c.aiIntent, me(), mail.account?.signature ?? ""), (t) => (c.body = t));
        c.aiIntent = null;
      } else if (!c.to) toEl?.focus();
      else bodyEl?.focus();
    });
    return () => job?.cancel();
  });
</script>

<div class="composer" class:wide role="dialog" aria-label={title} out:sheet|global={{ y: 24, scale: 0.985, duration: 180 }}>
  <header>
    <span class="title">{title}</span>
    <button class="icon-btn" title={wide ? "Riduci" : "Espandi"} onclick={() => (wide = !wide)}>
      <Icon name={wide ? "down" : "arrowUp"} size={15} />
    </button>
    <button class="icon-btn" title="Scarta" onclick={discard}><Icon name="x" size={16} /></button>
  </header>

  <div class="fields">
    <label class="row">
      <span>A</span>
      <input bind:this={toEl} bind:value={c.to} placeholder="nome@esempio.it" autocomplete="off" spellcheck="false" />
      {#if !showCc}<button class="cc-toggle" onclick={() => (showCc = true)}>Cc Ccn</button>{/if}
    </label>
    {#if showCc}
      <label class="row"><span>Cc</span><input bind:value={c.cc} autocomplete="off" spellcheck="false" /></label>
      <label class="row"><span>Ccn</span><input bind:value={c.bcc} autocomplete="off" spellcheck="false" /></label>
    {/if}
    <label class="row"><span>Oggetto</span><input class="subj" bind:value={c.subject} /></label>
  </div>

  <div class="body-wrap">
    <textarea
      bind:this={bodyEl}
      bind:value={c.body}
      class:streaming={aiBusy}
      placeholder={c.source ? "Scrivi la risposta, oppure lascia fare all'AI qui sotto…" : "Scrivi il messaggio…"}
      readonly={aiBusy}
    ></textarea>

    {#if mail.account?.signature}
      <div class="signature">{@html signatureHtml(mail.account.signature)}</div>
    {/if}

    {#if c.quoted}
      <button class="quote-toggle" onclick={() => (showQuote = !showQuote)}>
        {showQuote ? "Nascondi" : "Mostra"} il messaggio originale
      </button>
      {#if showQuote}
        <pre class="quoted">{c.quoted.header}

{c.quoted.text}</pre>
      {/if}
    {/if}

    {#if c.forwardOf && c.source?.attachments.some((a) => !a.inline)}
      <div class="atts">
        {#each c.source.attachments.filter((a) => !a.inline) as a (a.index)}
          <span class="att fwd" title="Allegato del messaggio originale">
            <Icon name="forward" size={13} />
            <span class="att-name">{a.name}</span>
            <small>{fileSize(a.size)}</small>
          </span>
        {/each}
      </div>
    {/if}

    {#if c.attachments.length}
      {#if viaOneDrive && attachSize > ONEDRIVE_FROM}
        <p class="cloud-note">I file più grandi verranno caricati su OneDrive e inviati come link, visibili ai destinatari.</p>
      {/if}
      <div class="atts">
        {#each c.attachments as a, i (i)}
          <span class="att">
            <Icon name="clip" size={13} />
            <span class="att-name">{a.name}</span>
            <small>{fileSize((a.data.length * 3) / 4)}</small>
            <button aria-label="Rimuovi" onclick={() => c.attachments.splice(i, 1)}><Icon name="x" size={12} /></button>
          </span>
        {/each}
      </div>
    {/if}
  </div>

  <div class="ai">
    {#if c.body.trim() && !aiBusy}
      <div class="tones">
        {#each rewriteModes as r (r.label)}
          <button class="tone" onclick={() => rewrite(r.how)}>{r.label}</button>
        {/each}
        {#if undoBody !== null && undoBody !== c.body}
          <button
            class="tone undo"
            onclick={() => {
              c.body = undoBody ?? "";
              undoBody = null;
            }}>Ripristina</button
          >
        {/if}
      </div>
    {/if}
    <form
      class="ai-input"
      onsubmit={(e) => {
        e.preventDefault();
        writeWithAi();
      }}
    >
      <Icon name="spark" size={15} />
      <input
        bind:value={instruction}
        disabled={aiBusy}
        placeholder={aiBusy
          ? `${provider} sta scrivendo…`
          : c.body.trim()
            ? `Chiedi a ${provider} di modificare il testo…`
            : c.source
              ? `Cosa vuoi rispondere? (vuoto = proposta di ${provider})`
              : `Descrivi l'email e ${provider} la scrive…`}
      />
      {#if aiBusy}
        <button type="button" class="ai-go stop" onclick={() => job?.cancel()} aria-label="Interrompi"><Icon name="stop" size={12} /></button>
      {:else}
        <button class="ai-go" disabled={!instruction.trim() && !(c.source && c.mode !== "forward")} aria-label="Genera">
          <Icon name="arrowUp" size={14} />
        </button>
      {/if}
    </form>
    {#if aiError}<p class="ai-err">{aiError}</p>{/if}
  </div>

  <footer>
    <button class="btn btn-primary send" disabled={c.sending || aiBusy} onclick={() => mail.send()}>
      {#if c.sending}Invio…{:else}<Icon name="send" size={15} /> Invia{/if}
    </button>
    <span class="hint"><kbd>⌘</kbd><kbd>↵</kbd></span>
    <span class="spacer"></span>
    <input bind:this={fileInput} type="file" multiple hidden onchange={(e) => addFiles((e.target as HTMLInputElement).files)} />
    <button class="icon-btn" title="Allega file" onclick={() => fileInput?.click()}><Icon name="clip" /></button>
    <button class="icon-btn" title="Scarta" onclick={discard}><Icon name="trash" /></button>
  </footer>
</div>

<style>
  .composer {
    position: absolute;
    z-index: 30;
    right: 24px;
    bottom: 20px;
    width: min(640px, calc(100vw - 640px));
    min-width: 440px;
    max-height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    border-radius: var(--r-lg);
    background: var(--paper);
    box-shadow: var(--shadow-sheet), 0 0 0 1px var(--rule);
    animation: rise 0.32s var(--ease-out);
    overflow: hidden;
  }
  .composer.wide {
    left: calc(236px + 380px + 24px);
    width: auto;
    top: 56px;
    max-height: none;
  }
  @media (max-width: 1100px) {
    .composer.wide {
      left: calc(210px + 330px + 16px);
    }
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(24px) scale(0.985);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  header {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 8px 8px 6px 18px;
    background: var(--cobalt-deep);
    color: var(--on-deep);
  }
  .title {
    flex: 1;
    font-family: var(--font-serif);
    font-size: 1.15rem;
  }
  header .icon-btn {
    width: 28px;
    height: 28px;
    color: var(--on-deep-2);
  }
  header .icon-btn:hover {
    background: oklch(1 0 0 / 0.1);
    color: var(--on-deep);
  }

  .fields {
    padding: 2px 18px 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .row span {
    width: 56px;
    flex: none;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .row input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--t-md);
  }
  .row .subj {
    font-weight: 600;
  }
  .cc-toggle {
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
  }
  .cc-toggle:hover {
    color: var(--cobalt);
  }

  .body-wrap {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 18px 4px;
  }
  textarea {
    display: block;
    width: 100%;
    min-height: 180px;
    border: 0;
    outline: 0;
    resize: none;
    background: none;
    font-size: 0.95rem;
    line-height: 1.6;
    color: var(--ink);
    overflow: hidden;
  }
  textarea.streaming {
    caret-color: transparent;
    color: var(--cobalt-deep);
  }
  textarea::placeholder {
    color: var(--ink-3);
  }
  .signature {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px dashed var(--rule);
    font-size: var(--t-sm);
    line-height: 1.45;
    color: var(--ink-2);
    pointer-events: none;
  }
  .signature :global(img) {
    max-width: 100%;
    height: auto;
    vertical-align: middle;
  }
  .signature :global(a) {
    color: var(--cobalt);
  }
  .signature :global(p) {
    margin: 0;
  }
  .quote-toggle {
    margin-top: 10px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
  }
  .quote-toggle:hover {
    color: var(--cobalt);
  }
  .quoted {
    margin: 8px 0 0;
    padding-left: 12px;
    border-left: 2px solid var(--cobalt-wash);
    white-space: pre-wrap;
    font-family: inherit;
    font-size: var(--t-sm);
    color: var(--ink-3);
    max-height: 240px;
    overflow-y: auto;
  }
  .atts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .att {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 6px 0 9px;
    border-radius: 7px;
    background: var(--cobalt-mist);
    color: var(--cobalt);
    font-size: var(--t-xs);
  }
  .att-name {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
    font-weight: 550;
  }
  .att small {
    color: var(--ink-3);
  }
  .cloud-note {
    margin: 12px 0 0;
    font-size: var(--t-xs);
    color: var(--cobalt);
  }
  .att.fwd {
    padding-right: 10px;
    background: var(--paper-2);
  }
  .att button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    color: var(--ink-3);
  }
  .att button:hover {
    background: var(--cobalt-wash);
    color: var(--cobalt);
  }

  .ai {
    padding: 8px 14px 4px;
    border-top: 1px solid var(--rule-soft);
    background: linear-gradient(var(--paper), var(--cobalt-mist));
  }
  .tones {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 8px;
  }
  .tone {
    height: 24px;
    padding: 0 9px;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--cobalt);
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .tone:hover {
    box-shadow: inset 0 0 0 1px oklch(0.8 0.08 260);
    background: var(--cobalt-mist);
  }
  .tone.undo {
    color: var(--ink-2);
  }
  .ai-input {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 4px 0 11px;
    border-radius: var(--r-md);
    background: var(--paper);
    color: var(--cobalt);
    box-shadow: inset 0 0 0 1px oklch(0.86 0.06 260);
  }
  .ai-input:focus-within {
    box-shadow: inset 0 0 0 1px var(--cobalt), 0 0 0 3px oklch(0.5 0.215 263 / 0.12);
  }
  .ai-input input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--t-sm);
    color: var(--ink);
  }
  .ai-input input::placeholder {
    color: var(--ink-3);
  }
  .ai-go {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--cobalt);
    color: var(--on-deep);
  }
  .ai-go:disabled {
    background: var(--paper-3);
    color: var(--ink-3);
  }
  .ai-go.stop {
    background: var(--cobalt-deep);
  }
  .ai-err {
    margin: 6px 2px 0;
    font-size: var(--t-xs);
    color: var(--danger);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 12px 12px 14px;
    background: var(--cobalt-mist);
  }
  .send {
    height: 36px;
    padding: 0 18px;
  }
  .hint {
    display: inline-flex;
    gap: 2px;
    margin-left: 4px;
  }
  .spacer {
    flex: 1;
  }
</style>
