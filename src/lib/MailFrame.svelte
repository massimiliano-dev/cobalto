<script lang="ts">
  import DOMPurify from "dompurify";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { escapeHtml } from "./format";

  let {
    html,
    text,
    allowRemote,
    onblocked,
    onmailto,
  }: {
    html: string | null;
    text: string | null;
    allowRemote: boolean;
    onblocked: (count: number) => void;
    onmailto: (address: string) => void;
  } = $props();

  let frame: HTMLIFrameElement | undefined = $state();
  let height = $state(120);

  const BASE_CSS = `
    html,body{margin:0;padding:0;background:transparent}
    body{font-family:-apple-system,"Segoe UI",system-ui,sans-serif;font-size:14.5px;line-height:1.6;color:#1b2650;
      overflow-wrap:anywhere;-webkit-font-smoothing:antialiased}
    img{max-width:100%;height:auto}
    table{max-width:100%}
    a{color:#2453d6}
    blockquote{margin:.6em 0;padding-left:12px;border-left:2px solid #c9d6f5;color:#4a5a80}
    pre,code{white-space:pre-wrap;font-size:13px}
    .plain{white-space:pre-wrap;font-family:inherit;margin:0}
    .plain .q{color:#4a5a80}
    img[data-blocked]{display:inline-block;min-width:24px;min-height:12px;background:#eef3ff;border-radius:3px}
  `;

  function linkify(s: string) {
    return escapeHtml(s)
      .replace(/(https?:\/\/[^\s<>"']+[^\s<>"'.,;:!?)\]])/g, '<a href="$1">$1</a>')
      .split("\n")
      .map((l) => (l.startsWith("&gt;") ? `<span class="q">${l}</span>` : l))
      .join("\n");
  }

  let doc = $derived.by(() => {
    let blocked = 0;
    let body: string;
    if (html) {
      DOMPurify.removeAllHooks();
      DOMPurify.addHook("afterSanitizeAttributes", (node) => {
        if (node.tagName === "IMG") {
          const src = node.getAttribute("src") ?? "";
          if (/^https?:/i.test(src) && !allowRemote) {
            node.setAttribute("data-blocked", "");
            node.removeAttribute("src");
            node.removeAttribute("srcset");
            blocked++;
          }
        }
        if (!allowRemote && node.hasAttribute("background")) {
          node.removeAttribute("background");
          blocked++;
        }
        if (!allowRemote && node.hasAttribute("style")) {
          const st = node.getAttribute("style")!;
          if (/url\(\s*['"]?https?:/i.test(st)) {
            node.setAttribute("style", st.replace(/url\(\s*['"]?https?:[^)]*\)/gi, "none"));
            blocked++;
          }
        }
      });
      body = DOMPurify.sanitize(html, {
        WHOLE_DOCUMENT: false,
        FORBID_TAGS: ["script", "form", "iframe", "object", "embed", "base", "meta", "link", "input", "button", "textarea", "select"],
        FORBID_ATTR: ["onerror", "onload", "onclick"],
        ADD_ATTR: ["target"],
        ALLOW_DATA_ATTR: false,
      });
      DOMPurify.removeAllHooks();
      if (!allowRemote) {
        body = body.replace(/<style([^>]*)>([\s\S]*?)<\/style>/gi, (_m, attrs, css: string) => {
          const cleaned = css.replace(/url\(\s*['"]?https?:[^)]*\)/gi, () => (blocked++, "none"));
          return `<style${attrs}>${cleaned}</style>`;
        });
      }
    } else {
      body = `<pre class="plain">${linkify(text ?? "")}</pre>`;
    }
    return { srcdoc: `<!doctype html><html><head><meta charset="utf-8"><style>${BASE_CSS}</style></head><body>${body}</body></html>`, blocked };
  });

  $effect(() => {
    onblocked(doc.blocked);
  });

  function onload() {
    const d = frame?.contentDocument;
    if (!d) return;
    const measure = () => {
      height = Math.max(60, d.documentElement.scrollHeight);
    };
    measure();
    new ResizeObserver(measure).observe(d.body);
    d.addEventListener("click", (e) => {
      const a = (e.target as Element).closest?.("a");
      if (!a) return;
      e.preventDefault();
      const href = a.getAttribute("href") ?? "";
      if (href.startsWith("mailto:")) onmailto(decodeURIComponent(href.slice(7).split("?")[0]));
      else if (/^https?:/i.test(href)) openUrl(href);
    });
  }
</script>

<iframe
  bind:this={frame}
  title="Contenuto del messaggio"
  sandbox="allow-same-origin"
  srcdoc={doc.srcdoc}
  style:height="{height}px"
  {onload}
></iframe>

<style>
  iframe {
    display: block;
    width: 100%;
    border: 0;
    background: transparent;
    color-scheme: light;
  }
</style>
