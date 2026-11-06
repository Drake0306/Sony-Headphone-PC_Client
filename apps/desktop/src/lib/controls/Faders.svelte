<!--
  The full-size equalizer faders (Sound screen). Raw band values run 0..max
  (centre max/2) and read as -10..+10. A drag keeps a local draft (the handle
  and its value follow the pointer) and commits once on release; ↑/↓ on a
  focused handle step by one and commit 150 ms after the last key. Pending:
  faded and inert. Refused: the reason is the container's tooltip.
  Unavailable or disabled: faded and inert.
-->
<script lang="ts">
  import { isPending, refusedReason, type Status } from "../state";

  interface Props {
    /** Raw band values, 0..=max; null before the device has reported them. */
    bands: number[] | null;
    labels: string[];
    status: Status;
    disabled?: boolean;
    oncommit: (bands: number[]) => void;
    /** Top raw value (steps − 1); the centre is max / 2. */
    max?: number;
  }

  let { bands, labels, status, disabled = false, oncommit, max = 20 }: Props = $props();

  const KEY_COMMIT_MS = 150;
  /** Handle height in px; the handle's centre travels the rail minus this. */
  const HANDLE = 10;
  /** Without a device answer, a sent draft gives way to the props after this. */
  const DRAFT_HOLD_MS = 2000;

  const off = $derived(disabled || status === "Unavailable");
  const busy = $derived(isPending(status));
  const reason = $derived(refusedReason(status));
  const inert = $derived(off || busy || bands === null);
  const centre = $derived(max / 2);

  let draft = $state<number[] | null>(null);
  let dragging = $state<number | null>(null);
  let keyTimer: ReturnType<typeof setTimeout> | undefined;
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let sentPending = false;

  const shown = $derived(draft ?? bands);
  const count = $derived(bands?.length ?? labels.length);

  const clamp = (v: number) => Math.min(max, Math.max(0, Math.round(v)));
  const same = (a: number[] | null, b: number[] | null) =>
    a !== null && b !== null && a.length === b.length && a.every((v, i) => v === b[i]);
  const label = (v: number | undefined) => {
    if (v === undefined) return "--";
    const db = v - centre;
    return db > 0 ? `+${db}` : `${db}`;
  };

  // A sent draft stays drawn until the device answers: the props catch up,
  // a pending write settles, or a refusal arrives.
  $effect(() => {
    const b = bands;
    const p = busy;
    const r = reason;
    if (draft === null || dragging !== null || keyTimer !== undefined) return;
    if (same(draft, b) || r !== null || (sentPending && !p)) release();
    else if (p) sentPending = true;
  });

  // Leaving the screen drops an unsent key step and the draft hold.
  $effect(() => () => {
    clearTimeout(keyTimer);
    clearTimeout(holdTimer);
  });

  function release() {
    clearTimeout(holdTimer);
    holdTimer = undefined;
    sentPending = false;
    draft = null;
  }

  function set(i: number, v: number) {
    const base = draft ?? bands;
    if (!base) return;
    const next = clamp(v);
    if (base[i] === next) {
      draft = draft ?? [...base];
      return;
    }
    draft = base.map((x, j) => (j === i ? next : x));
  }

  function commit() {
    if (draft === null) return;
    if (same(draft, bands)) {
      release();
      return;
    }
    sentPending = false;
    clearTimeout(holdTimer);
    holdTimer = setTimeout(release, DRAFT_HOLD_MS);
    oncommit([...draft]);
  }

  function fromPointer(rail: Element, y: number) {
    const r = rail.getBoundingClientRect();
    const travel = Math.max(1, r.height - HANDLE);
    return ((r.bottom - HANDLE / 2 - y) / travel) * max;
  }

  function down(i: number, e: PointerEvent) {
    if (inert || e.button > 0) return;
    const rail = e.currentTarget as HTMLElement;
    try {
      rail.setPointerCapture?.(e.pointerId);
    } catch {
      // jsdom and synthetic events have no active pointer to capture.
    }
    dragging = i;
    rail.querySelector<HTMLElement>("[role=slider]")?.focus();
    set(i, fromPointer(rail, e.clientY));
    e.preventDefault();
  }

  function move(i: number, e: PointerEvent) {
    if (dragging !== i) return;
    set(i, fromPointer(e.currentTarget as Element, e.clientY));
  }

  function up(i: number) {
    if (dragging !== i) return;
    dragging = null;
    commit();
  }

  function key(i: number, e: KeyboardEvent) {
    if (inert) return;
    const step = e.key === "ArrowUp" ? 1 : e.key === "ArrowDown" ? -1 : 0;
    if (step === 0) return;
    e.preventDefault();
    const base = draft ?? bands;
    if (!base) return;
    set(i, base[i] + step);
    clearTimeout(keyTimer);
    keyTimer = setTimeout(() => {
      keyTimer = undefined;
      commit();
    }, KEY_COMMIT_MS);
  }
