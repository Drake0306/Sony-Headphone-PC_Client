<!--
  Sound: the full equalizer faders and the preset grid on the left; ULT, the
  codec box with the connection-quality switch, and DSEE on the right. Every
  value comes from DeviceState; faders, presets and ULT share the EQ group's
  status (`eq_ult`). On a fixed Sony preset the headphones apply the curve
  inside and report centre values, so the faders show those values read-only
  with a note; on Off they are disabled. Only Manual, Custom 1 and Custom 2
  take band writes.
-->
<script lang="ts">
  import Boxes from "../lib/controls/Boxes.svelte";
  import Faders from "../lib/controls/Faders.svelte";
  import Row from "../lib/controls/Row.svelte";
  import Toggle from "../lib/controls/Toggle.svelte";
  import { control } from "../lib/control.svelte";
  import { setConnectionMode, setDseeAutoOff, setEqBands, setEqPreset, setUlt } from "../lib/ipc";
  import {
    codecLabel,
    isPending,
    PRESET_LOCK_NOTES,
    presetLock,
    presetName,
    type DeviceState,
    type UltLevel,
  } from "../lib/state";

  // `onadvanced` opens the Advanced equalizer (Main swaps it in for this screen).
  let { st, onadvanced }: { st: DeviceState; onadvanced?: () => void } = $props();

  // Index 0 is Clear Bass (docs/hardware-validation.md).
  const LABELS = ["Clear Bass", "400 Hz", "1 kHz", "2.5 kHz", "6.3 kHz", "16 kHz"];
  const ULT: { label: string; value: UltLevel; orange: boolean }[] = [
    { label: "OFF", value: "Off", orange: true },
    { label: "ULT 1", value: "Ult1", orange: true },
    { label: "ULT 2", value: "Ult2", orange: true },
  ];
  const ULT_ARG: Record<UltLevel, 0 | 1 | 2> = { Off: 0, Ult1: 1, Ult2: 2 };
  const CONNECTION = [
    { label: "Sound quality", value: 0 },
    { label: "Stable connection", value: 1 },
  ];

  const eqStatus = () => st.eq_ult.status;
  const bands = control<number[]>(() => ({ value: st.eq_ult.value?.bands ?? null, status: eqStatus() }));
  const preset = control<number>(() => ({ value: st.eq_ult.value?.preset ?? null, status: eqStatus() }));
  const ult = control<UltLevel>(() => ({ value: st.eq_ult.value?.ult ?? null, status: eqStatus() }));
  const connection = control<number>(() => st.connection_mode);
  const dseeAutoOff = control<boolean>(() => st.dsee_auto_off);

  const max = $derived(Math.max(1, (st.eq_capability?.steps ?? 21) - 1));
  const bandCount = $derived(st.eq_capability?.bands ?? st.eq_ult.value?.bands.length ?? LABELS.length);
  const presets = $derived((st.eq_capability?.presets ?? []).map(([id]) => ({ label: presetName(id), value: id })));
  const eqOff = $derived(st.eq_ult.status === "Unavailable" || st.eq_ult.value === null);
  const eqBusy = $derived(isPending(bands.status));
  /** A fixed Sony preset (bands applied inside, not reported) or Off: the faders are read-only. */
  const lock = $derived(presetLock(preset.shown));
  /** Band writes (faders, Reset) only go to Manual, Custom 1 and Custom 2. */
  const bandsLocked = $derived(lock !== null);
  const codec = $derived(codecLabel(st.codec.value));
  const dsee = $derived(st.dsee.value);

  function reset() {
    if (eqOff || eqBusy || bandsLocked) return;
    const flat = Array.from({ length: bandCount }, () => max / 2);
    bands.request(flat, () => setEqBands(flat));
  }
</script>

