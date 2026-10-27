import type { Folder } from "./api";
import type { IconName } from "./Icon.svelte";

export const roleIcon: Record<string, IconName> = {
  inbox: "inbox",
  sent: "send",
  drafts: "draft",
  trash: "trash",
  junk: "junk",
  archive: "archive",
  all: "all",
  flagged: "star",
};

const roleLabel: Record<string, string> = {
  inbox: "Posta in arrivo",
  sent: "Inviata",
  drafts: "Bozze",
  trash: "Cestino",
  junk: "Indesiderata",
  archive: "Archivio",
  all: "Tutta la posta",
  flagged: "Speciali",
};

export function folderLabel(f: Folder | null | undefined): string {
  if (!f) return "Posta in arrivo";
  return roleLabel[f.role] ?? f.name;
}
