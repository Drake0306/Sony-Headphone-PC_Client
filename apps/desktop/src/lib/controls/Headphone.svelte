<!--
  The Overview photo block: the headphone silhouette on a panel in the
  device's own colourway. These tints are the only colour literals outside
  tokens.css: they describe the hardware, not the theme.
-->
<script lang="ts">
  interface Props {
    /** Device info colour byte: 1 Black, 2 Off White, 8 Forest Gray. */
    color: number | null;
    /** Small caption, e.g. "WH-ULT900N · Forest Gray". */
    caption?: string;
    /** Large name under the caption. */
    name?: string;
  }

  let { color, caption, name }: Props = $props();

  const TINTS: Record<number, string> = {
    1: "--panel:#2a2a28;--cup:#161614;--text:#efede6",
    2: "--panel:#d9d6cc;--cup:#efede6;--text:#161614",
    8: "--panel:#4a5550;--cup:#efede6;--text:#efede6",
  };
  const tint = $derived(TINTS[color ?? 8] ?? TINTS[8]);
</script>

<div class="photo" style={tint}>
  <svg width="220" height="190" viewBox="0 0 64 56" aria-hidden="true">
    <path
      d="M8 36V28C8 14.7 18.7 4 32 4s24 10.7 24 24v8"
      fill="none"
      stroke="currentColor"
      stroke-width="4"
      stroke-linecap="round"
    />
    <rect x="4" y="30" width="14" height="22" rx="5" fill="currentColor" />
    <rect x="46" y="30" width="14" height="22" rx="5" fill="currentColor" />
  </svg>
  {#if caption || name}
    <div class="cap">
      {#if caption}<span class="lab">{caption}</span>{/if}
      {#if name}<b class="dot">{name}</b>{/if}
    </div>
  {/if}
</div>

<style>
  .photo {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 0;
    height: 100%;
    border-radius: 6px;
    background: var(--panel);
    color: var(--cup);
  }

  /* Scales down in a short panel so the caption stays clear. */
  svg {
    max-width: 70%;
    max-height: 60%;
  }

  .cap {
    position: absolute;
    left: 14px;
    bottom: 12px;
    color: var(--text);
  }

  .cap .lab {
    color: inherit;
    opacity: 0.75;
  }

  .cap b {
    display: block;
    font-size: 22px;
  }
</style>
