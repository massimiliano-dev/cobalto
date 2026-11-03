<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { textToHtml } from "./format";
  import { sanitizeSignature } from "./signature";

  let {
    text = $bindable(""),
    html = $bindable(""),
    streaming = false,
    placeholder = "",
  }: { text: string; html: string; streaming?: boolean; placeholder?: string } = $props();

  // Caratteri che i client di posta (Outlook, Gmail, Apple Mail) mostrano allo stesso modo
  const FONTS = [
    { label: "Predefinito", value: "" },
    { label: "Arial", value: "Arial, Helvetica, sans-serif" },
    { label: "Verdana", value: "Verdana, Geneva, sans-serif" },
    { label: "Trebuchet", value: "'Trebuchet MS', sans-serif" },
    { label: "Georgia", value: "Georgia, serif" },
    { label: "Times", value: "'Times New Roman', Times, serif" },
    { label: "Courier", value: "'Courier New', Courier, monospace" },
  ];
  const SIZES = [
    { label: "Piccolo", value: "2" },
    { label: "Normale", value: "3" },
    { label: "Grande", value: "5" },
    { label: "Titolo", value: "6" },
  ];
  // Il contenuto delle email ha i colori che sceglie chi scrive: qui non vale la palette blu
  const COLORS = ["#1f2937", "#6b7280", "#dc2626", "#ea580c", "#16a34a", "#2563eb", "#7c3aed", "#db2777"];
  const HIGHLIGHTS = ["#fef08a", "#bbf7d0", "#bae6fd", "#fbcfe8", "#fed7aa"];

  let el: HTMLDivElement | undefined = $state();
  let menu = $state<"color" | "highlight" | null>(null);

  // I menu a tendina rubano il fuoco: si ricorda l'ultima selezione dentro l'editor
  let saved: Range | null = null;

  onMount(() => {
    if (el) el.innerHTML = html || textToHtml(text);
    sync();
    const remember = () => {
      const sel = window.getSelection();
      if (sel?.rangeCount && el?.contains(sel.anchorNode)) saved = sel.getRangeAt(0).cloneRange();
    };
    document.addEventListener("selectionchange", remember);
    return () => document.removeEventListener("selectionchange", remember);
  });

  function sync() {
    if (!el) return;
    const empty = !el.textContent?.trim() && !el.querySelector("img");
    html = empty ? "" : el.innerHTML;
    text = empty ? "" : el.innerText;
  }

  /** Testo scritto dall'AI: sostituisce il contenuto (la formattazione riparte da zero). */
  export function setText(t: string) {
    text = t;
    html = textToHtml(t);
    if (el) el.innerHTML = html;
  }

  export function setHtml(h: string) {
    if (el) el.innerHTML = h;
    sync();
  }

  export function focus() {
    el?.focus();
  }

  function exec(cmd: string, value?: string) {
    el?.focus();
    const sel = window.getSelection();
    if (saved && sel) {
      sel.removeAllRanges();
      sel.addRange(saved);
    }
    // Stili in linea (span style="…"): sono quelli che i client di posta rispettano
    document.execCommand("styleWithCSS", false, "true");
    document.execCommand(cmd, false, value);
    menu = null;
    sync();
  }

  function onPaste(e: ClipboardEvent) {
    const pasted = e.clipboardData?.getData("text/html");
    if (!pasted) return;
    e.preventDefault();
    document.execCommand("insertHTML", false, sanitizeSignature(pasted));
    sync();
  }

  // Un clic sui comandi non deve togliere la selezione dal testo
  const keep = (e: MouseEvent) => e.preventDefault();
</script>

