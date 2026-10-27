import {
  api,
  errorText,
  type Account,
  type Envelope,
  type Folder,
  type FolderRole,
  type FullMessage,
  type OutgoingAttachment,
  type SettingsState,
} from "./api";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { notify } from "./sounds";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatAddr, fullDate, textToHtml, escapeHtml, displayName } from "./format";
import { signatureHtml, signatureText } from "./signature";
import { providerLabel } from "./ai";
import { cal } from "./calendar.svelte";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
  action?: { label: string; run: () => void };
}

export interface ComposeState {
  key: number;
  mode: "new" | "reply" | "replyAll" | "forward";
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  quoted: { header: string; html: string; text: string } | null;
  inReplyTo: string | null;
  references: string[];
  attachments: OutgoingAttachment[];
  source: FullMessage | null;
  /** Inoltro: cartella e UID del messaggio originale, per riallegarne i file */
  forwardOf: { folder: string; uid: number } | null;
  sending: boolean;
  /** Se presente, il composer genera subito una risposta con l'AI */
  aiIntent: string | null;
}

const LAST_KEY = "cobalto:last";

class MailState {
  ready = $state(false);
  accounts = $state<Account[]>([]);
  accountId = $state<string | null>(null);
  folders = $state<Record<string, Folder[]>>({});
  folder = $state("INBOX");

  messages = $state<Envelope[]>([]);
  total = $state(0);
  hasMore = $state(false);
  page = $state(0);
  listLoading = $state(false);
  syncing = $state(false);

  query = $state("");
  searching = $state(false);
  searchResults = $state<Envelope[] | null>(null);

  selectedUid = $state<number | null>(null);
  message = $state<FullMessage | null>(null);
  messageLoading = $state(false);
  messageError = $state<string | null>(null);

  compose = $state<ComposeState | null>(null);
  settings = $state<SettingsState | null>(null);

  showSetup = $state(false);
  editAccount = $state<Account | null>(null);
  showSettings = $state(false);
  showPalette = $state(false);
  showAssistant = $state(false);
  view = $state<"mail" | "calendar">("mail");

  toasts = $state<Toast[]>([]);

  aiLabel = $derived(providerLabel(this.settings?.settings));
  aiReady = $derived.by(() => {
    const st = this.settings;
    if (!st) return false;
    if (st.settings.aiProvider === "local") return true;
    return st.settings.aiProvider === "openai" ? st.hasOpenaiKey : st.hasAnthropicKey;
  });
  account = $derived(this.accounts.find((a) => a.id === this.accountId) ?? null);
  currentFolders = $derived(this.accountId ? (this.folders[this.accountId] ?? []) : []);
  currentFolder = $derived(this.currentFolders.find((f) => f.path === this.folder) ?? null);
  visible = $derived(this.searchResults ?? this.messages);
  selectedIndex = $derived(this.visible.findIndex((m) => m.uid === this.selectedUid));

  /** UID appena arrivati o ripristinati: solo loro entrano in lista con l'animazione */
  fresh = new Set<number>();
  private lastUid: Record<string, number> = {};
  private unseenByAccount = $state<Record<string, number>>({});
  private checking = false;
  private loadToken = 0;
  private messageToken = 0;
  private toastId = 0;
  private pending = new Map<number, ReturnType<typeof setTimeout>>();

  // ---------- Avvio ----------

  async init() {
    try {
      const [accounts, settings] = await Promise.all([api.accounts(), api.settings()]);
      this.accounts = accounts;
      this.settings = settings;
      let last: { accountId?: string; folder?: string } = {};
      try {
        last = JSON.parse(localStorage.getItem(LAST_KEY) ?? "{}");
      } catch {}
      const acc = accounts.find((a) => a.id === last.accountId) ?? accounts[0];
      if (acc) {
        this.accountId = acc.id;
        this.folder = last.accountId === acc.id && last.folder ? last.folder : "INBOX";
        this.loadFolder();
        this.loadFolders(acc.id);
      } else {
        this.showSetup = true;
      }
    } catch (e) {
      this.toast(errorText(e), "error");
    } finally {
      this.ready = true;
    }
    setInterval(() => this.backgroundSync(), 90_000);
    this.checkInboxes();
    setInterval(() => this.checkInboxes(), 60_000);
    // Prossime riunioni: avvisi e contesto dell'assistente anche senza aprire il calendario
    cal.start();
    // Tornando sull'app si ricontrolla subito: badge e notifiche non restano indietro
    window.addEventListener("focus", () => this.checkInboxes());

    // Badge reattivo: i non letti della Posta in arrivo dell'account che stai guardando.
    // Cambia con l'account e scende nell'istante in cui leggi, archivi o elimini
    $effect.root(() => {
      $effect(() => {
        const n = this.accountId ? this.unreadFor(this.accountId) : 0;
        getCurrentWindow()
          .setBadgeCount(n > 0 ? n : undefined)
          .catch(() => {});
      });
    });
  }

