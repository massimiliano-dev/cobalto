<script lang="ts">
  import { onMount } from "svelte";
  import { mail } from "./lib/state.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import MessageList from "./lib/MessageList.svelte";
  import Reader from "./lib/Reader.svelte";
  import Composer from "./lib/Composer.svelte";
  import AccountSetup from "./lib/AccountSetup.svelte";
  import Settings from "./lib/Settings.svelte";
  import Palette from "./lib/Palette.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import Assistant from "./lib/Assistant.svelte";
  import CalendarView from "./lib/CalendarView.svelte";
  import { cal } from "./lib/calendar.svelte";

  const isMac = navigator.userAgent.includes("Mac");

  onMount(() => {
    mail.init();
  });

  function typing(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    return t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName);
  }

  // Scorciatoie da tastiera, in stile Gmail/Superhuman
  function onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      mail.showPalette = !mail.showPalette;
      return;
    }
    if (mod && (e.key === "1" || e.key === "2")) {
      e.preventDefault();
      mail.view = e.key === "1" ? "mail" : "calendar";
      return;
    }
    if (mod && e.key.toLowerCase() === "j") {
      e.preventDefault();
      mail.showAssistant = !mail.showAssistant;
      return;
    }
    if (mod && e.key.toLowerCase() === "n") {
      e.preventDefault();
      mail.startCompose();
      return;
    }
    if (mod && e.key === "Enter" && mail.compose) {
      e.preventDefault();
      mail.send();
      return;
    }
    if (e.key === "Escape") {
      if (mail.showPalette) mail.showPalette = false;
      else if (mail.view === "calendar" && cal.selected) cal.selected = null;
      else if (mail.showAssistant && !mail.compose) mail.showAssistant = false;
      else if (mail.showSettings) mail.showSettings = false;
      else if (mail.showSetup && mail.accounts.length) mail.showSetup = false;
      else if (mail.searchResults) mail.clearSearch();
      return;
    }
    if (typing(e) || mod || e.altKey || mail.compose || mail.showPalette || mail.showSetup || mail.showSettings) return;
    if (e.key === "?") {
      e.preventDefault();
      mail.showAssistant = true;
      return;
    }

    if (mail.view === "calendar") {
      const calKeys: Record<string, () => void> = {
        t: () => cal.today(),
        ArrowLeft: () => cal.go(-1),
        ArrowRight: () => cal.go(1),
        k: () => cal.go(-1),
        j: () => cal.go(1),
        c: () => mail.startCompose(),
      };
      const runCal = calKeys[e.key];
      if (runCal) {
        e.preventDefault();
        runCal();
      }
      return;
    }

    const m = mail.message;
    const actions: Record<string, () => void> = {
      j: () => mail.move(1),
      ArrowDown: () => mail.move(1),
      k: () => mail.move(-1),
      ArrowUp: () => mail.move(-1),
      c: () => mail.startCompose(),
      r: () => m && mail.startCompose("reply", m),
      a: () => m && mail.startCompose("replyAll", m),
      f: () => m && mail.startCompose("forward", m),
      e: () => mail.archive(),
      "#": () => mail.trash(),
      Delete: () => mail.trash(),
      Backspace: () => mail.trash(),
      s: () => mail.toggleFlag(),
      u: () => mail.toggleSeen(),
      "/": () => document.getElementById("search")?.focus(),
    };
    const run = actions[e.key];
    if (run) {
      e.preventDefault();
      run();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="shell" class:mac={isMac}>
  {#if mail.ready && !mail.accounts.length}
    <AccountSetup welcome />
  {:else}
    <Sidebar />
    {#if mail.view === "calendar"}
      <CalendarView />
    {:else}
      <MessageList />
      <Reader />
    {/if}
    {#if mail.showAssistant}
      <Assistant />
    {/if}
    {#if mail.compose}
      {#key mail.compose.key}
        <Composer />
      {/key}
    {/if}
    {#if mail.showSetup}
      <AccountSetup />
    {/if}
  {/if}
  {#if mail.showSettings}
    <Settings />
  {/if}
  {#if mail.showPalette}
    <Palette />
  {/if}
  <Toasts />
</div>

<style>
  .shell {
    position: relative;
    display: grid;
    grid-template-columns: 236px minmax(320px, 380px) 1fr;
    height: 100vh;
    background: var(--paper);
  }

  @media (max-width: 1100px) {
    .shell {
      grid-template-columns: 210px minmax(290px, 330px) 1fr;
    }
  }
</style>
