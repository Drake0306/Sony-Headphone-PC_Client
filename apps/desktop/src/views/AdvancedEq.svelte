<!--
  The Advanced equalizer: a ten-band curve (31 Hz .. 16 kHz) stored in the
  app, edited four big faders at a time. Under the faders a strip of ten
  segments marks the visible four; ‹ and › step a page (1–4, 5–8, 7–10). Every
  settled edit saves the whole curve (`set_eq_curve`, never the headphones)
  and refreshes the six fitted device values in the mini table beside the
  actions; during a drag they follow the local mirror of the fit. Reset sets
  the ten bands to 0 the same way. Only Send writes to the headphones, and
  only after a confirmation that names the target preset (Manual, Custom 1 or
  Custom 2 as they are; any other preset goes to Custom 1) and the six values:
  one EQ write of that preset and the fitted bands, ULT kept, with the EQ
  group's status (`eq_ult`): pending, then "Sent" once the device reports the
  fitted bands, or the refusal. Takes the whole content area, sized so nothing
  scrolls at 880×560; "‹ Sound" goes back. Share and Paste a code, at the
  right of the top row: Share shows the curve as a code and QR; Paste takes one (an Advanced code replaces and saves the
  curve; a Sound code goes to the headphones only after its confirmation).
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Fader, { faderText } from "../lib/controls/Fader.svelte";
  import Confirm from "../lib/controls/Confirm.svelte";
  import FaderScale from "../lib/controls/FaderScale.svelte";
  import PasteCode from "../lib/controls/PasteCode.svelte";
  import ShareCode from "../lib/controls/ShareCode.svelte";
  import { control } from "../lib/control.svelte";
  import {
    ADVANCED_BAND_LABELS,
    BAND_COUNT,
    bestFitPreview,
    curveSaver,
    DEVICE_BAND_LABELS,
    fromRaw,
    LEVEL_MAX,
    LEVEL_MIN,
    PAGE_COUNT,
    resetCurve,
    shapeProblem,
    targetPreset,
    targetProblem,
    toRaw,
    visibleBands,
    type TargetPreset,
  } from "../lib/eqCurve";
  import { getBestFitPreview, getEqCurve, sendBestFit, setEqCurve } from "../lib/ipc";
  import { encodeAdvanced } from "../lib/presetCode";
  import { isPending, PRESET_LOCK_NOTES, presetLock, type DeviceState } from "../lib/state";

  let { st, onback }: { st: DeviceState; onback: () => void } = $props();

  const NOTE =
    "These bands run in SHDC only. The headphones keep their six-band curve; use Send best fit to put the closest match on them.";
  /** Top raw fader value; the curve's levels run LEVEL_MIN..LEVEL_MAX around its centre. */
  const SPAN = LEVEL_MAX - LEVEL_MIN;
  const CONFIRM_TITLE = "Send best fit to the headphones?";
  const confirmBody = (target: string) =>
    `The headphones hold six bands (${DEVICE_BAND_LABELS.join(", ")}), not ten. SHDC fits your ten bands onto those six and writes them into ${target}.`;
  const ALL = Array.from({ length: BAND_COUNT }, (_, i) => i);

  /** Levels −10..=10; null until the stored curve has been read. */
  let curve = $state<number[] | null>(null);
  /** The six fitted levels: the local mirror during edits, the store's answer after a save. */
  let fit = $state<number[] | null>(null);
  let problem = $state<string | null>(null);
  let page = $state(0);
  /** The raw bands last sent, until the curve changes again. */
  let sentRaw = $state<number[] | null>(null);
  /** Orders the store's fit answers so an older one never replaces a newer edit. */
  let edits = 0;
  /** The send waiting for the confirmation: the curve drawn when it opened and where it goes. */
  let asking = $state<{ bands: number[]; fit: number[]; target: TargetPreset } | null>(null);

  const saver = curveSaver(setEqCurve);
  const eq = control<number[]>(() => ({ value: st.eq_ult.value?.bands ?? null, status: st.eq_ult.status }));

  const visible = $derived(visibleBands(page));
  const busy = $derived(isPending(eq.status));
  /** Best fit writes six values of 21 steps; other headphones cannot take it. */
  const shape = $derived(shapeProblem(st.eq_capability));
  /** Where Send would write, and why it cannot (the headphones do not list that preset). */
  const target = $derived(targetPreset(st.eq_ult.value?.preset ?? null));
  const unlisted = $derived(targetProblem(target, st.eq_capability));
  /** A fixed Sony preset or Off: the headphones do not report its bands. */
  const lock = $derived(st.connected ? presetLock(st.eq_ult.value?.preset ?? null) : null);
  const sendOff = $derived(
    !st.connected ||
      st.eq_ult.status === "Unavailable" ||
      st.eq_ult.value === null ||
      curve === null ||
      busy ||
      shape !== null ||
      unlisted !== null,
  );
  const same = (a: readonly number[] | null | undefined, b: readonly number[] | null) =>
    !!a && !!b && a.length === b.length && a.every((v, i) => v === b[i]);
  const sent = $derived(sentRaw !== null && st.eq_ult.status === "Confirmed" && same(st.eq_ult.value?.bands, sentRaw));
  const sendNote = $derived(
    eq.refused?.reason ?? (sent ? "Sent" : busy && sentRaw !== null ? "Sending" : null),
  );
  const levelText = (v: number | null | undefined) => faderText(v, 0);
  const resetOff = $derived(curve === null || busy);
  /** The headphones' six levels now, from state; null while disconnected, unknown, or on a fixed preset or Off. */
  const onDevice = $derived(
    st.connected && st.eq_ult.value && lock === null ? st.eq_ult.value.bands.map(fromRaw) : null,
  );

  onMount(() => {
    getEqCurve()
      .then((r) => {
        if (curve !== null || !r || !Array.isArray(r.bands) || r.bands.length !== BAND_COUNT) return;
        curve = [...r.bands];
        fit = bestFitPreview(curve);
      })
      .catch((e) => {
        problem = "The saved curve could not be read.";
        console.error(e);
      });
  });

  function change(i: number, raw: number) {
    if (curve === null) return;
    const level = fromRaw(raw);
    if (curve[i] !== level) {
      curve = curve.map((v, j) => (j === i ? level : v));
      sentRaw = null;
    }
    edits++;
    fit = bestFitPreview(curve);
  }

  function commit(i: number, raw: number) {
    change(i, raw);
    persist();
  }

  /** Saves the drawn curve, then shows the store's fit for it. */
  function persist() {
    if (curve === null) return;
    const mine = edits;
    saver.save(curve).then(
      () =>
        getBestFitPreview().then(
          (stored) => {
            problem = null;
            if (mine === edits && Array.isArray(stored)) fit = stored;
          },
          (e) => {
            // Saved; the fit shown stays the local mirror.
            problem = "Saved; preview unavailable";
            console.error(e);
          },
        ),
      (e) => {
        problem = `Not saved: ${e}`;
        console.error(e);
      },
    );
  }

  /** All ten bands back to 0, saved in the app; nothing goes to the headphones. */
  function reset() {
    if (resetOff) return;
    curve = resetCurve();
    sentRaw = null;
    edits++;
    fit = bestFitPreview(curve);
    persist();
  }

  let sharing = $state<string | null>(null);
  let pasting = $state(false);

  function share() {
    if (curve !== null) sharing = encodeAdvanced(curve);
  }

  /**
   * A pasted curve: saved first, then drawn with its fit, as an edit would
   * be. A failed save leaves the drawn curve alone, sets the note and rejects
   * (the paste panel then says it was not saved).
   */
  function adopt(bands: number[]): Promise<void> {
    const next = [...bands];
    return saver.save(next).then(
      () => {
        curve = next;
        sentRaw = null;
        edits++;
        fit = bestFitPreview(next);
        problem = null;
        const mine = edits;
        getBestFitPreview().then(
          (stored) => {
            if (mine === edits && Array.isArray(stored)) fit = stored;
          },
          (e) => {
            problem = "Saved; preview unavailable";
            console.error(e);
          },
        );
      },
      (e) => {
        problem = `Not saved: ${e}`;
        throw e;
      },
    );
  }

  function go(step: number) {
    page = Math.min(PAGE_COUNT - 1, Math.max(0, page + step));
  }

  /** Opens the confirmation; nothing is sent until it is confirmed. */
  function ask() {
    if (sendOff || curve === null) return;
    const bands = [...curve];
    asking = { bands, fit: bestFitPreview(bands), target };
  }

  function send() {
    const req = asking;
    asking = null;
    if (req === null || sendOff) return;
    // The curve drawn when the confirmation opened goes with the request
    // (saved, fitted and sent by the command), so the fit sent is the fit
    // shown. The saver gets it too, so a save still queued cannot end on an
    // older curve. The faders stay inert while the send is pending.
    const { bands, fit: shown, target } = req;
    sentRaw = shown.map(toRaw);
    saver.save(bands).catch((e) => console.error(e));
    eq.request(sentRaw, async () => {
      const levels = await sendBestFit(bands, target.id);
      if (Array.isArray(levels)) sentRaw = levels.map(toRaw);
      return levels;
    });
  }
