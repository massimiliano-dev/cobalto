export interface Provider {
  id: string;
  label: string;
  imapHost: string;
  imapPort: number;
  smtpHost: string;
  smtpPort: number;
  smtpSecurity: "ssl" | "starttls";
  domains: string[];
  /** Suggerimento sulla password da usare */
  note?: string;
  helpUrl?: string;
  /** Accesso OAuth disponibile per questo provider */
  oauth?: "microsoft" | "google";
  /** Solo OAuth: la password non funziona */
  oauthOnly?: boolean;
}

export const providers: Provider[] = [
  {
    id: "gmail",
    label: "Gmail",
    imapHost: "imap.gmail.com",
    imapPort: 993,
    smtpHost: "smtp.gmail.com",
    smtpPort: 465,
    smtpSecurity: "ssl",
    domains: ["gmail.com", "googlemail.com"],
    oauth: "google",
    note: "Usa una password per le app (serve la verifica in due passaggi).",
    helpUrl: "https://myaccount.google.com/apppasswords",
  },
  {
    id: "icloud",
    label: "iCloud",
    imapHost: "imap.mail.me.com",
    imapPort: 993,
    smtpHost: "smtp.mail.me.com",
    smtpPort: 587,
    smtpSecurity: "starttls",
    domains: ["icloud.com", "me.com", "mac.com"],
    note: "Usa una password specifica per app generata su account.apple.com.",
    helpUrl: "https://account.apple.com",
  },
  {
    id: "aruba",
    label: "Aruba",
    imapHost: "imaps.aruba.it",
    imapPort: 993,
    smtpHost: "smtps.aruba.it",
    smtpPort: 465,
    smtpSecurity: "ssl",
    domains: [],
  },
  {
    id: "libero",
    label: "Libero",
    imapHost: "imapmail.libero.it",
    imapPort: 993,
    smtpHost: "smtp.libero.it",
    smtpPort: 465,
    smtpSecurity: "ssl",
    domains: ["libero.it", "inwind.it", "iol.it", "blu.it", "giallo.it"],
  },
  {
    id: "yahoo",
    label: "Yahoo",
    imapHost: "imap.mail.yahoo.com",
    imapPort: 993,
    smtpHost: "smtp.mail.yahoo.com",
    smtpPort: 465,
    smtpSecurity: "ssl",
    domains: ["yahoo.com", "yahoo.it", "ymail.com"],
    note: "Usa una password per le app generata nelle impostazioni di sicurezza Yahoo.",
  },
  {
    id: "microsoft",
    label: "Outlook / 365",
    imapHost: "outlook.office365.com",
    imapPort: 993,
    smtpHost: "smtp.office365.com",
    smtpPort: 587,
    smtpSecurity: "starttls",
    domains: ["outlook.com", "hotmail.com", "hotmail.it", "live.com", "live.it", "msn.com"],
    oauth: "microsoft",
    oauthOnly: true,
    note: "Vale anche per gli account aziendali Microsoft 365 con dominio proprio.",
  },
  {
    id: "other",
    label: "Altro",
    imapHost: "",
    imapPort: 993,
    smtpHost: "",
    smtpPort: 465,
    smtpSecurity: "ssl",
    domains: [],
  },
];

export function providerForEmail(email: string): Provider | undefined {
  const domain = email.split("@")[1]?.toLowerCase().trim();
  if (!domain) return undefined;
  return providers.find((p) => p.domains.includes(domain));
}
