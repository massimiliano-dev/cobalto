<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { imageToDataUri, sanitizeSignature, signatureHtml } from "./signature";

  let { value = $bindable("") }: { value: string } = $props();

  let editor: HTMLDivElement | undefined = $state();
  let fileInput: HTMLInputElement | undefined = $state();
  let linkOpen = $state(false);
  let linkUrl = $state("");
  let linkText = $state("");
  let selectedImg = $state<HTMLImageElement | null>(null);
  let savedRange: Range | null = null;

  const SIZES = [
    { label: "S", width: 120 },
    { label: "M", width: 200 },
    { label: "L", width: 320 },
  ];

  onMount(() => {
    if (editor) editor.innerHTML = signatureHtml(value);
  });

  function sync() {
    if (!editor) return;
    const empty = !editor.textContent?.trim() && !editor.querySelector("img");
    value = empty ? "" : sanitizeSignature(editor.innerHTML);
  }

  function saveSelection() {
    const sel = window.getSelection();
    if (sel && sel.rangeCount && editor?.contains(sel.anchorNode)) savedRange = sel.getRangeAt(0).cloneRange();
  }

  function restoreSelection() {
    editor?.focus();
    const sel = window.getSelection();
    if (savedRange && sel) {
      sel.removeAllRanges();
      sel.addRange(savedRange);
    }
  }

  function exec(cmd: string, arg?: string) {
    restoreSelection();
    document.execCommand(cmd, false, arg);
    sync();
  }

  function openLink() {
    saveSelection();
    linkText = savedRange?.toString() ?? "";
    linkUrl = "";
    linkOpen = true;
  }

  function insertLink(e: SubmitEvent) {
    e.preventDefault();
    let url = linkUrl.trim();
    if (!url) return;
    if (url.includes("@") && !url.includes(":")) url = "mailto:" + url;
    else if (!/^(https?:|mailto:|tel:)/i.test(url)) url = "https://" + url;
    restoreSelection();
    const label = linkText.trim() || url.replace(/^(mailto:|tel:|https?:\/\/)/, "");
    if (savedRange && !savedRange.collapsed && label === savedRange.toString()) {
      document.execCommand("createLink", false, url);
    } else {
      const a = document.createElement("a");
      a.href = url;
      a.textContent = label;
      document.execCommand("insertHTML", false, a.outerHTML);
    }
    linkOpen = false;
    sync();
  }

  async function insertImage(files: FileList | null) {
    const file = files?.[0];
    if (!file || !file.type.startsWith("image/")) return;
    const src = await imageToDataUri(file);
    restoreSelection();
    document.execCommand("insertHTML", false, `<img src="${src}" alt="${file.name.replace(/\.[^.]+$/, "").replace(/"/g, "")}" width="200">`);
    if (fileInput) fileInput.value = "";
    sync();
  }

  function onPaste(e: ClipboardEvent) {
    const html = e.clipboardData?.getData("text/html");
    if (!html) return;
    e.preventDefault();
    document.execCommand("insertHTML", false, sanitizeSignature(html));
    sync();
  }

  function onClick(e: MouseEvent) {
    const t = e.target as HTMLElement;
    selectedImg = t.tagName === "IMG" ? (t as HTMLImageElement) : null;
  }

  function resize(width: number) {
    if (!selectedImg) return;
    selectedImg.setAttribute("width", String(width));
    selectedImg.removeAttribute("height");
    selectedImg.style.removeProperty("width");
    selectedImg.style.removeProperty("height");
    sync();
  }

  function removeImg() {
    selectedImg?.remove();
    selectedImg = null;
    sync();
  }
</script>

<div class="sig-editor">
  <div class="tools">
    <button type="button" title="Grassetto" onmousedown={(e) => e.preventDefault()} onclick={() => exec("bold")}><b>G</b></button>
    <button type="button" title="Corsivo" onmousedown={(e) => e.preventDefault()} onclick={() => exec("italic")}><i>C</i></button>
    <button type="button" title="Sottolineato" onmousedown={(e) => e.preventDefault()} onclick={() => exec("underline")}><u>S</u></button>
    <span class="sep"></span>
    <button type="button" class="wide" onmousedown={(e) => (e.preventDefault(), saveSelection())} onclick={openLink}>
      <Icon name="clip" size={13} /> Link
    </button>
    <button type="button" class="wide" onmousedown={(e) => (e.preventDefault(), saveSelection())} onclick={() => fileInput?.click()}>
      <Icon name="image" size={13} /> Immagine
    </button>
    <input bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/gif,image/webp" hidden onchange={(e) => insertImage((e.target as HTMLInputElement).files)} />
    {#if selectedImg}
      <span class="sep"></span>
      {#each SIZES as s (s.label)}
        <button type="button" title="Larghezza {s.width}px" onclick={() => resize(s.width)}>{s.label}</button>
      {/each}
      <button type="button" title="Rimuovi immagine" onclick={removeImg}><Icon name="trash" size={13} /></button>
    {/if}
  </div>

  {#if linkOpen}
    <form class="link-form" onsubmit={insertLink}>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input" bind:value={linkUrl} placeholder="https://… oppure email" autofocus spellcheck="false" />
      <input class="input" bind:value={linkText} placeholder="Testo (facoltativo)" />
      <button class="btn btn-primary">Inserisci</button>
      <button type="button" class="btn btn-quiet" onclick={() => (linkOpen = false)}>Annulla</button>
    </form>
  {/if}

  <div
    bind:this={editor}
    class="area"
    contenteditable="true"
    role="textbox"
    tabindex="0"
    aria-multiline="true"
    aria-label="Firma"
    data-placeholder="Mario Rossi · Acme — puoi incollare una firma con logo e link"
    oninput={sync}
    onblur={() => (saveSelection(), sync())}
    onkeyup={saveSelection}
    onmouseup={saveSelection}
    onpaste={onPaste}
    onclick={onClick}
  ></div>
</div>

<style>
  .sig-editor {
    border-radius: var(--r-sm);
    border: 1px solid var(--rule);
    background: var(--paper);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .sig-editor:focus-within {
    border-color: var(--cobalt);
    box-shadow: 0 0 0 3px oklch(0.5 0.215 263 / 0.14);
  }
  .tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 2px;
    padding: 4px;
    border-bottom: 1px solid var(--rule-soft);
    background: var(--paper-2);
    border-radius: var(--r-sm) var(--r-sm) 0 0;
  }
  .tools button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-width: 26px;
    height: 26px;
    padding: 0 6px;
    border-radius: 5px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-2);
  }
  .tools button:hover {
    background: var(--cobalt-mist);
    color: var(--cobalt);
  }
  .sep {
    width: 1px;
    height: 16px;
    margin: 0 4px;
    background: var(--rule);
  }
  .link-form {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid var(--rule-soft);
    animation: settle 0.18s var(--ease-out);
  }
  .link-form .input {
    flex: 1 1 140px;
    height: 32px;
  }
  .link-form .btn {
    height: 32px;
  }
  .area {
    min-height: 84px;
    max-height: 240px;
    overflow-y: auto;
    padding: 10px 12px;
    font-size: var(--t-sm);
    line-height: 1.5;
    outline: none;
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }
  .area:empty::before {
    content: attr(data-placeholder);
    color: var(--ink-3);
  }
  .area :global(a) {
    color: var(--cobalt);
  }
  .area :global(img) {
    max-width: 100%;
    height: auto;
    vertical-align: middle;
    border-radius: 2px;
    cursor: pointer;
  }
  .area :global(img:hover) {
    outline: 2px solid oklch(0.5 0.215 263 / 0.35);
  }
</style>
