<script lang="ts">
  import { mail } from "./state.svelte";
  import MailFrame from "./MailFrame.svelte";
  import { api, type Envelope, type FullMessage } from "./api";
  import { displayName, initials, listDate, senderHue } from "./format";

  /** I messaggi precedenti della conversazione aperta, chiusi; si aprono con un clic. */
  let { current }: { current: FullMessage } = $props();

  type Item = { env: Envelope; folder: string; mine: boolean };

  let related = $state<Item[]>([]);
  let open = $state<Record<string, FullMessage | "loading" | "error">>({});

  const me = $derived(mail.account?.email.toLowerCase() ?? "");
  const sentPath = $derived(mail.folderByRole("sent")?.path ?? null);

  let items = $derived.by<Item[]>(() => {
    const thread = mail.selectedThread?.messages ?? [];
    const here: Item[] = thread
      .filter((m) => m.uid !== current.uid)
      .map((env) => ({ env, folder: mail.folder, mine: env.from.email.toLowerCase() === me }));
    const seen = new Set([current.messageId, ...thread.map((m) => m.messageId)].filter(Boolean));
    const extra = related.filter((r) => !r.env.messageId || !seen.has(r.env.messageId));
    // Solo quelli venuti prima del messaggio aperto: è lo storico che porta fin qui
    return [...here, ...extra].filter((i) => i.env.date <= current.date).sort((a, b) => a.env.date - b.env.date);
  });

  // Le proprie risposte stanno in Inviata: si cercano per identificativo di conversazione
  $effect(() => {
    const accountId = mail.accountId;
    const sent = sentPath;
    const thread = mail.selectedThread?.messages ?? [];
    related = [];
    open = {};
    if (!accountId || !sent || sent === mail.folder) return;
    const ids = [...new Set([current.messageId ?? "", ...current.references, ...thread.flatMap((m) => [m.messageId ?? "", ...(m.parents ?? [])])])]
      .filter(Boolean)
      .slice(-8);
    if (!ids.length) return;
    let alive = true;
    api
      .threadRelated(accountId, sent, ids)
      .then((list) => {
        if (alive) related = list.map((env) => ({ env, folder: sent, mine: true }));
      })
      .catch(() => {});
    return () => (alive = false);
  });

  const key = (i: Item) => `${i.folder}:${i.env.uid}`;

  async function toggle(i: Item) {
    const k = key(i);
    if (open[k]) {
      delete open[k];
      return;
    }
    open[k] = "loading";
    try {
      open[k] = await api.message(mail.accountId!, i.folder, i.env.uid);
    } catch {
      open[k] = "error";
    }
  }
</script>

{#if items.length}
  <div class="stack" aria-label="Messaggi precedenti della conversazione">
    {#each items as i (key(i))}
      {@const state = open[key(i)]}
      <div class="msg" class:expanded={!!state}>
        <button class="head" onclick={() => toggle(i)} aria-expanded={!!state}>
          <span class="avatar" class:mine={i.mine} style:--h={senderHue(i.env.from.email)}>{i.mine ? "Tu" : initials(i.env.from)}</span>
          <span class="who">{i.mine ? "Tu" : displayName(i.env.from)}</span>
          {#if !state}<span class="snippet">{i.env.preview}</span>{/if}
          <time>{listDate(i.env.date)}</time>
        </button>
        {#if state === "loading"}
          <div class="loading"><span class="skeleton" style="width:70%;height:10px"></span></div>
        {:else if state === "error"}
          <p class="err">Messaggio non più disponibile.</p>
        {:else if state}
          <div class="body">
            <MailFrame html={state.html} text={state.text} allowRemote={false} trimQuotes={true} onblocked={() => {}} onmailto={() => {}} />
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .stack {
    display: grid;
    margin: 0 0 18px;
    border-radius: var(--r-lg);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    overflow: hidden;
    animation: settle var(--dur-slow) var(--ease-ink) both;
    animation-delay: 60ms;
  }
  .msg + .msg {
    border-top: 1px solid var(--rule-soft);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 14px;
    text-align: left;
    transition: background var(--dur-fast);
  }
  .head:hover {
    background: var(--paper-2);
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    font-size: 0.62rem;
    font-weight: 700;
    color: oklch(0.32 0.14 var(--h));
    background: oklch(0.92 0.045 var(--h));
  }
  .avatar.mine {
    color: var(--on-deep);
    background: var(--cobalt);
  }
  .who {
    flex: none;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink);
  }
  .snippet {
    flex: 1;
    min-width: 0;
    font-size: var(--t-sm);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .expanded .head {
    background: var(--paper-2);
  }
  .expanded .who {
    flex: 1;
  }
  time {
    flex: none;
    font-size: var(--t-xs);
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .body {
    padding: 6px 16px 14px 50px;
    animation: settle var(--dur) var(--ease-ink);
  }
  .loading {
    padding: 12px 16px 16px 50px;
  }
  .err {
    margin: 0;
    padding: 8px 16px 14px 50px;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
</style>
