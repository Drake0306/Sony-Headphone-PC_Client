<!--
  The full-size equalizer faders (Sound screen): a dB scale and one Fader per
  band. Raw band values run 0..max (centre max/2) and read as −10..+10. This
  component owns the draft and the commit: while a fader moves, its snapped
  value is a local draft (the knob and its value follow it); the fader's
  commit (pointer release, or 150 ms after the last key) sends the whole set
  once. Pending: faded and inert; a commit that lands while pending is queued
  and sent once the status settles, and a sent draft is held for as long as it
  stays pending. Refused: the reason is the container's tooltip. Unavailable
  or disabled: faded and inert, and a commit then drops the draft.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { isPending, refusedReason, type Status } from "../state";
  import Fader from "./Fader.svelte";
  import FaderScale from "./FaderScale.svelte";

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

  /** Without a device answer, a sent draft gives way to the props after this. */
  const DRAFT_HOLD_MS = 2000;

  const off = $derived(disabled || status === "Unavailable");
  const busy = $derived(isPending(status));
  const reason = $derived(refusedReason(status));

  let draft = $state<number[] | null>(null);
  /** The band being moved (drag or unsent key step), if any. */
  let active = $state<number | null>(null);
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let sentPending = false;
  /** A commit arrived while pending; send it once the status leaves Pending. */
  let queued = false;

  const shown = $derived(draft ?? bands);
  const count = $derived(bands?.length ?? labels.length);

  const clamp = (v: number) => Math.min(max, Math.max(0, Math.round(v)));
  const same = (a: number[] | null, b: number[] | null) =>
    a !== null && b !== null && a.length === b.length && a.every((v, i) => v === b[i]);

  // A sent draft stays drawn until the device answers: the props catch up,
  // a pending write settles, or a refusal arrives.
  $effect(() => {
    const b = bands;
    const p = busy;
    const r = reason;
    if (draft === null || active !== null) return;
    if (queued) {
      if (!p) untrack(commit);
      return;
    }
    if (same(draft, b) || r !== null || (sentPending && !p)) release();
    else if (p) sentPending = true;
  });

  // Leaving the screen drops the draft hold.
  $effect(() => () => clearTimeout(holdTimer));

  function release() {
    clearTimeout(holdTimer);
    holdTimer = undefined;
    sentPending = false;
    queued = false;
    draft = null;
  }

  // The hold never lets go while a write is still pending; the effect above
  // releases the draft once the device answers.
  function holdExpired() {
    holdTimer = busy || queued ? setTimeout(holdExpired, DRAFT_HOLD_MS) : undefined;
    if (holdTimer === undefined) release();
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
    if (off || bands === null || same(draft, bands)) {
      release();
      return;
    }
    if (busy) {
      queued = true;
      return;
    }
    queued = false;
    sentPending = false;
    clearTimeout(holdTimer);
    holdTimer = setTimeout(holdExpired, DRAFT_HOLD_MS);
    oncommit([...draft]);
  }

  function change(i: number, v: number) {
    active = i;
    set(i, v);
  }

  function settle(i: number, v: number) {
    set(i, v);
    active = null;
    commit();
  }
</script>

<div
  class="faders"
  class:pending={busy}
  class:refused={reason !== null}
  class:unavailable={off || bands === null}
  title={reason ?? undefined}
>
  <FaderScale {max} />
  {#each Array.from({ length: count }, (_, i) => i) as i (i)}
    <Fader
      value={shown?.[i] ?? null}
      label={labels[i] ?? ""}
      {status}
      disabled={off || bands === null}
      {max}
      band={i + 1}
      onchange={(v) => change(i, v)}
      oncommit={(v) => settle(i, v)}
    />
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
</style>
