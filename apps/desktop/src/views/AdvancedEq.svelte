<!--
  The Advanced equalizer: a ten-band curve (31 Hz .. 16 kHz) stored in the
  app, edited four big faders at a time. Under the faders a strip of ten
  segments marks the visible four; ‹ and › step a page (1–4, 5–8, 7–10). Every
  settled edit saves the whole curve (`set_eq_curve`, never the headphones)
  and refreshes the six fitted device values shown beside "Send best fit to
  headphones"; during a drag they follow the local mirror of the fit. Only the
  Send button writes to the headphones, through the Sound screen's band write
  and the EQ group's status (`eq_ult`): pending, then "Sent" once the device
  reports the fitted bands, or the refusal. Takes the whole content area;
  "‹ Sound" goes back.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Fader, { faderText } from "../lib/controls/Fader.svelte";
  import FaderScale from "../lib/controls/FaderScale.svelte";
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
    toRaw,
    visibleBands,
  } from "../lib/eqCurve";
  import { getBestFitPreview, getEqCurve, sendBestFit, setEqCurve } from "../lib/ipc";
  import { isPending, type DeviceState } from "../lib/state";

  let { st, onback }: { st: DeviceState; onback: () => void } = $props();

  const NOTE =
    "These bands run in Hairline only. The headphones keep their six-band curve; use Send best fit to put the closest match on them.";
  /** Top raw fader value; the curve's levels run LEVEL_MIN..LEVEL_MAX around its centre. */
  const SPAN = LEVEL_MAX - LEVEL_MIN;
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

  const saver = curveSaver(setEqCurve);
  const eq = control<number[]>(() => ({ value: st.eq_ult.value?.bands ?? null, status: st.eq_ult.status }));

  const visible = $derived(visibleBands(page));
  const busy = $derived(isPending(eq.status));
  const sendOff = $derived(
    !st.connected || st.eq_ult.status === "Unavailable" || st.eq_ult.value === null || curve === null || busy,
  );
  const same = (a: readonly number[] | null | undefined, b: readonly number[] | null) =>
    !!a && !!b && a.length === b.length && a.every((v, i) => v === b[i]);
  const sent = $derived(sentRaw !== null && st.eq_ult.status === "Confirmed" && same(st.eq_ult.value?.bands, sentRaw));
  const sendNote = $derived(
    eq.refused?.reason ?? (sent ? "Sent" : busy && sentRaw !== null ? "Sending" : null),
  );
  const levelText = (v: number | null | undefined) => faderText(v, 0);

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
    if (curve === null) return;
    const mine = edits;
    saver
      .save(curve)
      .then(() => getBestFitPreview())
      .then((stored) => {
        problem = null;
        if (mine === edits && Array.isArray(stored)) fit = stored;
      })
      .catch((e) => {
        problem = `Not saved: ${e}`;
        console.error(e);
      });
  }

  function go(step: number) {
    page = Math.min(PAGE_COUNT - 1, Math.max(0, page + step));
  }

  function send() {
    if (sendOff || curve === null) return;
    const bands = [...curve];
    sentRaw = bestFitPreview(bands).map(toRaw);
    // Save what is drawn first (a key step may not have settled yet), so the
    // fit sent is the fit shown.
    eq.request(sentRaw, async () => {
      await saver.save(bands);
      const levels = await sendBestFit();
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
  </div>

  <div class="faders" class:unavailable={curve === null}>
    <FaderScale max={SPAN} size="lg" />
    {#each visible as i (i)}
      <Fader
        value={curve === null ? null : toRaw(curve[i])}
        label={ADVANCED_BAND_LABELS[i]}
        status="Confirmed"
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
    <div>
      <div class="fit">
        {#each DEVICE_BAND_LABELS as label, k (label)}
          <div>
            <span class="lab">{label}</span>
            <span class="dot">{levelText(fit?.[k])}</span>
          </div>
        {/each}
      </div>
      <p class="hold">What the headphones will hold</p>
    </div>
    <div class="act">
      <p class="note live">{NOTE}</p>
      {#if problem}<p class="note live">{problem}</p>{/if}
      <div class="send">
        {#if sendNote}<span class="lab" class:live={eq.refused !== null}>{sendNote}</span>{/if}
        <button
          class="btn fill"
          class:unavailable={sendOff}
          aria-disabled={sendOff}
          tabindex={sendOff ? -1 : 0}
          onclick={send}>Send best fit to headphones</button
        >
      </div>
    </div>
  </div>
</div>

<style>
  .adv {
    display: flex;
    flex-direction: column;
    gap: 16px;
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

  .faders {
    display: flex;
    flex: 1;
    gap: 24px;
    align-items: stretch;
    min-height: 0;
  }

  .faders.unavailable {
    opacity: 0.35;
  }

  .strip {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .strip .arrow {
    padding: 3px 10px;
    font-size: 12px;
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

  .bottom {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 24px;
    align-items: end;
  }

  .fit {
    display: grid;
    grid-template-columns: repeat(6, auto);
    gap: 14px;
    justify-content: start;
  }

  .fit div {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .fit .dot {
    font-size: 16px;
    line-height: 18px;
  }

  .hold {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 11px;
  }

  .act {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-end;
    max-width: 360px;
  }

  .act .note {
    margin: 0;
    font-size: 12px;
    text-align: right;
  }

  .send {
    display: flex;
    gap: 10px;
    align-items: center;
  }
</style>