  /** Non letti nella Posta in arrivo di un account (dato locale se disponibile, se no l'ultimo controllo). */
  unreadFor(accountId: string): number {
    const inbox = this.folders[accountId]?.find((f) => f.role === "inbox");
    return inbox?.unseen ?? this.unseenByAccount[accountId] ?? 0;
  }

  /** Nuova posta su tutti gli account: notifiche di sistema e numero sul Dock. */
  private async checkInboxes() {
    if (this.checking) return;
    this.checking = true;
    for (const acc of this.accounts) {
      try {
        const known = this.lastUid[acc.id];
        const res = await api.inboxCheck(acc.id, known ?? null);
        this.lastUid[acc.id] = res.lastUid;
        this.unseenByAccount[acc.id] = res.unseen;
        const inbox = this.folders[acc.id]?.find((f) => f.role === "inbox");
        if (inbox) inbox.unseen = res.unseen;
        if (known !== undefined && res.new.length) {
          await this.notify(acc, res.new);
          if (acc.id === this.accountId && this.currentFolder?.role === "inbox") this.backgroundSync();
        }
      } catch {
        // Rete assente o account da ricollegare: lo dirà la prossima azione esplicita
      }
    }
    this.checking = false;
  }

  private async notify(acc: Account, messages: Envelope[]) {
    if (document.hasFocus()) return;
    const many = this.accounts.length > 1 ? ` · ${acc.email}` : "";
    if (messages.length > 3) {
      await notify("mail", `${messages.length} nuovi messaggi${many}`, messages.slice(0, 3).map((m) => displayName(m.from)).join(", ") + "…");
      return;
    }
    // Un solo suono anche se arrivano più messaggi insieme
    for (const [i, m] of messages.entries()) {
      await notify("mail", displayName(m.from) + many, [m.subject, m.preview].filter(Boolean).join(" — ").slice(0, 180), i === 0 ? undefined : "none");
    }
  }


  private remember() {
    try {
      localStorage.setItem(LAST_KEY, JSON.stringify({ accountId: this.accountId, folder: this.folder }));
    } catch {}
  }

  async loadFolders(accountId: string) {
    try {
      this.folders[accountId] = await api.folders(accountId);
    } catch (e) {
      this.toast(errorText(e), "error");
    }
  }

  folderByRole(role: FolderRole): Folder | undefined {
    return this.currentFolders.find((f) => f.role === role);
  }

  // ---------- Navigazione ----------

  selectAccount(id: string) {
    if (id === this.accountId) return;
    this.accountId = id;
    this.folder = "INBOX";
    this.clearSelection();
    this.clearSearch();
    this.loadFolder();
    if (!this.folders[id]) this.loadFolders(id);
  }

  selectFolder(path: string) {
    this.folder = path;
    this.clearSelection();
    this.clearSearch();
    this.loadFolder();
  }

  clearSelection() {
    this.selectedUid = null;
    this.message = null;
    this.messageError = null;
  }

  /** Mostra subito la cache locale, poi aggiorna dal server. */
  async loadFolder() {
    const accountId = this.accountId;
    if (!accountId) return;
    const folder = this.folder;
    const token = ++this.loadToken;
    this.fresh.clear();
    this.remember();
    this.page = 0;
    const cached = await api.cached(accountId, folder).catch(() => null);
    if (token !== this.loadToken) return;
    if (cached) {
      this.messages = cached.messages;
      this.total = cached.total;
      this.hasMore = cached.hasMore;
    } else {
      this.messages = [];
      this.listLoading = true;
    }
    this.syncing = true;
    try {
      const page = await api.list(accountId, folder, 0);
      if (token !== this.loadToken) return;
      this.messages = page.messages;
      this.total = page.total;
      this.hasMore = page.hasMore;
    } catch (e) {
      if (token === this.loadToken) this.toast(errorText(e), "error");
    } finally {
      if (token === this.loadToken) {
        this.listLoading = false;
        this.syncing = false;
      }
    }
  }

