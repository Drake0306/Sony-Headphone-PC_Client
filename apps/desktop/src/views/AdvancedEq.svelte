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
    shapeProblem,
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
  /** Best fit writes six values of 21 steps; other headphones cannot take it. */
  const shape = $derived(shapeProblem(st.eq_capability));
  const sendOff = $derived(
    !st.connected ||
      st.eq_ult.status === "Unavailable" ||
      st.eq_ult.value === null ||
      curve === null ||
      busy ||
      shape !== null,
  );
  const same = (a: readonly number[] | null | undefined, b: readonly number[] | null) =>
    !!a && !!b && a.length === b.length && a.every((v, i) => v === b[i]);
  const sent = $derived(sentRaw !== null && st.eq_ult.status === "Confirmed" && same(st.eq_ult.value?.bands, sentRaw));
  const sendNote = $derived(
    eq.refused?.reason ?? (sent ? "Sent" : busy && sentRaw !== null ? "Sending" : null),
  );
  const levelText = (v: number | null | undefined) => faderText(v, 0);
  /** The headphones' six levels now, from state; null while disconnected or unknown. */
  const onDevice = $derived(st.connected && st.eq_ult.value ? st.eq_ult.value.bands.map(fromRaw) : null);

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

  function go(step: number) {
    page = Math.min(PAGE_COUNT - 1, Math.max(0, page + step));
  }

  function send() {
    if (sendOff || curve === null) return;
    // The drawn curve goes with the request (saved, fitted and sent by the
    // command), so the fit sent is the fit shown even if a key step has not
    // settled. The saver gets it too, so a save still queued cannot end on an
    // older curve. The faders stay inert while the send is pending.
    const bands = [...curve];
    sentRaw = bestFitPreview(bands).map(toRaw);
    saver.save(bands).catch((e) => console.error(e));
    eq.request(sentRaw, async () => {
      const levels = await sendBestFit(bands);
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
    <div>
      <!-- One column per device value: label, the headphones now, the best fit. -->
      <div class="fit">
        <div class="r">
          <span></span>
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
      <p class="hold">What the headphones will hold</p>
    </div>
    <div class="act">
      <p class="note live">{NOTE}</p>
      {#if shape}<p class="note err">{shape}</p>{/if}
      {#if problem}<p class="note err">{problem}</p>{/if}
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

  .faders.pending {
    opacity: 0.6;
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

  /* Values and the Send block side by side; the Send block wraps under when narrow. */
  .bottom {
    display: flex;
    flex-wrap: wrap;
    gap: 16px 28px;
    align-items: flex-end;
    justify-content: space-between;
  }

  .fit {
    display: grid;
    grid-template-columns: auto repeat(6, auto);
    gap: 4px 14px;
    align-items: baseline;
    justify-content: start;
  }

  .fit .r {
    display: contents;
  }

  .fit .dev {
    color: var(--ink);
    text-align: center;
  }

  .fit .dot {
    font-size: 16px;
    line-height: 18px;
    text-align: center;
  }

  .fit .now .dot {
    color: var(--muted);
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
    margin-left: auto;
    max-width: 360px;
  }

  .act .note {
    margin: 0;
    font-size: 12px;
    text-align: right;
  }

  /* Problems read as muted text; orange is kept for the note and refusals. */
  .act .note.err {
    color: var(--muted);
  }

  .send {
    display: flex;
    gap: 10px;
    align-items: center;
  }
</style>
