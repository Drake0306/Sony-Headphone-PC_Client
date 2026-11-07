<!--
  The frame log: one mono line per wire frame, oldest at the top. Follows the
  newest frame unless the reader has scrolled up.
-->
<script lang="ts">
  import { frameLine, type FrameEntry } from "../advanced";

  interface Props {
    entries: FrameEntry[];
    /** The time shown as +0. */
    origin: number;
  }

  let { entries, origin }: Props = $props();

  let box = $state<HTMLDivElement>();
  // Whether the view sits at the bottom; scrolling up stops the follow.
  let follow = true;

  function onscroll() {
    if (box) follow = box.scrollHeight - box.scrollTop - box.clientHeight < 8;
  }

  $effect(() => {
    void entries;
    if (box && follow) box.scrollTop = box.scrollHeight;
  });
</script>

<div class="frames" bind:this={box} {onscroll}>
  {#each entries as e, i (i)}
    {@const line = frameLine(e, origin)}
    <div>
      <span class="t">{line.t}</span><span>{line.arrow}</span><span>{line.table}</span><span class="hex">{line.hex}</span>
    </div>
  {/each}
</div>

<style>
  .frames {
    height: calc(100vh - 200px);
    min-height: 160px;
    overflow-y: auto;
    border-top: 1px solid var(--hair);
    border-bottom: 1px solid var(--hair);
    font-family: "DM Mono", monospace;
    font-size: 11px;
    -webkit-user-select: text;
    user-select: text;
  }

  .frames div {
    display: grid;
    grid-template-columns: 72px 14px 22px 1fr;
    gap: 6px;
    padding: 2px 0;
  }

  .t {
    color: var(--muted);
  }

  .hex {
    overflow-wrap: anywhere;
  }
</style>