  async loadMore() {
    if (!this.accountId || !this.hasMore || this.listLoading || this.searchResults) return;
    const token = this.loadToken;
    this.listLoading = true;
    try {
      const next = await api.list(this.accountId, this.folder, this.page + 1);
      if (token !== this.loadToken) return;
      const known = new Set(this.messages.map((m) => m.uid));
      this.messages = [...this.messages, ...next.messages.filter((m) => !known.has(m.uid))];
      this.page += 1;
      this.hasMore = next.hasMore;
    } catch (e) {
      this.toast(errorText(e), "error");
    } finally {
      this.listLoading = false;
    }
  }

  async refresh() {
    if (this.accountId) this.loadFolders(this.accountId);
    await this.loadFolder();
  }

  private async backgroundSync() {
    if (!this.accountId || this.syncing || document.hidden || this.searchResults) return;
    const accountId = this.accountId;
    const folder = this.folder;
    const token = this.loadToken;
    try {
      const page = await api.list(accountId, folder, 0);
      if (token !== this.loadToken) return;
      const top = new Map(page.messages.map((m) => [m.uid, m]));
      const known = new Set(this.messages.map((m) => m.uid));
      for (const m of page.messages) if (!known.has(m.uid)) this.fresh.add(m.uid);
      const older = this.messages.filter((m) => !top.has(m.uid) && m.uid < (page.messages.at(-1)?.uid ?? 0));
      this.messages = [...page.messages, ...older];
      this.total = page.total;
      this.loadFolders(accountId);
    } catch {
      // Silenzioso: riproveremo al prossimo giro
    }
  }

  // ---------- Ricerca ----------

  async search(q: string) {
    this.query = q;
    if (!q.trim() || !this.accountId) {
      this.clearSearch();
      return;
    }
    this.searching = true;
    try {
      const results = await api.search(this.accountId, this.folder, q.trim());
      if (this.query === q) this.searchResults = results;
    } catch (e) {
      this.toast(errorText(e), "error");
    } finally {
      this.searching = false;
    }
  }

  clearSearch() {
    this.query = "";
    this.searchResults = null;
  }

  // ---------- Lettura ----------

  async open(uid: number) {
    if (!this.accountId) return;
    this.selectedUid = uid;
    const token = ++this.messageToken;
    this.messageLoading = true;
    this.messageError = null;
    const env = this.visible.find((m) => m.uid === uid);
    if (env && !env.seen) {
      env.seen = true;
      this.bumpUnseen(-1);
    }
    try {
      const msg = await api.message(this.accountId, this.folder, uid);
      if (token === this.messageToken) this.message = msg;
    } catch (e) {
      if (token === this.messageToken) {
        this.message = null;
        this.messageError = errorText(e);
      }
    } finally {
      if (token === this.messageToken) this.messageLoading = false;
    }
  }

  move(delta: number) {
    const list = this.visible;
    if (!list.length) return;
    const i = this.selectedIndex;
    const next = i < 0 ? 0 : Math.min(list.length - 1, Math.max(0, i + delta));
    if (list[next] && list[next].uid !== this.selectedUid) this.open(list[next].uid);
  }

  private bumpUnseen(delta: number) {
    const f = this.currentFolder;
    if (f) f.unseen = Math.max(0, f.unseen + delta);
  }

  // ---------- Azioni ----------

  async toggleFlag(uid = this.selectedUid) {
    const env = this.visible.find((m) => m.uid === uid);
    if (!env || !this.accountId) return;
    env.flagged = !env.flagged;
    if (this.message?.uid === uid) this.message.flagged = env.flagged;
    try {
      await api.flag(this.accountId, this.folder, [env.uid], "flagged", env.flagged);
    } catch (e) {
      env.flagged = !env.flagged;
      this.toast(errorText(e), "error");
    }
  }

  async toggleSeen(uid = this.selectedUid) {
    const env = this.visible.find((m) => m.uid === uid);
    if (!env || !this.accountId) return;
    env.seen = !env.seen;
    this.bumpUnseen(env.seen ? -1 : 1);
    try {
      await api.flag(this.accountId, this.folder, [env.uid], "seen", env.seen);
    } catch (e) {
      env.seen = !env.seen;
      this.toast(errorText(e), "error");
    }
  }

