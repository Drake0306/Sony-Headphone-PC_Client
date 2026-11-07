<!--
  An in-app confirmation (the webview has no alert/confirm): a paper panel
  with an ink border over a veiled window. A title, a body, optional
  label/value details (values in Doto), then Cancel and the confirm button.
  Focus starts on Cancel and stays inside the panel (Tab wraps); Escape or a
  click on the backdrop cancels. The owner unmounts it from either callback.
-->
<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    title: string;
    body: string;
    details?: { label: string; value: string }[];
    confirmLabel: string;
    onconfirm: () => void;
    oncancel: () => void;
  }

  let { title, body, details = [], confirmLabel, onconfirm, oncancel }: Props = $props();

  let panel: HTMLElement;
  let cancel: HTMLButtonElement;
  const id = `confirm-${Math.random().toString(36).slice(2)}`;

  onMount(() => cancel.focus());

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      oncancel();
      return;
    }
    if (e.key !== "Tab") return;
    const buttons = [...panel.querySelectorAll<HTMLElement>("button")];
    const at = buttons.indexOf(document.activeElement as HTMLElement);
    const next = (at + (e.shiftKey ? -1 : 1) + buttons.length) % buttons.length;
    e.preventDefault();
    buttons[next]?.focus();
  }
</script>

<svelte:window onkeydown={key} />

<!-- Escape is the keyboard path for a backdrop click (the window listener above). -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && oncancel()}>
  <div
    class="panel"
    role="dialog"
    aria-modal="true"
    aria-labelledby={`${id}-title`}
    aria-describedby={`${id}-body`}
    bind:this={panel}
  >
    <h4 id={`${id}-title`}>{title}</h4>
    <p id={`${id}-body`} class="body">{body}</p>
    {#if details.length > 0}
      <dl class="details">
        {#each details as d (d.label)}
          <div>
            <dt class="lab">{d.label}</dt>
            <dd class="dot">{d.value}</dd>
          </div>
        {/each}
      </dl>
    {/if}
    <div class="actions">
      <button class="btn" bind:this={cancel} onclick={oncancel}>Cancel</button>
      <button class="btn fill" onclick={onconfirm}>{confirmLabel}</button>
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
    width: 100%;
    max-width: 460px;
    padding: 20px 22px 18px;
    border: 1px solid var(--ink);
    background: var(--paper);
    color: var(--ink);
  }

  h4 {
    margin: 0 0 10px;
    font-size: 15px;
    font-weight: 600;
  }

  .body {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
  }

  .details {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 8px;
    margin: 14px 0 0;
    padding: 10px 0;
    border-top: 1px solid var(--hair);
    border-bottom: 1px solid var(--hair);
  }

  .details div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: center;
    text-align: center;
  }

  .details dt {
    color: var(--ink);
  }

  .details dd {
    margin: 0;
    font-size: 16px;
    line-height: 18px;
  }

  .actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 16px;
  }
</style>