<div class="page">
  <div class="two sound">
    <div>
      <div class="block">
        <div class="eqhead">
          <h4>Equalizer{preset.shown === null ? "" : ` · ${presetName(preset.shown)}`}{bandsLocked ? "" : " · drag a handle"}</h4>
          {#if onadvanced}
            <button class="btn" onclick={onadvanced}>Advanced</button>
          {/if}
        </div>
        <div class="eq">
          <Faders
            bands={bands.shown}
            labels={LABELS}
            status={bands.status}
            disabled={bandsLocked}
            {max}
            oncommit={(next) => bands.request(next, () => setEqBands(next))}
          />
        </div>
        {#if lock}
          <p class="note">{PRESET_LOCK_NOTES[lock]}</p>
        {/if}
      </div>
      <div class="block">
        <h4>Presets</h4>
        {#if presets.length > 0}
          <div class="presets">
            <Boxes
              options={presets}
              value={preset.shown}
              attempted={preset.refused?.value ?? null}
              status={preset.status}
              onpick={(id) => preset.request(id, () => setEqPreset(id))}
            />
          </div>
        {:else}
          <p class="note">Not reported by the headphones yet.</p>
        {/if}
        <div class="btnrow">
          <button class="btn unavailable" aria-disabled="true" tabindex="-1" title="Phase 4">Save as Custom 1</button>
          <button
            class="btn"
            class:unavailable={eqOff || eqBusy || bandsLocked}
            aria-disabled={eqOff || eqBusy || bandsLocked}
            tabindex={eqOff || eqBusy || bandsLocked ? -1 : 0}
            onclick={reset}>Reset</button
          >
        </div>
      </div>
    </div>
    <div>
      <div class="block">
        <h4>ULT</h4>
        <Boxes
          options={ULT}
          value={ult.shown}
          attempted={ult.refused?.value ?? null}
          status={ult.status}
          size="lg"
          onpick={(level) => ult.request(level, () => setUlt(ULT_ARG[level]))}
        />
        <p class="note">ULT 1 lifts the deep bass. ULT 2 adds power across the low range.</p>
      </div>
      <div class="block">
        <h4>Codec and connection</h4>
        <div class="codec">
          <span class="dot">{codec || "--"}</span>
          <span class="meta">{dsee === null ? "" : `DSEE ${dsee ? "on" : "off"}`}</span>
          {#if st.connected}<span class="lab live-tag">Live</span>{/if}
        </div>
        <div class="quality">
          <Boxes
            options={CONNECTION}
            value={connection.shown}
            attempted={connection.refused?.value ?? null}
            status={connection.status}
            onpick={(v) => connection.request(v, () => setConnectionMode(v))}
          />
        </div>
        <p class="note">Stable connection prefers a steadier link over sound quality.</p>
      </div>
      <div class="block">
        <h4>Upscaling</h4>
        <Row label="DSEE" note="Restores detail lost to compression">
          <Toggle readonly on={dsee} label="DSEE" />
        </Row>
        <!-- On is Auto, off is Off (UpscalingTypeAutoOff). -->
        <Row label="DSEE Extreme" note="Auto applies upscaling when it helps">
          <Toggle
            on={dseeAutoOff.shown}
            status={dseeAutoOff.status}
            label="DSEE Extreme"
            onpick={(on) => dseeAutoOff.request(on, () => setDseeAutoOff(on))}
          />
        </Row>
      </div>
    </div>
  </div>
</div>

<style>
  .sound {
    grid-template-columns: 1.35fr 1fr;
  }

  .eqhead {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .eqhead h4 {
    margin: 0;
  }

  .eqhead .btn {
    padding: 2px 8px;
  }

  .eq {
    height: 216px;
    padding-top: 6px;
  }

  .presets :global(.boxes) {
    grid-auto-flow: row;
    grid-template-columns: repeat(3, 1fr);
  }

  .presets :global(.boxes button) {
    padding: 5px 8px;
    text-align: left;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .presets :global(.boxes button:not(.on):not(.refused)) {
    border-color: var(--hair);
  }

  .block .note {
    font-size: 12px;
  }

  .codec {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 14px;
    align-items: center;
    padding: 10px 12px;
    border: 1px solid var(--ink);
  }

  .codec .dot {
    font-size: 26px;
    line-height: 1;
  }

  .codec .meta {
    color: var(--muted);
    font-size: 12px;
  }

  .codec .live-tag {
    color: var(--ink);
  }

  .quality {
    margin-top: 8px;
  }
</style>
