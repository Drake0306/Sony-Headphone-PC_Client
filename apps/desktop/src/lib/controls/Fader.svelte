<!--
  One equalizer fader: a track with one tick per step (max + 1, 21 by
  default) and a stronger tick at zero, an ink fill from zero to the knob, an
  orange knob, and the value in Doto above the band name. Raw values run
  0..max (centre max/2) and read as −10..+10. The fader is controlled: it
  reports a snapped step through `onchange` while the user moves it and
  `oncommit` when they finish (pointer release, or 150 ms after the last key).
  Pointer positions snap to the nearest step; ↑/↓ step by one, Shift by five,
  Home/End go to the ends. Pending, unavailable or disabled: inert.
-->
<script module lang="ts">
  /** A raw step as dB text: "+5", "0", "−5" (a real minus sign), "--" for none. */
  export function faderText(v: number | null | undefined, centre: number): string {
    if (v === null || v === undefined) return "--";
    const db = v - centre;
    return db > 0 ? `+${db}` : db < 0 ? `−${-db}` : "0";
  }
</script>

<script lang="ts">
  import { isPending, type Status } from "../state";

  interface Props {
    /** Raw step, 0..=max; null before the device has reported it. */
    value: number | null;
    label: string;
    status: Status;
    disabled?: boolean;
    /** A new snapped step while the user moves the fader. */
    onchange: (v: number) => void;
    /** The step the user settled on. */
    oncommit: (v: number) => void;
    /** Top raw value (steps − 1); the centre is max / 2. */
    max?: number;
  }

  let { value, label, status, disabled = false, onchange, oncommit, max = 20 }: Props = $props();

  const KEY_COMMIT_MS = 150;
  /** Knob height in px; the knob's centre travels the rail minus this. */
  const KNOB = 10;
  const BIG_STEP = 5;

  const centre = $derived(max / 2);
  const inert = $derived(disabled || status === "Unavailable" || isPending(status) || value === null);
  const steps = $derived(Array.from({ length: max + 1 }, (_, k) => k));

  let dragging = $state(false);
  let keyTimer: ReturnType<typeof setTimeout> | undefined;
  /** The last step reported, so a release commits what was drawn. */
  let last: number | null = null;

  /** Where step k's centre sits, measured from the rail's bottom. */
  const at = (k: number) => `calc(${KNOB / 2}px + ${k / max} * (100% - ${KNOB}px))`;
  const span = (n: number) => `calc(${n / max} * (100% - ${KNOB}px))`;
  const snap = (v: number) => Math.min(max, Math.max(0, Math.round(v)));

  // Leaving the screen drops an unsent key step.
  $effect(() => () => clearTimeout(keyTimer));

  function change(v: number) {
    last = snap(v);
    onchange(last);
  }

  function fromPointer(rail: Element, y: number) {
    const r = rail.getBoundingClientRect();
    const travel = Math.max(1, r.height - KNOB);
    return ((r.bottom - KNOB / 2 - y) / travel) * max;
  }

  function down(e: PointerEvent) {
    if (inert || e.button > 0) return;
    const rail = e.currentTarget as HTMLElement;
    try {
      rail.setPointerCapture?.(e.pointerId);
    } catch {
      // jsdom and synthetic events have no active pointer to capture.
    }
    dragging = true;
    rail.querySelector<HTMLElement>("[role=slider]")?.focus();
    change(fromPointer(rail, e.clientY));
    e.preventDefault();
  }

  // A drag that began before the fader went inert still finishes, so the
  // owner can queue or drop it.
  function move(e: PointerEvent) {
    if (dragging) change(fromPointer(e.currentTarget as Element, e.clientY));
  }

  function up() {
    if (!dragging) return;
    dragging = false;
    oncommit(last ?? snap(value ?? centre));
  }

  function target(e: KeyboardEvent, v: number): number | null {
    const big = e.shiftKey ? BIG_STEP : 1;
    switch (e.key) {
      case "ArrowUp":
      case "ArrowRight":
        return v + big;
      case "ArrowDown":
      case "ArrowLeft":
        return v - big;
      case "PageUp":
        return v + BIG_STEP;
      case "PageDown":
        return v - BIG_STEP;
      case "Home":
        return 0;
      case "End":
        return max;
      default:
        return null;
    }
  }

  function key(e: KeyboardEvent) {
    if (inert || value === null) return;
    const next = target(e, value);
    if (next === null) return;
    e.preventDefault();
    change(next);
    clearTimeout(keyTimer);
    keyTimer = setTimeout(() => {
      keyTimer = undefined;
      oncommit(last ?? snap(next));
    }, KEY_COMMIT_MS);
  }
</script>

<div class="fader" class:inert>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="rail"
    class:dragging
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
  >
    <div class="track"></div>
    {#each steps as k (k)}
      <span class="seg" class:zero={k === centre} style:bottom={at(k)}></span>
    {/each}
    {#if value !== null && value !== centre}
      <div
        class="fill"
        class:neg={value < centre}
        style:bottom={at(Math.min(value, centre))}
        style:height={span(Math.abs(value - centre))}
      ></div>
    {/if}
    {#if value !== null}
      <i
        class="knob"
        role="slider"
        tabindex={inert ? -1 : 0}
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={`${faderText(value, centre)} dB`}
        aria-disabled={inert}
        style:bottom={span(value)}
        onkeydown={key}
      ></i>
    {/if}
  </div>
  <div class="foot">
    <span class="val dot">{faderText(value, centre)}</span>
    <span class="name lab">{label}</span>
  </div>
</div>

<style>
  .fader {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    min-width: 0;
    height: 100%;
  }

  .rail {
    position: relative;
    flex: 1;
    width: 100%;
    max-width: 44px;
    cursor: ns-resize;
    touch-action: none;
  }

  .fader.inert .rail {
    cursor: default;
  }

  .track {
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: calc(50% - 3px);
    width: 6px;
    background: var(--hair);
  }

  /* One tick per step, centred on the step; zero is longer and darker. */
  .seg {
    position: absolute;
    left: calc(50% - 8px);
    width: 16px;
    height: 1px;
    transform: translateY(50%);
    background: var(--hair);
    pointer-events: none;
  }

  .seg.zero {
    left: calc(50% - 14px);
    width: 28px;
    background: var(--muted);
  }

  .fill {
    position: absolute;
    left: calc(50% - 3px);
    width: 6px;
    background: var(--ink);
    pointer-events: none;
    transition:
      bottom 120ms ease-out,
      height 120ms ease-out;
  }

  .knob {
    position: absolute;
    left: calc(50% - 12px);
    width: 24px;
    height: 10px;
    background: var(--ult);
    transition: bottom 120ms ease-out;
  }

  .rail.dragging .knob,
  .rail.dragging .fill {
    transition: none;
  }

  :global(body.kbd) .knob:focus-visible {
    outline: 1px solid var(--ink);
    outline-offset: 2px;
  }

  .knob:focus {
    outline: none;
  }

  /* Fixed height so every rail (and the scale beside them) lines up. */
  .foot {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    height: 48px;
  }

  .val {
    font-size: 18px;
    line-height: 20px;
    color: var(--ink);
    white-space: nowrap;
  }

  /* Two lines for every name, bottom-aligned, so "Clear Bass" may wrap. */
  .name {
    display: flex;
    align-items: flex-end;
    justify-content: center;
    height: 26px;
    line-height: 13px;
    text-align: center;
  }
</style>
