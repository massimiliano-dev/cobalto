<script lang="ts">
  import { onMount, tick } from "svelte";
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { generate, messageAsText, type AiJob } from "./ai";
  import { displayName, listDate, miniMarkdown } from "./format";
  import { folderLabel } from "./folders";
  import type { ChatMsg } from "./api";
  import { sheet } from "./motion";
  import { cal, span } from "./calendar.svelte";

  interface Turn extends ChatMsg {
    /** Numero citato [n] → UID dell'email, fotografato quando è stata fatta la domanda */
    refs?: number[];
    error?: string | null;
  }

  const SUGGESTIONS = [
    "Cosa ho in agenda oggi?",
    "Cosa devo fare oggi?",
    "Riassumi le email non lette",
    "C'è qualcosa di urgente?",
    "Chi aspetta una mia risposta?",
  ];

  let turns = $state<Turn[]>([]);
  let input = $state("");
  let busy = $state(false);
  let job: AiJob | null = null;
  let scroller: HTMLElement | undefined = $state();
  let field: HTMLTextAreaElement | undefined = $state();

  function context(): { text: string; refs: number[] } {
    const list = mail.visible.slice(0, 40);
    const lines = list.map(
      (m, i) =>
        `[${i + 1}] ${m.seen ? "" : "(NON LETTA) "}${listDate(m.date)} · Da: ${displayName(m.from)} <${m.from.email}> · ` +
        `Oggetto: ${m.subject || "(senza oggetto)"}${m.flagged ? " · CONTRASSEGNATA" : ""}${m.preview ? ` · ${m.preview.slice(0, 180)}` : ""}`,
    );
    const today = new Intl.DateTimeFormat("it-IT", { weekday: "long", day: "numeric", month: "long", year: "numeric" }).format(new Date());
    let text =
      `Oggi è ${today}. Account: ${mail.account?.email}. Cartella aperta: ${folderLabel(mail.currentFolder)}` +
      (mail.searchResults ? ` (risultati della ricerca «${mail.query}»)` : "") +
      `.\n\nEmail visibili, dalla più recente:\n${lines.join("\n") || "(nessuna)"}`;
    if (mail.message) {
      text += `\n\nEmail aperta in questo momento:\n${messageAsText(mail.message)}`;
    }
    text += `\n\n${agenda()}`;
    return { text, refs: list.map((m) => m.uid) };
  }

  /** Appuntamenti da oggi ai prossimi 7 giorni, da tutti i calendari collegati. */
  function agenda(): string {
    const dayFmt = new Intl.DateTimeFormat("it-IT", { weekday: "long", day: "numeric", month: "long" });
    const t = new Intl.DateTimeFormat("it-IT", { hour: "2-digit", minute: "2-digit" });
    const startOfToday = new Date().setHours(0, 0, 0, 0);
    const list = cal.upcoming
      .filter((e) => span(e)[1] > startOfToday)
      .sort((a, b) => span(a)[0] - span(b)[0])
      .slice(0, 60)
      .map((e) => {
        const [a, b] = span(e);
        const when = e.allDay ? `${dayFmt.format(a)}, tutto il giorno` : `${dayFmt.format(a)}, ${t.format(a)}–${t.format(b)}`;
        const extra = [e.location, e.organizer && `organizza ${e.organizer}`, e.joinUrl && "videochiamata", e.showAs === "tentative" && "provvisorio"]
          .filter(Boolean)
          .join(" · ");
        return `- ${when}: ${e.title}${extra ? ` (${extra})` : ""}`;
      });
    if (!cal.status.some((s) => s.status === "ok")) return "Calendario: non collegato (serve un account Microsoft o Google).";
    return `Appuntamenti in calendario (oggi e prossimi 7 giorni):\n${list.join("\n") || "(nessuno)"}`;
  }

  function system(ctx: string) {
    const me = mail.account?.name || mail.account?.email || "l'utente";
    const lang = mail.settings?.settings.aiLanguage || "italiano";
    return (
      `Sei l'assistente di posta di ${me} dentro Cobalto. Rispondi in ${lang}, in modo breve e concreto, ` +
      `usando solo le informazioni qui sotto: se qualcosa non c'è, dillo. Quando parli di un'email citala ` +
      `con il suo numero tra parentesi quadre, per esempio [3]. Per domande su impegni e disponibilità usa il calendario qui sotto. ` +
      `Se ti chiedono di scrivere un'email, ` +
      `scrivila con una prima riga "Oggetto: …" e poi il testo, senza firma.\n\n${ctx}`
    );
  }

  async function ask(question: string) {
    const q = question.trim();
    if (!q || busy) return;
    input = "";
    const { text, refs } = context();
    turns.push({ role: "user", content: q });
    const answer: Turn = { role: "assistant", content: "", refs, error: null };
    turns.push(answer);
    const idx = turns.length - 1;
    busy = true;
    scrollDown();
    const history: ChatMsg[] = turns.slice(0, -1).map((t) => ({ role: t.role, content: t.content }));
    job = await generate(
      system(text),
      history,
      (t) => {
        turns[idx].content = t;
        scrollDown();
      },
      (err) => {
        busy = false;
        job = null;
        turns[idx].error = err;
        if (!turns[idx].content && !err) turns.splice(idx - 1, 2);
      },
    );
  }

  async function scrollDown() {
    await tick();
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: "smooth" });
  }

  /** Markdown + citazioni [n] trasformate in pulsanti che aprono l'email. */
  function render(t: Turn) {
    return miniMarkdown(t.content).replace(/\[(\d{1,2})\]/g, (all, n) => {
      const uid = t.refs?.[Number(n) - 1];
      return uid ? `<button class="ref" data-uid="${uid}">${n}</button>` : all;
    });
  }

  function onAnswerClick(e: MouseEvent) {
    const uid = (e.target as HTMLElement).closest<HTMLElement>(".ref")?.dataset.uid;
    if (uid && mail.visible.some((m) => m.uid === Number(uid))) mail.open(Number(uid));
  }

  function useAsDraft(t: Turn) {
    const match = t.content.match(/^\s*Oggetto:\s*(.*)\n+/i);
    mail.startCompose("new", null, match ? t.content.slice(match[0].length) : t.content);
    if (mail.compose && match) mail.compose.subject = match[1].trim();
  }

  function looksLikeEmail(t: Turn) {
    return /^\s*Oggetto:/i.test(t.content);
  }

  onMount(() => {
    field?.focus();
    return () => job?.cancel();
  });