<div class="toolbar" role="toolbar" aria-label="Formattazione">
  <select title="Carattere" onchange={(e) => exec("fontName", (e.target as HTMLSelectElement).value || "-apple-system, 'Segoe UI', sans-serif")}>
    {#each FONTS as f (f.label)}
      <option value={f.value} style:font-family={f.value || null}>{f.label}</option>
    {/each}
  </select>
  <select title="Dimensione" onchange={(e) => exec("fontSize", (e.target as HTMLSelectElement).value)}>
    {#each SIZES as s (s.value)}
      <option value={s.value} selected={s.value === "3"}>{s.label}</option>
    {/each}
  </select>
  <span class="sep"></span>
  <button type="button" title="Grassetto (⌘B)" onmousedown={keep} onclick={() => exec("bold")}><b>G</b></button>
  <button type="button" title="Corsivo (⌘I)" onmousedown={keep} onclick={() => exec("italic")}><i>C</i></button>
  <button type="button" title="Sottolineato (⌘U)" onmousedown={keep} onclick={() => exec("underline")}><u>S</u></button>
  <span class="sep"></span>
  <div class="pop">
    <button type="button" title="Colore del testo" onmousedown={keep} onclick={() => (menu = menu === "color" ? null : "color")}>
      <span class="glyph-a">A</span><span class="bar" style:background="#dc2626"></span>
    </button>
    {#if menu === "color"}
      <div class="swatches">
        {#each COLORS as col (col)}
          <button type="button" class="sw" style:background={col} aria-label="Colore {col}" onmousedown={keep} onclick={() => exec("foreColor", col)}></button>
        {/each}
      </div>
    {/if}
  </div>
  <div class="pop">
    <button type="button" title="Evidenzia" onmousedown={keep} onclick={() => (menu = menu === "highlight" ? null : "highlight")}>
      <span class="glyph-a hl">ab</span>
    </button>
    {#if menu === "highlight"}
      <div class="swatches">
        {#each HIGHLIGHTS as col (col)}
          <button type="button" class="sw" style:background={col} aria-label="Evidenziatore {col}" onmousedown={keep} onclick={() => exec("hiliteColor", col)}></button>
        {/each}
        <button type="button" class="sw none" aria-label="Togli evidenziazione" onmousedown={keep} onclick={() => exec("hiliteColor", "transparent")}><Icon name="x" size={11} /></button>
      </div>
    {/if}
  </div>
  <span class="sep"></span>
  <button type="button" title="Elenco puntato" onmousedown={keep} onclick={() => exec("insertUnorderedList")}><Icon name="all" size={15} /></button>
  <button type="button" title="Elenco numerato" onmousedown={keep} onclick={() => exec("insertOrderedList")}><span class="num">1.</span></button>
  <button type="button" title="Togli formattazione" onmousedown={keep} onclick={() => exec("removeFormat")}><Icon name="x" size={13} /></button>
</div>

<div
  bind:this={el}
  class="editor"
  class:streaming
  contenteditable={!streaming}
  role="textbox"
  tabindex="0"
  aria-multiline="true"
  aria-label="Messaggio"
  data-placeholder={placeholder}
  oninput={sync}
  onpaste={onPaste}
></div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px;
    margin: 0 -6px 8px;
    padding: 3px;
    border-radius: var(--r-md);
    background: var(--paper-2);
  }
  .toolbar select {
    height: 26px;
    max-width: 120px;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .toolbar select:hover {
    background: var(--cobalt-mist);
  }
  .toolbar button {
    display: inline-grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    padding: 0 5px;
    border-radius: 6px;
    font-size: var(--t-sm);
    color: var(--ink-2);
  }
  .toolbar button:hover {
    background: var(--cobalt-mist);
    color: var(--cobalt);
  }
  .sep {
    width: 1px;
    height: 16px;
    margin: 0 3px;
    background: var(--rule);
  }
  .pop {
    position: relative;
  }
  .glyph-a {
    font-weight: 700;
    line-height: 1;
  }
  .glyph-a.hl {
    padding: 1px 3px;
    border-radius: 3px;
    background: #fef08a;
    color: var(--ink);
    font-size: var(--t-xs);
  }
  .bar {
    display: block;
    width: 14px;
    height: 3px;
    margin-top: 1px;
    border-radius: 2px;
  }
  .num {
    font-size: var(--t-xs);
    font-weight: 700;
  }
  .swatches {
    position: absolute;
    z-index: 30;
    top: 30px;
    left: 0;
    display: grid;
    grid-template-columns: repeat(4, 22px);
    gap: 5px;
    padding: 8px;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: var(--shadow-sheet), 0 0 0 1px var(--rule);
    animation: settle var(--dur-fast) var(--ease-ink);
  }
  .toolbar .sw {
    min-width: 0;
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 6px;
    box-shadow: inset 0 0 0 1px oklch(0 0 0 / 0.08);
    transition: transform var(--dur-fast) var(--ease-ink);
  }
  .toolbar .sw:hover {
    transform: scale(1.12);
  }
  .toolbar .sw.none {
    background: var(--paper);
    color: var(--ink-3);
  }

  .editor {
    min-height: 180px;
    outline: none;
    font-size: 0.95rem;
    line-height: 1.6;
    color: var(--ink);
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
    overflow-wrap: anywhere;
  }
  .editor:empty::before {
    content: attr(data-placeholder);
    color: var(--ink-3);
    pointer-events: none;
  }
  .editor.streaming {
    caret-color: transparent;
    color: var(--cobalt-deep);
  }
  .editor :global(p) {
    margin: 0 0 0.6em;
  }
  .editor :global(ul),
  .editor :global(ol) {
    margin: 0 0 0.6em;
    padding-left: 1.4em;
  }
</style>