</script>

<div
  class="faders"
  class:pending={busy}
  class:refused={reason !== null}
  class:unavailable={off || bands === null}
  title={reason ?? undefined}
>
  {#each Array.from({ length: count }, (_, i) => i) as i (i)}
    {@const v = shown?.[i]}
    <div class="band" class:cb={i === 0}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="rail"
        class:dragging={dragging === i}
        onpointerdown={(e) => down(i, e)}
        onpointermove={(e) => move(i, e)}
        onpointerup={() => up(i)}
        onpointercancel={() => up(i)}
      >
        <div class="track"></div>
        {#if i === 0}<b class="mk top">+{centre}</b><b class="mk bottom">-{centre}</b>{/if}
        {#if v !== undefined}
          <i
            role="slider"
            tabindex={inert ? -1 : 0}
            aria-label={labels[i]}
            aria-valuemin={0}
            aria-valuemax={max}
            aria-valuenow={v}
            aria-valuetext={`${label(v)} dB`}
            aria-disabled={inert}
            style:bottom={`calc(${v / max} * (100% - ${HANDLE}px))`}
            onkeydown={(e) => key(i, e)}
          ></i>
        {/if}
      </div>
      <span class="lab name">{labels[i] ?? ""}</span>
      <span class="lab val">{label(v)}</span>
    </div>
  {/each}
</div>

<style>
  .faders {
    display: flex;
    gap: 14px;
    align-items: stretch;
    height: 100%;
  }

  .faders.pending {
    opacity: 0.6;
  }

  .faders.unavailable {
    opacity: 0.35;
  }

  .band {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .rail {
    position: relative;
    flex: 1;
    width: 100%;
    max-width: 40px;
    cursor: ns-resize;
    touch-action: none;
  }

  .faders.pending .rail,
  .faders.unavailable .rail {
    cursor: default;
  }

  .track {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(50% - 1.5px);
    width: 3px;
    background: var(--hair);
  }

  .rail i {
    position: absolute;
    left: calc(50% - 11.5px);
    width: 23px;
    height: 10px;
    background: var(--ink);
    transition: bottom 120ms ease-out;
  }

  .rail.dragging i {
    transition: none;
  }

  .cb .rail i {
    background: var(--forest);
  }

  :global(body.kbd) .rail i:focus-visible {
    outline: 1px solid var(--ink);
    outline-offset: 2px;
  }

  .rail i:focus {
    outline: none;
  }

  .mk {
    position: absolute;
    left: calc(50% - 24px);
    width: 48px;
    text-align: center;
    font-family: "DM Mono", monospace;
    font-size: 9px;
    font-weight: 400;
    color: var(--muted);
    pointer-events: none;
  }

  .mk.top {
    top: -2px;
  }

  .mk.bottom {
    bottom: -2px;
  }

  .lab {
    text-align: center;
    white-space: nowrap;
  }

  /* Two lines for every name, bottom-aligned, so "Clear Bass" may wrap and
     every rail keeps the same height. */
  .name {
    display: flex;
    align-items: flex-end;
    justify-content: center;
    height: 26px;
    line-height: 13px;
    white-space: normal;
  }

  .val {
    color: var(--ink);
  }
</style>
