<script lang="ts">
  import { mail } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { flip } from "svelte/animate";
  import { sheet } from "./motion";
</script>

<div class="toasts" aria-live="polite">
  {#each mail.toasts as t (t.id)}
    <div class="toast" class:error={t.kind === "error"} out:sheet={{ y: 10, scale: 0.97, duration: 160 }} animate:flip={{ duration: 220 }}>
      <span class="text">{t.text}</span>
      {#if t.action}
        <button
          class="action"
          onclick={() => {
            t.action!.run();
            mail.dismiss(t.id);
          }}>{t.action.label}</button
        >
      {/if}
      <button class="x" onclick={() => mail.dismiss(t.id)} aria-label="Chiudi"><Icon name="x" size={13} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: absolute;
    z-index: 100;
    left: 50%;
    bottom: 22px;
    transform: translateX(-50%);
    display: grid;
    gap: 8px;
    justify-items: center;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 14px;
    max-width: min(560px, calc(100vw - 40px));
    min-height: 42px;
    padding: 6px 8px 6px 16px;
    border-radius: 12px;
    background: var(--cobalt-night);
    color: var(--on-deep);
    font-size: var(--t-sm);
    box-shadow: 0 14px 34px -12px oklch(0.2 0.12 266 / 0.6);
    pointer-events: auto;
    animation: up 0.3s var(--ease-out);
  }
  .toast.error {
    background: oklch(0.32 0.1 20);
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
  .text {
    line-height: 1.35;
    user-select: text;
    -webkit-user-select: text;
  }
  .action {
    font-weight: 700;
    color: oklch(0.85 0.09 255);
  }
  .action:hover {
    color: var(--on-deep);
  }
  .x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    color: var(--on-deep-3);
  }
  .x:hover {
    background: oklch(1 0 0 / 0.1);
    color: var(--on-deep);
  }
</style>