</script>

<div class="page adv">
  <div class="top">
    <button class="btn" onclick={onback}>‹ Sound</button>
    <h3>Advanced equalizer</h3>
    <span class="lab range">Bands {visible[0] + 1}–{visible[visible.length - 1] + 1} of {BAND_COUNT}</span>
    <div class="codes">
      <button
        class="btn"
        class:unavailable={curve === null}
        aria-disabled={curve === null}
        tabindex={curve === null ? -1 : 0}
        onclick={share}>Share</button
      >
      <button class="btn" onclick={() => (pasting = true)}>Paste a code</button>
    </div>
  </div>

  <div class="faders" class:pending={busy} class:unavailable={curve === null}>
    <FaderScale max={SPAN} size="lg" />
    {#each visible as i (i)}
      <Fader
        value={curve === null ? null : toRaw(curve[i])}
        label={ADVANCED_BAND_LABELS[i]}
        status={busy ? eq.status : "Confirmed"}
        max={SPAN}
        size="lg"
        onchange={(v) => change(i, v)}
        oncommit={(v) => commit(i, v)}
      />
    {/each}
  </div>

  <div class="strip">
    <button
      class="btn arrow"
      class:unavailable={page === 0}
      aria-label="Previous bands"
      aria-disabled={page === 0}
      onclick={() => go(-1)}>‹</button
    >
    <div class="bands" aria-hidden="true">
      {#each ALL as i (i)}
        <span class:on={visible.includes(i)}></span>
      {/each}
    </div>
    <button
      class="btn arrow"
      class:unavailable={page === PAGE_COUNT - 1}
      aria-label="Next bands"
      aria-disabled={page === PAGE_COUNT - 1}
      onclick={() => go(1)}>›</button
    >
  </div>

  <div class="bottom">
    <div class="values">
      <!-- One column per device value: label, the headphones now, the best fit. -->
      <div class="fit">
        <div class="r">
          <span class="lab hold">What the headphones will hold</span>
          {#each DEVICE_BAND_LABELS as label (label)}<span class="lab dev">{label}</span>{/each}
        </div>
        <div class="r now">
          <span class="lab">On the headphones now</span>
          {#each DEVICE_BAND_LABELS as label, k (label)}<span class="dot">{levelText(onDevice?.[k])}</span>{/each}
        </div>
        <div class="r best">
          <span class="lab">Best fit</span>
          {#each DEVICE_BAND_LABELS as label, k (label)}<span class="dot">{levelText(fit?.[k])}</span>{/each}
        </div>
      </div>
      {#if lock}<p class="note">{PRESET_LOCK_NOTES[lock]}</p>{/if}
    </div>
    <div class="act">
      <p class="note live">{NOTE}</p>
      {#if shape}<p class="note err">{shape}</p>{/if}
      {#if unlisted}<p class="note err">{unlisted}</p>{/if}
      {#if problem}<p class="note err">{problem}</p>{/if}
      <div class="send">
        {#if sendNote}<span class="lab" class:live={eq.refused !== null}>{sendNote}</span>{/if}
        <button
          class="btn"
          class:unavailable={resetOff}
          aria-disabled={resetOff}
          tabindex={resetOff ? -1 : 0}
          onclick={reset}>Reset</button
        >

        <button
          class="btn fill"
          class:unavailable={sendOff}
          aria-disabled={sendOff}
          tabindex={sendOff ? -1 : 0}
          onclick={ask}>Send best fit to headphones</button
        >
      </div>
    </div>
  </div>
</div>

{#if asking}
  <Confirm
    title={CONFIRM_TITLE}
    body={confirmBody(asking.target.label)}
    details={DEVICE_BAND_LABELS.map((label, k) => ({ label, value: levelText(asking?.fit[k]) }))}
    confirmLabel="Send"
    onconfirm={send}
    oncancel={() => (asking = null)}
  />
{/if}

{#if sharing}
  <ShareCode title="Share this curve" code={sharing} onclose={() => (sharing = null)} />
{/if}
{#if pasting}
  <PasteCode {st} savecurve={adopt} onclose={() => (pasting = false)} />
{/if}

<style>
  /* Height at 880×560 (no scroll): 20 + top ≈26 + 8 + faders (the rest,
     ≈385) + 8 + strip 17 + 8 + bottom ≈60–73 + 16 (Share and Paste a code
     sit in the top row, so the bottom stays one row). Each rail takes the
     faders' height less the 58 px foot and 8 px gap: ≈320. */
  .adv {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 20px 32px 16px;
  }

  .top {
    display: flex;
    gap: 14px;
    align-items: baseline;
  }

  .top h3 {
    margin: 0;
  }

  .top .range {
    margin-left: auto;
  }

  /* Share and Paste a code, at the right of the top row (the bottom row has no room). */
  .codes {
    display: flex;
    gap: 6px;
    align-self: center;
  }

  .codes .btn {
    padding: 2px 8px;
  }

  /* The faders take every free pixel; each rail fills its column. */
  .faders {
    display: flex;
    flex: 1;
    gap: 32px;
    align-items: stretch;
    min-height: 300px;
  }

  .faders.pending {
    opacity: 0.6;
  }

  .faders.unavailable {
    opacity: 0.35;
  }

  .strip {
    display: flex;
    flex: none;
    gap: 10px;
    align-items: center;
  }

  .strip .arrow {
    padding: 2px 10px;
    font-size: 11px;
    line-height: 1;
  }

  .bands {
    display: grid;
    flex: 1;
    grid-template-columns: repeat(10, 1fr);
    gap: 3px;
  }

  .bands span {
    height: 8px;
    background: var(--hair);
  }

  .bands span.on {
    background: var(--ult);
  }

  /* One compact row: the mini table on the left, the note and buttons on the right. */
  .bottom {
    display: flex;
    flex: none;
    gap: 24px;
    align-items: flex-end;
    justify-content: space-between;
  }

  /* The mini table, and under it the fixed-preset or Off note. */
  .values {
    flex: none;
    max-width: 500px;
  }

  .values .note {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 14px;
  }

  /* Seven columns: row label, then one per device value (labels may wrap to two lines). */
  .fit {
    display: grid;
    flex: none;
    grid-template-columns: auto repeat(6, minmax(36px, auto));
    gap: 3px 10px;
    align-items: end;
    font-family: "DM Mono", monospace;
    font-size: 11px;
    line-height: 14px;
  }

  .fit .r {
    display: contents;
  }

  .fit .lab {
    line-height: 13px;
  }

  .fit .now .lab,
  .fit .best .lab {
    white-space: nowrap;
  }

  .fit .hold {
    max-width: 150px;
  }

  .fit .dev {
    color: var(--ink);
    text-align: center;
  }

  .fit .dot {
    font-family: inherit;
    font-weight: 500;
    text-align: center;
  }

  .fit .now .dot {
    color: var(--muted);
  }

  .act {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 6px;
    align-items: flex-end;
    min-width: 0;
    max-width: 420px;
  }

  .act .note {
    margin: 0;
    font-size: 11px;
    line-height: 14px;
    text-align: right;
  }

  /* Problems read as muted text; orange is kept for the note and refusals. */
  .act .note.err {
    color: var(--muted);
  }

  .send {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
