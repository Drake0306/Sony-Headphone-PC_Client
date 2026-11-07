<!--
  Share a setting: a modal like Confirm with the QR of the code (the code
  text itself, drawn at open time), the code in mono with its dashes, a muted
  line, then Copy (clipboard; says "Copied") and Close. Focus starts on Copy
  and stays inside the panel; Escape or a click on the backdrop closes.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { QR_MARGIN, qrModules, qrPath } from "../qr";

  let { title, code, onclose }: { title: string; code: string; onclose: () => void } = $props();

  let panel: HTMLElement;
  let copyButton: HTMLButtonElement;
  let copied = $state<string | null>(null);
  const id = `share-${Math.random().toString(36).slice(2)}`;

  const qr = $derived(qrModules(code));
  const box = $derived(qr.size + 2 * QR_MARGIN);

  onMount(() => {
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    copyButton.focus();
    return () => {
      if (before?.isConnected) before.focus();
    };
  });

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      copied = "Copied";
    } catch (e) {
      copied = "Copy failed; select the code instead";
      console.error(e);
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key !== "Tab") return;
    const buttons = [...panel.querySelectorAll<HTMLElement>("button")];
    const at = buttons.indexOf(document.activeElement as HTMLElement);
    e.preventDefault();
    buttons[(at + (e.shiftKey ? -1 : 1) + buttons.length) % buttons.length]?.focus();
  }
</script>

<svelte:window onkeydown={key} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="panel" role="dialog" aria-modal="true" aria-labelledby={`${id}-title`} bind:this={panel}>
    <h4 id={`${id}-title`}>{title}</h4>
    <svg class="qr" viewBox={`0 0 ${box} ${box}`} role="img" aria-label={`QR code for ${code}`} shape-rendering="crispEdges">
      <rect width={box} height={box} class="paper" />
      <path d={qrPath(qr.dark)} class="ink" />
    </svg>
    <p class="code">{code}</p>
    <p class="hint">Anyone with Hairline can paste this code</p>
    <div class="actions">
      {#if copied}<span class="lab" role="status">{copied}</span>{/if}
      <button class="btn" onclick={onclose}>Close</button>
      <button class="btn fill" bind:this={copyButton} onclick={copy}>Copy</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    background: color-mix(in srgb, var(--paper) 72%, transparent);
  }

  .panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    max-width: 360px;
    padding: 20px 22px 18px;
    border: 1px solid var(--ink);
    background: var(--paper);
    color: var(--ink);
  }

  h4 {
    align-self: flex-start;
    margin: 0 0 14px;
    font-size: 15px;
    font-weight: 600;
  }

  .qr {
    width: 180px;
    height: 180px;
  }

  .qr .paper {
    fill: var(--paper);
  }

  .qr .ink {
    fill: var(--ink);
  }

  .code {
    margin: 12px 0 0;
    font-family: "DM Mono", monospace;
    font-size: 20px;
    letter-spacing: 0.08em;
    -webkit-user-select: text;
    user-select: text;
  }

  .hint {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 12px;
  }

  .actions {
    display: flex;
    gap: 6px;
    align-items: center;
    align-self: stretch;
    justify-content: flex-end;
    margin-top: 16px;
  }
</style>
