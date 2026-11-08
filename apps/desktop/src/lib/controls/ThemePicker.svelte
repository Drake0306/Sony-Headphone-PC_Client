<!--
  The appearance picker: one swatch per theme, painted with that theme's own
  tokens (the swatch carries `data-theme`, so tokens.css colours it), the name
  underneath. Status rules as the boxed switch: Pending inert, Refused
  outlined in the accent with the reason as a tooltip, Unavailable faded.
-->
<script lang="ts">
  import { isPending, refusedReason, type Status, type Theme } from "../state";
  import { THEMES } from "../theme";

  interface Props {
    value: Theme | null;
    status: Status;
    onpick: (t: Theme) => void;
    attempted?: Theme | null;
  }

  let { value, status, onpick, attempted = null }: Props = $props();

  const off = $derived(status === "Unavailable");
  const busy = $derived(isPending(status));
  const reason = $derived(refusedReason(status));
  const outlined = $derived(attempted ?? value);
</script>

<div class="picker" class:pending={busy} class:unavailable={off} role="radiogroup" aria-label="Appearance">
  {#each THEMES as theme (theme.value)}
    {@const refused = reason !== null && theme.value === outlined}
    <button
      type="button"
      role="radio"
      class:on={theme.value === value}
      class:refused
      aria-checked={theme.value === value}
      aria-disabled={off}
      title={refused ? reason : undefined}
      tabindex={off ? -1 : 0}
      onclick={off ? undefined : () => busy || onpick(theme.value)}
    >
      {#if theme.kind === "system"}
        <span class="swatch split"><span data-theme="paper"><i></i><b></b></span><span data-theme="ink"><i></i><b></b></span></span>
      {:else}
        <span class="swatch" data-theme={theme.value}><i></i><b></b></span>
      {/if}
      <span class="name">{theme.label}</span>
    </button>
  {/each}
</div>

<style>
  .picker {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
    gap: 6px;
  }

  button {
    display: grid;
    gap: 5px;
    justify-items: start;
    padding: 5px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 0;
    cursor: pointer;
    text-align: left;
  }

  button.on {
    border-color: var(--ink);
  }

  button.refused {
    border-color: var(--ult);
  }

  button.refused .name {
    color: var(--ult);
  }

  .pending button,
  .unavailable button {
    cursor: default;
  }

  .unavailable button {
    opacity: 0.35;
  }

  /* The swatch: the theme's paper, a line of its ink, a block of its accent. */
  .swatch {
    position: relative;
    display: block;
    width: 100%;
    height: 28px;
    background: var(--paper);
    border: 1px solid var(--hair);
    overflow: hidden;
  }

  .swatch i,
  .swatch b {
    position: absolute;
    display: block;
  }

  .swatch i {
    left: 6px;
    right: 6px;
    top: 9px;
    height: 2px;
    background: var(--ink);
  }

  .swatch b {
    left: 6px;
    bottom: 6px;
    width: 10px;
    height: 7px;
    background: var(--ult);
  }

  .swatch.split {
    display: grid;
    grid-template-columns: 1fr 1fr;
    background: none;
    border: none;
  }

  .swatch.split > span {
    position: relative;
    display: block;
    background: var(--paper);
    border: 1px solid var(--hair);
  }

  .swatch.split > span + span {
    border-left: none;
  }

  .name {
    font-family: "DM Mono", monospace;
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ink);
    white-space: nowrap;
  }
</style>