  archive(uid = this.selectedUid) {
    const dest = this.folderByRole("archive") ?? this.folderByRole("all");
    if (!dest) return this.toast("Nessuna cartella Archivio su questo account", "error");
    if (this.folder === dest.path) return;
    this.removeWithUndo(uid, "Archiviato", (acc, folder, uids) => api.move(acc, folder, uids, dest.path));
  }

  trash(uid = this.selectedUid) {
    const trash = this.folderByRole("trash");
    if (!trash || this.folder === trash.path) {
      this.removeWithUndo(uid, "Eliminato definitivamente", (acc, folder, uids) => api.deleteForever(acc, folder, uids));
    } else {
      this.removeWithUndo(uid, "Spostato nel cestino", (acc, folder, uids) => api.move(acc, folder, uids, trash.path));
    }
  }

  moveTo(path: string, uid = this.selectedUid) {
    const f = this.currentFolders.find((x) => x.path === path);
    this.removeWithUndo(uid, `Spostato in ${f?.name ?? path}`, (acc, folder, uids) => api.move(acc, folder, uids, path));
  }

  /**
   * UI ottimistica: il messaggio sparisce subito, l'operazione sul server parte
   * dopo qualche secondo così "Annulla" non richiede un secondo viaggio sul server.
   */
  private removeWithUndo(
    uid: number | null,
    label: string,
    op: (accountId: string, folder: string, uids: number[]) => Promise<void>,
  ) {
    if (uid == null || !this.accountId) return;
    const accountId = this.accountId;
    const folder = this.folder;
    const list = this.searchResults ?? this.messages;
    const index = list.findIndex((m) => m.uid === uid);
    if (index < 0) return;
    const [env] = list.splice(index, 1);
    if (this.searchResults) this.messages = this.messages.filter((m) => m.uid !== uid);
    if (!env.seen) this.bumpUnseen(-1);

    if (this.selectedUid === uid) {
      const next = list[index] ?? list[index - 1];
      if (next) this.open(next.uid);
      else this.clearSelection();
    }

    const timer = setTimeout(async () => {
      this.pending.delete(uid);
      try {
        await op(accountId, folder, [uid]);
      } catch (e) {
        this.toast(errorText(e), "error");
        if (this.accountId === accountId && this.folder === folder) this.loadFolder();
      }
    }, 4500);
    this.pending.set(uid, timer);

    this.toast(label, "info", {
      label: "Annulla",
      run: () => {
        clearTimeout(timer);
        this.pending.delete(uid);
        if (this.accountId !== accountId || this.folder !== folder) return;
        const target = this.searchResults ?? this.messages;
        this.fresh.add(env.uid);
        target.splice(Math.min(index, target.length), 0, env);
        if (!env.seen) this.bumpUnseen(1);
        this.open(env.uid);
      },
    });
  }

  async saveAttachment(index: number) {
    if (!this.accountId || !this.message) return;
    try {
      const path = await api.saveAttachment(this.accountId, this.folder, this.message.uid, index);
      this.toast(`Salvato in Download`, "info", {
        label: "Apri",
        run: () => openPath(path).catch(() => revealItemInDir(path)),
      });
    } catch (e) {
      this.toast(errorText(e), "error");
    }
  }

  // ---------- Scrittura ----------

