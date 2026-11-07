<!--
  One equalizer fader: a track with one tick per step (max + 1, 21 by
  default) and a stronger tick at zero, an ink fill from zero to the knob, an
  orange knob, and the value in Doto above the band name. Raw values run
  0..max (centre max/2) and read as −10..+10. The fader is controlled: it
  reports a snapped step through `onchange` while the user moves it and
  `oncommit` when they finish (pointer release, or 150 ms after the last key).
  Pointer positions snap to the nearest step; ↑/↓ step by one, Shift by five,
  Home/End go to the ends. Pending, unavailable or disabled: inert. Unmounting
  commits a key step that has not settled yet.
-->
<script module lang="ts">
  /** A raw step as dB text: "+5", "0", "−5" (a real minus sign), "--" for none. */
  export function faderText(v: number | null | undefined, centre: number): string {
    if (v === null || v === undefined) return "--";
    const db = v - centre;
    return db > 0 ? `+${db}` : db < 0 ? `−${-db}` : "0";
  }

  /**
   * Fader dimensions in px, defined once: the knob height also drives the
   * pointer maths, and FaderScale uses the knob and foot so its marks line up.
   * "md" is the Sound screen, "lg" the Advanced equalizer's big faders.
   */
  export const FADER_SIZES = {
    md: { knob: 10, knobWidth: 24, track: 6, seg: 16, zero: 28, rail: 44, gap: 6, foot: 48, value: 18 },
    lg: { knob: 14, knobWidth: 44, track: 8, seg: 26, zero: 48, rail: 88, gap: 8, foot: 58, value: 28 },
  } as const;
  export type FaderSize = keyof typeof FADER_SIZES;

  /** The sizes as CSS custom properties (`--fader-knob: 10px`, ...). */
  export function faderVars(size: FaderSize): string {
    return Object.entries(FADER_SIZES[size])
      .map(([k, v]) => `--fader-${k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}: ${v}px`)
      .join("; ");
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
    /** "md" (Sound screen) or "lg" (Advanced equalizer); the height follows the parent. */
    size?: FaderSize;
  }

  let { value, label, status, disabled = false, onchange, oncommit, max = 20, size = "md" }: Props = $props();

  const KEY_COMMIT_MS = 150;
  /** Knob height in px; the knob's centre travels the rail minus this. */
  const KNOB = $derived(FADER_SIZES[size].knob);
  const BIG_STEP = 5;

  const centre = $derived(max / 2);
  const inert = $derived(disabled || status === "Unavailable" || isPending(status) || value === null);
  const steps = $derived(Array.from({ length: max + 1 }, (_, k) => k));

  let dragging = $state(false);
  let keyTimer: ReturnType<typeof setTimeout> | undefined;
  /** The last step reported, so a release commits what was drawn. */
  let last: number | null = null;
  /** The key step waiting for its commit timer. */
  let keyStep: number | null = null;

  /** Where step k's centre sits, measured from the rail's bottom. */
  const at = (k: number) => `calc(${KNOB / 2}px + ${k / max} * (100% - ${KNOB}px))`;
  const span = (n: number) => `calc(${n / max} * (100% - ${KNOB}px))`;
  const snap = (v: number) => Math.min(max, Math.max(0, Math.round(v)));

  // Leaving the screen commits an unsent key step at once rather than drop it.
  $effect(() => () => {
    if (keyTimer === undefined) return;
    clearTimeout(keyTimer);
    keyTimer = undefined;
    if (keyStep !== null) oncommit(keyStep);
  });

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
    keyStep = last ?? snap(next);
    clearTimeout(keyTimer);
    keyTimer = setTimeout(() => {
      keyTimer = undefined;
      if (keyStep !== null) oncommit(keyStep);
      keyStep = null;
    }, KEY_COMMIT_MS);
  }
</script>

<div class="fader" class:inert style={faderVars(size)}>
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
        style:bottom={at(Math.min(value, centre))}
        style:height={span(Math.abs(value - centre))}
      ></div>
    {/if}
    {#if value !== null}
      <i
        class="knob"
        role="slider"
        aria-orientation="vertical"
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
    gap: var(--fader-gap);
    min-width: 0;
    height: 100%;
  }

  .rail {
    position: relative;
    flex: 1;
    width: 100%;
    max-width: var(--fader-rail);
    cursor: ns-resize;
    touch-action: none;
  }

  .fader.inert .rail {
    cursor: default;
  }

  .track {
    position: absolute;
    top: calc(var(--fader-knob) / 2);
    bottom: calc(var(--fader-knob) / 2);
    left: calc(50% - var(--fader-track) / 2);
    width: var(--fader-track);
    background: var(--hair);
  }

  /* One tick per step, centred on the step; zero is longer and darker. */
  .seg {
    position: absolute;
    left: calc(50% - var(--fader-seg) / 2);
    width: var(--fader-seg);
    height: 1px;
    transform: translateY(50%);
    background: var(--hair);
    pointer-events: none;
  }

  .seg.zero {
    left: calc(50% - var(--fader-zero) / 2);
    width: var(--fader-zero);
    background: var(--muted);
  }

  .fill {
    position: absolute;
    left: calc(50% - var(--fader-track) / 2);
    width: var(--fader-track);
    background: var(--ink);
    pointer-events: none;
    transition:
      bottom 120ms ease-out,
      height 120ms ease-out;
  }

  .knob {
    position: absolute;
    left: calc(50% - var(--fader-knob-width) / 2);
    width: var(--fader-knob-width);
    height: var(--fader-knob);
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
    height: var(--fader-foot);
  }

  .val {
    font-size: var(--fader-value);
    line-height: calc(var(--fader-value) + 2px);
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
