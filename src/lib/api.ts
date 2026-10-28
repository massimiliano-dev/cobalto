import { invoke } from "@tauri-apps/api/core";

export interface Account {
  id: string;
  name: string;
  email: string;
  username: string;
  imapHost: string;
  imapPort: number;
  smtpHost: string;
  smtpPort: number;
  smtpSecurity: "ssl" | "starttls";
  signature: string;
  auth?: "password" | "oauth-microsoft" | "oauth-google";
  /** Casella condivisa: id dell'account con cui la si apre */
  sharedOf?: string;
}

export interface Addr {
  name: string;
  email: string;
}

export interface Envelope {
  uid: number;
  subject: string;
  from: Addr;
  to: Addr[];
  date: number;
  preview: string;
  seen: boolean;
  flagged: boolean;
  answered: boolean;
  hasAttachments: boolean;
  size: number;
}

export interface MessagePage {
  messages: Envelope[];
  total: number;
  hasMore: boolean;
}

export interface Attachment {
  index: number;
  name: string;
  mime: string;
  size: number;
  inline: boolean;
}

export interface FullMessage {
  uid: number;
  subject: string;
  from: Addr;
  to: Addr[];
  cc: Addr[];
  replyTo: Addr[];
  date: number;
  flagged: boolean;
  html: string | null;
  text: string | null;
  attachments: Attachment[];
  messageId: string | null;
  references: string[];
  listUnsubscribe: string | null;
}

export type FolderRole = "inbox" | "sent" | "drafts" | "trash" | "junk" | "archive" | "all" | "flagged" | "";

export interface Folder {
  path: string;
  name: string;
  depth: number;
  role: FolderRole;
  unseen: number;
}

export type AiProvider = "anthropic" | "openai" | "local";

export interface Settings {
  aiProvider: AiProvider;
  anthropicModel: string;
  openaiModel: string;
  openaiBaseUrl: string;
  aiLanguage: string;
  localModel: string;
  localUrl: string;
}

export interface ChatMsg {
  role: "user" | "assistant";
  content: string;
}

export interface LocalStatus {
  running: boolean;
  models: { name: string; size: number }[];
}

export interface PullProgress {
  model: string;
  status: string;
  completed: number;
  total: number;
  done: boolean;
  error: string | null;
}

export interface SettingsState {
  settings: Settings;
  hasAnthropicKey: boolean;
  hasOpenaiKey: boolean;
}

export interface OutgoingAttachment {
  name: string;
  mime: string;
  data: string;
}

export interface Draft {
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  text: string;
  html: string;
  inReplyTo?: string | null;
  references?: string[];
  attachments: OutgoingAttachment[];
  forwardOf?: { folder: string; uid: number } | null;
}

export interface InboxCheck {
  unseen: number;
  lastUid: number;
  new: Envelope[];
}

export type OAuthProvider = "microsoft" | "google";

export interface CalEvent {
  id: string;
  accountId: string;
  title: string;
  start: number;
  end: number;
  allDay: boolean;
  startDate: string | null;
  endDate: string | null;
  location: string;
  organizer: string;
  attendees: string[];
  joinUrl: string | null;
  joinKind: "teams" | "meet" | "zoom" | "webex" | "altro" | null;
  webLink: string | null;
  preview: string;
  showAs: string;
  cancelled: boolean;
}

export interface CalendarResult {
  events: CalEvent[];
  accounts: { accountId: string; status: "ok" | "unsupported" | "error"; message: string | null }[];
}

export const api = {
  accounts: () => invoke<Account[]>("accounts_list"),
  saveAccount: (account: Account, password: string | null) =>
    invoke<Account>("account_save", { account, password }),
  removeAccount: (id: string) => invoke<void>("account_remove", { id }),
  oauthAvailable: () => invoke<OAuthProvider[]>("oauth_available"),
  oauthConnect: (provider: OAuthProvider) => invoke<Account>("oauth_connect", { provider }),
  oauthCancel: () => invoke<void>("oauth_cancel"),
  sharedMailboxes: (accountId: string) =>
    invoke<{ email: string; name: string; unread: number }[]>("shared_mailboxes", { accountId }),
  sharedAdd: (accountId: string, email: string, name: string) => invoke<Account>("shared_add", { accountId, email, name }),
  calendar: (start: number, end: number) => invoke<CalendarResult>("calendar_events", { start, end }),
  folderUnseen: (accountId: string, folder: string) => invoke<number>("folder_unseen", { accountId, folder }),
  inboxCheck: (accountId: string, afterUid: number | null) =>
    invoke<InboxCheck>("inbox_check", { accountId, afterUid }),

  folders: (accountId: string) => invoke<Folder[]>("folders_list", { accountId }),
  cached: (accountId: string, folder: string) =>
    invoke<MessagePage | null>("messages_cached", { accountId, folder }),
  list: (accountId: string, folder: string, page: number) =>
    invoke<MessagePage>("messages_list", { accountId, folder, page }),
  search: (accountId: string, folder: string, query: string) =>
    invoke<Envelope[]>("messages_search", { accountId, folder, query }),
  message: (accountId: string, folder: string, uid: number) =>
    invoke<FullMessage>("message_get", { accountId, folder, uid }),
  flag: (accountId: string, folder: string, uids: number[], flag: "seen" | "flagged", value: boolean) =>
    invoke<void>("message_flag", { accountId, folder, uids, flag, value }),
  move: (accountId: string, folder: string, uids: number[], dest: string) =>
    invoke<void>("message_move", { accountId, folder, uids, dest }),
  deleteForever: (accountId: string, folder: string, uids: number[]) =>
    invoke<void>("message_delete_forever", { accountId, folder, uids }),
  saveAttachment: (accountId: string, folder: string, uid: number, index: number) =>
    invoke<string>("attachment_save", { accountId, folder, uid, index }),
  send: (accountId: string, draft: Draft, sentFolder: string | null) =>
    invoke<void>("mail_send", { accountId, draft, sentFolder }),

  settings: () => invoke<SettingsState>("settings_get"),
  saveSettings: (settings: Settings) => invoke<void>("settings_save", { settings }),
  setAiKey: (provider: string, key: string) => invoke<void>("ai_key_set", { provider, key }),
  aiModels: (provider: string) => invoke<string[]>("ai_models", { provider }),
  aiRun: (id: string, system: string, messages: ChatMsg[]) => invoke<void>("ai_run", { id, system, messages }),
  localStatus: () => invoke<LocalStatus>("local_ai_status"),
  localPull: (model: string) => invoke<void>("local_ai_pull", { model }),
  aiCancel: (id: string) => invoke<void>("ai_cancel", { id }),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Qualcosa è andato storto";
}