  startCompose(mode: ComposeState["mode"] = "new", source: FullMessage | null = null, body = "") {
    const me = this.account?.email.toLowerCase() ?? "";
    let to = "";
    let cc = "";
    let subject = "";
    let quoted: ComposeState["quoted"] = null;
    if (source) {
      const replyTargets = source.replyTo.length ? source.replyTo : [source.from];
      if (mode === "reply") to = replyTargets.map(formatAddr).join(", ");
      if (mode === "replyAll") {
        to = replyTargets.map(formatAddr).join(", ");
        cc = [...source.to, ...source.cc]
          .filter((a) => a.email.toLowerCase() !== me && !replyTargets.some((r) => r.email === a.email))
          .map(formatAddr)
          .join(", ");
      }
      const base = source.subject.replace(/^((re|r|fw|fwd|i|inoltra)\s*:\s*)+/i, "");
      subject = mode === "forward" ? `I: ${base}` : `Re: ${base}`;
      const header =
        mode === "forward"
          ? `---------- Messaggio inoltrato ----------\nDa: ${formatAddr(source.from)}\nData: ${fullDate(source.date)}\nOggetto: ${source.subject}\nA: ${source.to.map(formatAddr).join(", ")}`
          : `Il ${fullDate(source.date)}, ${displayName(source.from)} ha scritto:`;
      quoted = {
        header,
        html: source.html ?? textToHtml(source.text ?? ""),
        text: source.text ?? "",
      };
    }
    this.compose = {
      key: Date.now(),
      mode,
      to,
      cc,
      bcc: "",
      subject,
      body,
      quoted,
      inReplyTo: mode === "forward" ? null : (source?.messageId ?? null),
      references: mode === "forward" ? [] : (source?.references ?? []),
      attachments: [],
      source,
      forwardOf: mode === "forward" && source ? { folder: this.folder, uid: source.uid } : null,
      sending: false,
      aiIntent: null,
    };
  }

  async send() {
    const c = this.compose;
    const acc = this.account;
    if (!c || !acc) return;
    const split = (s: string) =>
      s
        .split(/[,;]\s*(?=(?:[^"]*"[^"]*")*[^"]*$)/)
        .map((x) => x.trim())
        .filter(Boolean);
    const to = split(c.to);
    if (!to.length) return this.toast("Aggiungi almeno un destinatario", "error");

    const sigText = signatureText(acc.signature);
    const sigHtml = signatureHtml(acc.signature);
    const text =
      c.body.trimEnd() +
      // Qui il backend inserisce gli eventuali link OneDrive (o toglie il segnaposto)
      "\n\n{{cobalto-links}}" +
      (sigText ? `\n\n-- \n${sigText}` : "") +
      (c.quoted ? `\n\n${c.quoted.header}\n${c.quoted.text.split("\n").map((l) => "> " + l).join("\n")}` : "");
    const html =
      `<div style="font-family:-apple-system,'Segoe UI',sans-serif;font-size:14px;line-height:1.5;color:#14213d">` +
      textToHtml(c.body.trimEnd()) +
      "<!--cobalto-links-->" +
      (sigHtml ? `<div class="cobalto-signature" style="margin-top:16px">${sigHtml}</div>` : "") +
      (c.quoted
        ? `<br><div style="color:#4a5a80">${escapeHtml(c.quoted.header).replace(/\n/g, "<br>")}</div>` +
          `<blockquote style="margin:8px 0 0;padding-left:12px;border-left:2px solid #c9d6f5">${c.quoted.html}</blockquote>`
        : "") +
      `</div>`;

    c.sending = true;
    try {
      await api.send(
        acc.id,
        {
          to,
          cc: split(c.cc),
          bcc: split(c.bcc),
          subject: c.subject,
          text,
          html,
          inReplyTo: c.inReplyTo,
          references: c.references,
          attachments: c.attachments,
          forwardOf: c.forwardOf,
        },
        this.folderByRole("sent")?.path ?? null,
      );
      if ((c.mode === "reply" || c.mode === "replyAll") && c.source) {
        const env = this.messages.find((m) => m.uid === c.source!.uid);
        if (env) env.answered = true;
      }
      this.compose = null;
      this.toast("Messaggio inviato");
    } catch (e) {
      c.sending = false;
      this.toast(errorText(e), "error");
    }
  }

  // ---------- Account ----------

  async accountSaved(acc: Account) {
    this.accounts = await api.accounts();
    this.showSetup = false;
    this.editAccount = null;
    this.selectAccount(acc.id);
    this.loadFolders(acc.id);
    this.toast(`${acc.email} è pronto`);
  }

  async removeAccount(id: string) {
    await api.removeAccount(id);
    this.accounts = this.accounts.filter((a) => a.id !== id);
    delete this.folders[id];
    if (this.accountId === id) {
      this.accountId = null;
      this.messages = [];
      this.clearSelection();
      if (this.accounts[0]) this.selectAccount(this.accounts[0].id);
      else this.showSetup = true;
    }
  }

  // ---------- Notifiche ----------

  toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"]) {
    const id = ++this.toastId;
    this.toasts.push({ id, text, kind, action });
    setTimeout(() => this.dismiss(id), kind === "error" ? 7000 : action ? 5000 : 3000);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const mail = new MailState();
