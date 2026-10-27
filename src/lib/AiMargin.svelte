<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { generate, prompts, replyIntents, type AiJob } from "./ai";
  import { miniMarkdown } from "./format";
  import type { FullMessage } from "./api";

  let { message }: { message: FullMessage } = $props();

  let provider = $derived(mail.aiLabel);
  let hasKey = $derived(mail.aiReady);

  let mode = $state<"idle" | "summary" | "ask">("idle");
  let output = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let question = $state("");
  let asked = $state("");
  let job: AiJob | null = null;

  function system() {
    const me = mail.account?.name || mail.account?.email || "l'utente";
    return prompts.system(mail.settings?.settings.aiLanguage || "italiano", me);
  }

  async function run(kind: "summary" | "ask", prompt: string) {
    job?.cancel();
    mode = kind;
    output = "";
    error = null;
    busy = true;
    job = await generate(
      system(),
      prompt,
      (t) => (output = t),
      (err) => {
        busy = false;
        error = err;
      },
    );
  }

  function summarize() {
    run("summary", prompts.summary(message));
  }

  function ask() {
    if (!question.trim()) return;
    asked = question.trim();
    question = "";
    run("ask", prompts.ask(message, asked));
  }

  function quickReply(intent: string) {
    mail.startCompose("reply", message);
    if (mail.compose) mail.compose.aiIntent = intent;
  }

  $effect(() => () => job?.cancel());
</script>

<aside class="margin">
  <div class="mark">
    <Icon name="spark" size={14} />
    <span>{mail.settings?.settings.aiProvider === "local" ? "Nota dell'AI locale" : `Nota di ${provider}`}</span>
  </div>

  {#if !hasKey}
    <p class="hint">
      Collega un assistente (Claude, GPT o un modello locale gratuito) per riassunti, risposte già scritte e domande sull'email.
    </p>
    <button class="btn btn-line" onclick={() => (mail.showSettings = true)}>Collega l'assistente</button>
  {:else}
    {#if mode === "idle"}
      <button class="primary-action" onclick={summarize}>
        <span>Riassumi</span>
        <small>punti chiave e cose da fare</small>
      </button>
    {:else}
      <div class="note" class:busy>
        {#if mode === "ask"}<p class="q">«{asked}»</p>{/if}
        {#if output}
          <div class="md">{@html miniMarkdown(output)}</div>
        {:else if busy}
          <div class="writing"><span></span><span></span><span></span></div>
        {/if}
        {#if error}<p class="err">{error}</p>{/if}
        <div class="note-actions">
          {#if busy}
            <button class="link" onclick={() => job?.cancel()}>Interrompi</button>
          {:else}
            <button class="link" onclick={summarize}>{mode === "summary" ? "Rigenera" : "Riassumi"}</button>
            {#if output}
              <button class="link" onclick={() => navigator.clipboard.writeText(output)}>Copia</button>
            {/if}
          {/if}
        </div>
      </div>
    {/if}

    <div class="block">
      <div class="block-label">Rispondi con un clic</div>
      <div class="chips">
        {#each replyIntents as r (r.label)}
          <button class="chip" onclick={() => quickReply(r.intent)}>{r.label}</button>
        {/each}
        <button class="chip ghost" onclick={() => quickReply("")}>
          <Icon name="spark" size={12} /> Proponi tu
        </button>
      </div>
    </div>

    <form
      class="ask"
      onsubmit={(e) => {
        e.preventDefault();
        ask();
      }}
    >
      <input placeholder="Chiedi qualcosa su questa email…" bind:value={question} />
      <button class="send" disabled={!question.trim()} aria-label="Chiedi"><Icon name="arrowUp" size={14} /></button>
    </form>
  {/if}
</aside>

<style>
  .margin {
    display: grid;
    align-content: start;
    gap: 16px;
    font-size: var(--t-sm);
    color: var(--ink-2);
  }
  .mark {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.05rem;
    color: var(--cobalt);
  }
  .hint {
    margin: 0;
    line-height: 1.5;
  }

  .primary-action {
    display: grid;
    gap: 1px;
    text-align: left;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--cobalt-mist);
    box-shadow: inset 0 0 0 1px var(--cobalt-wash);
    transition: background 0.15s, box-shadow 0.15s;
  }
  .primary-action span {
    font-weight: 650;
    color: var(--cobalt);
  }
  .primary-action small {
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .primary-action:hover {
    background: var(--cobalt-wash);
    box-shadow: inset 0 0 0 1px oklch(0.85 0.07 260);
  }

  /* La nota: inchiostro blu su carta, con un filo a sinistra come appunto a margine */
  .note {
    position: relative;
    padding: 2px 0 2px 14px;
    animation: settle 0.35s var(--ease-out);
  }
  .note::before {
    content: "";
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 1px;
    background: linear-gradient(var(--cobalt), oklch(0.5 0.215 263 / 0.15));
  }
  .q {
    margin: 0 0 8px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1rem;
    color: var(--ink);
  }
  .md {
    line-height: 1.55;
    color: var(--ink);
    user-select: text;
    -webkit-user-select: text;
  }
  .md :global(p) {
    margin: 0 0 0.5em;
  }
  .md :global(ul) {
    margin: 0 0 0.6em;
    padding-left: 1.05em;
  }
  .md :global(li) {
    margin-bottom: 0.3em;
  }
  .md :global(li::marker) {
    color: var(--cobalt);
  }
  .md :global(strong) {
    color: var(--cobalt-deep);
    font-weight: 650;
  }
  .md :global(h4) {
    margin: 0.6em 0 0.3em;
    font-size: var(--t-sm);
  }
  .busy .md::after {
    content: "";
    display: inline-block;
    width: 7px;
    height: 1em;
    margin-left: 2px;
    vertical-align: -2px;
    background: var(--cobalt);
    animation: blink 0.9s steps(2) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .writing {
    display: flex;
    gap: 4px;
    padding: 6px 0;
  }
  .writing span {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--cobalt);
    animation: hop 1s var(--ease-out) infinite;
  }
  .writing span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .writing span:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes hop {
    0%,
    60%,
    100% {
      opacity: 0.25;
      transform: none;
    }
    30% {
      opacity: 1;
      transform: translateY(-3px);
    }
  }
  .err {
    margin: 6px 0 0;
    color: var(--danger);
  }
  .note-actions {
    display: flex;
    gap: 14px;
    margin-top: 6px;
  }
  .link {
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
  }
  .link:hover {
    color: var(--cobalt);
  }

  .block {
    display: grid;
    gap: 8px;
  }
  .block-label {
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink-2);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 11px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--cobalt);
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: background 0.12s, box-shadow 0.12s, transform 0.12s var(--ease-out);
  }
  .chip:hover {
    background: var(--cobalt-mist);
    box-shadow: inset 0 0 0 1px oklch(0.82 0.08 260);
    transform: translateY(-1px);
  }
  .chip.ghost {
    color: var(--ink-2);
  }

  .ask {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 4px 4px 12px;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .ask:focus-within {
    box-shadow: inset 0 0 0 1px var(--cobalt), 0 0 0 3px oklch(0.5 0.215 263 / 0.12);
  }
  .ask input {
    flex: 1;
    min-width: 0;
    height: 28px;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--t-sm);
  }
  .send {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 7px;
    background: var(--cobalt);
    color: var(--on-deep);
  }
  .send:disabled {
    background: var(--paper-3);
    color: var(--ink-3);
  }
</style>
