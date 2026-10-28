<script lang="ts">
  import { api } from "./api";
  import { parseAddress, suggestLocal, type Suggestion } from "./contacts";

  let {
    value = $bindable(""),
    accountId,
    placeholder = "",
    label,
  }: { value: string; accountId: string; placeholder?: string; label: string } = $props();

  // Separa "a, b" rispettando le virgole tra virgolette ("Rossi, Mario" <m@x.it>)
  const split = (s: string) =>
    s
      .split(/[,;]\s*(?=(?:[^"]*"[^"]*")*[^"]*$)/)
      .map((x) => x.trim())
      .filter(Boolean);

  let chips = $state<string[]>(split(value));
  let text = $state("");
  let input: HTMLInputElement | undefined = $state();
  let open = $state(false);
  let active = $state(0);
  let remote = $state<Suggestion[]>([]);
  let timer: ReturnType<typeof setTimeout>;
  let token = 0;

  // Il valore esterno resta una stringa: il resto del composer (e l'invio) non cambia
  $effect(() => {
    value = [...chips, text.trim()].filter(Boolean).join(", ");
  });

  let chosen = $derived(new Set(chips.map((c) => parseAddress(c).email.toLowerCase())));
  let suggestions = $derived.by(() => {
    if (!text.trim()) return [];
    const local = suggestLocal(accountId, text, chosen);
    const seen = new Set(local.map((s) => s.email.toLowerCase()));
    const extra = remote.filter((r) => !seen.has(r.email.toLowerCase()) && !chosen.has(r.email.toLowerCase()));
    return [...local, ...extra].slice(0, 8);
  });

  export function focus() {
    input?.focus();
  }

  function format(s: Suggestion) {
    return s.name && s.name !== s.email ? `${s.name.includes(",") ? `"${s.name}"` : s.name} <${s.email}>` : s.email;
  }

  function pick(s: Suggestion) {
    chips.push(format(s));
    text = "";
    remote = [];
    open = false;
    input?.focus();
  }

  function commitText() {
    const parts = split(text);
    if (parts.length) chips.push(...parts);
    text = "";
  }

  function onInput() {
    // Incollare o digitare una virgola chiude l'indirizzo
    if (/[,;]/.test(text)) {
      const parts = split(text);
      const last = /[,;]\s*$/.test(text) ? "" : (parts.pop() ?? "");
      chips.push(...parts);
      text = last;
    }
    open = true;
    active = 0;
    clearTimeout(timer);
    const q = text.trim();
    if (q.length < 2) {
      remote = [];
      return;
    }
    const mine = ++token;
    timer = setTimeout(async () => {
      const found = await api.contactsSearch(accountId, q).catch(() => []);
      if (mine === token) remote = found;
    }, 220);
  }

  function onKey(e: KeyboardEvent) {
    if (open && suggestions.length && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      active = (active + (e.key === "ArrowDown" ? 1 : -1) + suggestions.length) % suggestions.length;
    } else if ((e.key === "Enter" || e.key === "Tab") && open && suggestions.length) {
      e.preventDefault();
      pick(suggestions[active]);
    } else if (e.key === "Enter" && text.trim()) {
      e.preventDefault();
      commitText();
    } else if (e.key === "Backspace" && !text && chips.length) {
      // Riporta l'ultimo indirizzo nel campo, per correggerlo
      text = chips.pop()!;
      e.preventDefault();
    } else if (e.key === "Escape" && open) {
      e.stopPropagation();
      open = false;
    }
  }

  function chipLabel(c: string) {
    const a = parseAddress(c);
    return a.name || a.email;
  }

  function mark(s: string) {
    const q = text.trim();
    const i = s.toLowerCase().indexOf(q.toLowerCase());
    if (!q || i < 0) return [s, "", ""];
    return [s.slice(0, i), s.slice(i, i + q.length), s.slice(i + q.length)];
  }

  const SOURCE: Record<string, string> = { recenti: "recenti", rubrica: "rubrica", directory: "organizzazione", google: "contatti Google" };
</script>

<div class="recipients">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="rfield" onclick={() => input?.focus()}>
    {#each chips as c, i (i + c)}
      <span class="chip" class:bad={!parseAddress(c).email.includes("@")} title={c}>
        {chipLabel(c)}
        <button type="button" aria-label="Rimuovi {chipLabel(c)}" onclick={(e) => (e.stopPropagation(), chips.splice(i, 1))}>×</button>
      </span>
    {/each}
    <input
      bind:this={input}
      bind:value={text}
      aria-label={label}
      placeholder={chips.length ? "" : placeholder}
      autocomplete="off"
      spellcheck="false"
      oninput={onInput}
      onkeydown={onKey}
      onfocus={() => (open = true)}
      onblur={() => setTimeout(() => {
        open = false;
        if (text.includes("@")) commitText();
      }, 120)}
    />
  </div>

  {#if open && suggestions.length}
    <ul class="menu" role="listbox">
      {#each suggestions as s, i (s.email)}
        {@const [a, b, c] = mark(s.name || s.email)}
        <li role="option" aria-selected={i === active}>
          <button type="button" class:active={i === active} onmouseenter={() => (active = i)} onmousedown={(e) => (e.preventDefault(), pick(s))}>
            <span class="who">{a}<mark>{b}</mark>{c}</span>
            {#if s.name}<span class="addr">{s.email}</span>{/if}
            <span class="src">{SOURCE[s.source] ?? s.source}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .recipients {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .rfield {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    min-height: 40px;
    padding: 4px 0;
    cursor: text;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 26px;
    max-width: 260px;
    padding: 0 4px 0 10px;
    border-radius: 999px;
    background: var(--cobalt-mist);
    color: var(--cobalt-deep);
    font-size: var(--t-sm);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    animation: settle var(--dur-fast) var(--ease-ink);
  }
  .chip.bad {
    background: transparent;
    box-shadow: inset 0 0 0 1px oklch(0.75 0.08 260);
    border: 1px dashed transparent;
  }
  .chip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    font-size: 0.85rem;
    line-height: 1;
    color: var(--ink-3);
  }
  .chip button:hover {
    background: var(--cobalt-wash);
    color: var(--cobalt);
  }
  input {
    flex: 1;
    min-width: 120px;
    height: 28px;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--t-md);
  }

  .menu {
    position: absolute;
    z-index: 40;
    top: 100%;
    left: -8px;
    right: 0;
    margin: 2px 0 0;
    padding: 5px;
    list-style: none;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: var(--shadow-sheet), 0 0 0 1px var(--rule);
    animation: settle var(--dur-fast) var(--ease-ink);
  }
  .menu button {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    text-align: left;
    font-size: var(--t-sm);
  }
  .menu button.active {
    background: var(--cobalt-mist);
  }
  .who {
    font-weight: 600;
    color: var(--ink);
    white-space: nowrap;
  }
  mark {
    background: none;
    color: var(--cobalt);
  }
  .addr {
    flex: 1;
    min-width: 0;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .src {
    margin-left: auto;
    flex: none;
    font-size: 0.66rem;
    font-weight: 650;
    color: var(--ink-3);
  }
</style>
