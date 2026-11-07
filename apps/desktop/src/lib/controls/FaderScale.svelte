<!--
  The dB scale beside a row of faders: "+10", "0" and "−10" on the top, zero
  and bottom steps of the rails. It is its own column (never inside a rail or
  a fader column), sized to its text, so the end labels are never clipped.
  The bottom spacer matches Fader's foot so the marks line up with the rails.
-->
<script lang="ts">
  import { faderText, faderVars, type FaderSize } from "./Fader.svelte";

  let { max = 20, size = "md" }: { max?: number; size?: FaderSize } = $props();
  const centre = $derived(max / 2);
</script>

<div class="fader-scale" aria-hidden="true" style={faderVars(size)}>
  <div class="ends">
    <b class="lab">{faderText(max, centre)}</b>
    <b class="lab">{faderText(centre, centre)}</b>
    <b class="lab">{faderText(0, centre)}</b>
  </div>
  <div class="foot"></div>
</div>

<style>
  .fader-scale {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--fader-gap);
    height: 100%;
  }

  /* Each mark is one knob tall, so space-between puts the first, middle and
     last mark centres on the top, zero and bottom step centres. */
  .ends {
    display: flex;
    flex: 1;
    flex-direction: column;
    justify-content: space-between;
    white-space: nowrap;
    text-align: right;
  }

  .ends b {
    height: var(--fader-knob);
    font-weight: 400;
    line-height: var(--fader-knob);
  }

  .foot {
    flex: none;
    height: var(--fader-foot);
  }
</style>