</script>

<aside class="assistant" aria-label="Assistente" out:sheet={{ x: 32, duration: 180 }}>
  <header data-tauri-drag-region>
    <div class="title" data-tauri-drag-region>
      <Icon name="spark" size={16} />
      <h2>Assistente</h2>
      <span class="who">{mail.aiLabel}</span>
    </div>
    {#if turns.length}
      <button class="icon-btn" title="Nuova conversazione" onclick={() => (job?.cancel(), (turns = []))}><Icon name="refresh" size={15} /></button>
    {/if}
    <button class="icon-btn" title="Chiudi (⌘J)" onclick={() => (mail.showAssistant = false)}><Icon name="x" size={16} /></button>
  </header>

  <div class="scroll" bind:this={scroller}>
    {#if !mail.aiReady}
      <div class="empty">
        <p class="empty-title">Collega un assistente</p>
        <p>Scegli Claude, GPT o un modello locale gratuito nelle impostazioni.</p>
        <button class="btn btn-line" onclick={() => (mail.showSettings = true)}>Apri impostazioni</button>
      </div>
    {:else if !turns.length}
      <div class="empty">
        <p class="empty-title">Chiedi alla tua posta.</p>
        <p>Vedo le email di «{folderLabel(mail.currentFolder)}»{mail.message ? " e quella che hai aperto" : ""}.</p>
        <div class="sugg">
          {#each SUGGESTIONS as q, i (q)}
            <button style:--i={i} onclick={() => ask(q)}>{q}</button>
          {/each}
        </div>
      </div>
    {:else}
      {#each turns as t, i (i)}
        {#if t.role === "user"}
          <div class="q">{t.content}</div>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="a" class:streaming={busy && i === turns.length - 1} onclick={onAnswerClick}>
            {#if t.content}
              <div class="md">{@html render(t)}</div>
            {:else if !t.error}
              <div class="writing"><span></span><span></span><span></span></div>
            {/if}
            {#if t.error}<p class="err">{t.error}</p>{/if}
            {#if t.content && !(busy && i === turns.length - 1)}
              <div class="a-actions">
                <button onclick={() => navigator.clipboard.writeText(t.content)}>Copia</button>
                {#if looksLikeEmail(t)}<button onclick={() => useAsDraft(t)}>Usa come bozza</button>{/if}
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    {/if}
  </div>

  {#if mail.aiReady}
    <form
      class="composer"
      onsubmit={(e) => {
        e.preventDefault();
        ask(input);
      }}
    >
      <textarea
        bind:this={field}
        bind:value={input}
        rows="1"
        placeholder="Chiedi qualcosa sulle tue email…"
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            ask(input);
          }
        }}
      ></textarea>
      {#if busy}
        <button type="button" class="go stop" aria-label="Interrompi" onclick={() => job?.cancel()}><Icon name="stop" size={12} /></button>
      {:else}
        <button class="go" disabled={!input.trim()} aria-label="Invia"><Icon name="arrowUp" size={14} /></button>
      {/if}
    </form>
  {/if}
</aside>

<style>
  .assistant {
    position: absolute;
    z-index: 25;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(420px, 42vw);
    display: flex;
    flex-direction: column;
    background: var(--paper);
    box-shadow: -1px 0 0 var(--rule), -24px 0 48px -28px oklch(0.3 0.12 264 / 0.35);
    animation: drawer 0.38s var(--ease-out);
  }
  @keyframes drawer {
    from {
      transform: translateX(32px);
      opacity: 0;
    }
  }

  header {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 38px 12px 10px 20px;
    border-bottom: 1px solid var(--rule-soft);
  }
  :global(.shell:not(.mac)) header {
    padding-top: 14px;
  }
  .title {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--cobalt);
  }
  h2 {
    margin: 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.55rem;
    color: var(--ink);
  }
  .who {
    margin-top: 4px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 18px 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .empty {
    margin-top: 12vh;
    color: var(--ink-3);
    font-size: var(--t-sm);
    line-height: 1.5;
  }
  .empty p {
    margin: 0 0 6px;
  }
  .empty-title {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.9rem;
    line-height: 1.1;
    color: var(--ink-2);
  }
  .empty .btn {
    margin-top: 10px;
  }
  .sugg {
    display: grid;
    gap: 6px;
    margin-top: 18px;
  }
  .sugg button {
    text-align: left;
    padding: 10px 14px;
    border-radius: var(--r-md);
    font-size: var(--t-sm);
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--rule);
    opacity: 0;
    animation: settle 0.45s var(--ease-out) forwards;
    animation-delay: calc(var(--i) * 60ms + 120ms);
    transition: background 0.15s, box-shadow 0.15s, transform 0.2s var(--ease-out);
  }
  .sugg button:hover {
    background: var(--cobalt-mist);
    box-shadow: inset 0 0 0 1px oklch(0.82 0.08 260);
    transform: translateX(3px);
  }

  .q {
    align-self: flex-end;
    max-width: 85%;
    padding: 9px 14px;
    border-radius: 14px 14px 4px 14px;
    background: var(--cobalt);
    color: var(--on-deep);
    font-size: var(--t-sm);
    line-height: 1.45;
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
    animation: settle 0.25s var(--ease-out);
  }
  .a {
    position: relative;
    padding-left: 14px;
    font-size: var(--t-sm);
    animation: settle 0.3s var(--ease-out);
  }
  .a::before {
    content: "";
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    width: 1px;
    background: linear-gradient(var(--cobalt), oklch(0.5 0.215 263 / 0.1));
  }
  .md {
    line-height: 1.55;
    color: var(--ink);
    user-select: text;
    -webkit-user-select: text;
  }
  .md :global(p) {
    margin: 0 0 0.55em;
  }
  .md :global(ul) {
    margin: 0 0 0.6em;
    padding-left: 1.1em;
  }
  .md :global(li) {
    margin-bottom: 0.3em;
  }
  .md :global(li::marker) {
    color: var(--cobalt);
  }
  .md :global(strong) {
    color: var(--cobalt-deep);
  }
  .md :global(.ref) {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    margin: 0 1px;
    border-radius: 5px;
    vertical-align: 1px;
    font-size: 0.66rem;
    font-weight: 700;
    color: var(--cobalt);
    background: var(--cobalt-wash);
    transition: background 0.12s, color 0.12s;
  }
  .md :global(.ref:hover) {
    background: var(--cobalt);
    color: var(--on-deep);
  }
  .streaming .md::after {
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
    margin: 4px 0 0;
    color: var(--danger);
  }
  .a-actions {
    display: flex;
    gap: 14px;
    margin-top: 4px;
  }
  .a-actions button {
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
  }
  .a-actions button:hover {
    color: var(--cobalt);
  }

  .composer {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    margin: 0 14px 14px;
    padding: 8px 8px 8px 14px;
    border-radius: var(--r-lg);
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule), 0 8px 20px -14px oklch(0.3 0.12 264 / 0.4);
    transition: box-shadow 0.15s;
  }
  .composer:focus-within {
    box-shadow: inset 0 0 0 1px var(--cobalt), 0 0 0 3px oklch(0.5 0.215 263 / 0.12);
  }
  textarea {
    flex: 1;
    min-height: 22px;
    max-height: 140px;
    padding: 4px 0;
    border: 0;
    outline: 0;
    resize: none;
    background: none;
    font-size: var(--t-sm);
    line-height: 1.45;
    field-sizing: content;
  }
  .go {
    flex: none;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    background: var(--cobalt);
    color: var(--on-deep);
    transition: transform 0.15s var(--ease-out), background 0.15s;
  }
  .go:not(:disabled):hover {
    transform: translateY(-1px);
  }
  .go:disabled {
    background: var(--paper-3);
    color: var(--ink-3);
  }
  .go.stop {
    background: var(--cobalt-deep);
  }
</style>
