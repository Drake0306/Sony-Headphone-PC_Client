<!--
  Overview: the photo in the device's colourway on the left; battery, noise,
  level, ULT, equalizer and codec on the right, then the link list.
-->
<script lang="ts">
  import Boxes from "../lib/controls/Boxes.svelte";
  import Headphone from "../lib/controls/Headphone.svelte";
  import LinkList from "../lib/controls/LinkList.svelte";
  import Row from "../lib/controls/Row.svelte";
  import Ticks from "../lib/controls/Ticks.svelte";
  import { control } from "../lib/control.svelte";
  import { setUlt } from "../lib/ipc";
  import * as noise from "../lib/noise";
  import {
    autoOffLabel,
    codecLabel,
    colourway,
    isPlaying,
    modelNumber,
    presetName,
    type DeviceState,
    type NcAsmState,
    type NcMode,
    type UltLevel,
  } from "../lib/state";

  let { st }: { st: DeviceState } = $props();

  const NOISE: { label: string; value: NcMode }[] = [
    { label: "NC", value: "NoiseCancelling" },
    { label: "AMB", value: "Ambient" },
    { label: "OFF", value: "Off" },
  ];
  const ULT: { label: string; value: UltLevel; orange: boolean }[] = [
    { label: "OFF", value: "Off", orange: true },
    { label: "ULT 1", value: "Ult1", orange: true },
    { label: "ULT 2", value: "Ult2", orange: true },
  ];
  const ULT_ARG: Record<UltLevel, 0 | 1 | 2> = { Off: 0, Ult1: 1, Ult2: 2 };
  const ULT_NOTE: Record<UltLevel, string> = { Off: "", Ult1: "deep bass", Ult2: "deeper bass" };

  const nc = control<NcAsmState>(() => st.nc);
  const ult = control<UltLevel>(() => ({ value: st.eq_ult.value?.ult ?? null, status: st.eq_ult.status }));

  const ncShown = $derived(nc.shown);
  const battery = $derived(st.battery.value);
  const eq = $derived(st.eq_ult.value);
  const top = $derived(Math.max(1, (st.eq_capability?.steps ?? 21) - 1));
  const onOff = (v: boolean | null) => (v === null ? "--" : v ? "On" : "Off");
  const links = $derived([
    {
      label: "This Mac",
      value: !st.connected ? "--" : isPlaying(st.playback) ? "▶ Playing" : "Idle",
      live: st.connected && isPlaying(st.playback),
    },
    { label: "Multipoint", value: onOff(st.multipoint.value) },
    { label: "Auto off", value: autoOffLabel(st.auto_power_off.value) || "--" },
    { label: "Wear pause", value: onOff(st.wear_pause.value) },
  ]);
  const caption = $derived([modelNumber(st.model), colourway(st.color)].filter(Boolean).join(" · "));
</script>

<div class="ov">
  <Headphone color={st.color} {caption} name={st.model ?? ""} />
  <div class="right">
    <div class="rows">
      <Row kind="overview" label="Battery">
        {#if battery}
          <span class="dot big">{battery[0]}<small>%</small></span>
        {:else}
          <span class="dot big">--</span>
        {/if}
        {#snippet aside()}{battery?.[1] === "Charging" ? "charging" : ""}{/snippet}
      </Row>
      <Row kind="overview" label="Noise">
        <Boxes
          options={NOISE}
          value={ncShown?.mode ?? null}
          attempted={nc.refused?.value.mode ?? null}
          status={nc.status}
          width="214px"
          onpick={(mode) => noise.pickNoise(nc, st.nc.value, mode)}
        />
        {#snippet aside()}{ncShown?.mode === "Ambient"
            ? `voice focus ${ncShown.voice_focus ? "on" : "off"}`
            : ""}{/snippet}
      </Row>
      <Row kind="overview" label="Level">
        <Ticks
          count={20}
          value={ncShown?.level ?? null}
          status={nc.status}
          disabled={ncShown?.mode !== "Ambient"}
          label="Ambient level"
          onpick={(level) => noise.pickLevel(nc, st.nc.value, level)}
        />
        {#snippet aside()}<span class="dot num" class:off={ncShown?.mode !== "Ambient"}>{ncShown?.level ?? "--"}</span>{/snippet}
      </Row>
      <Row kind="overview" label="ULT">
        <Boxes
          options={ULT}
          value={ult.shown}
          attempted={ult.refused?.value ?? null}
          status={ult.status}
          width="214px"
          onpick={(level) => ult.request(level, () => setUlt(ULT_ARG[level]))}
        />
        {#snippet aside()}{ult.shown ? ULT_NOTE[ult.shown] : ""}{/snippet}
      </Row>
      <Row kind="overview" label="Equalizer">
        <div class="faders" class:unavailable={!eq}>
          {#if eq}
            {#each eq.bands as band, i (i)}
              <div class:cb={i === 0}><div class="track"><i style:bottom={`${(band / top) * 90}%`}></i></div></div>
            {/each}
          {:else}
            {#each [0, 1, 2, 3, 4, 5] as i (i)}<div><div class="track"></div></div>{/each}
          {/if}
        </div>
        {#snippet aside()}{eq ? presetName(eq.preset) : ""}{/snippet}
      </Row>
      <Row kind="overview" label="Codec">
        <span class="dot codec">{codecLabel(st.codec.value) || "--"}</span>
        {#snippet aside()}{st.dsee.value === null ? "" : `DSEE ${st.dsee.value ? "on" : "off"}`}{/snippet}
      </Row>
    </div>
    <div class="links"><LinkList items={links} /></div>
  </div>
</div>

<style>
  .ov {
    display: grid;
    grid-template-columns: 1fr 1.1fr;
    gap: 28px;
    height: 100%;
    padding: 44px 32px 24px;
  }

  .right {
    min-width: 0;
  }

  /* The rows sit flush with the photo's top edge and keep their last rule. */
  .rows > :global(.r:first-child) {
    padding-top: 0;
  }

  .rows > :global(.r:last-child) {
    border-bottom: 1px solid var(--hair);
  }

  .links {
    margin-top: 14px;
  }

  .num {
    font-size: 20px;
    color: var(--ink);
  }

  .num.off {
    opacity: 0.35;
  }

  .codec {
    font-size: 18px;
  }

  .faders {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    height: 56px;
  }

  .faders.unavailable {
    opacity: 0.35;
  }

  .faders > div {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    height: 100%;
  }

  .track {
    position: relative;
    width: 3px;
    height: 100%;
    background: var(--hair);
  }

  .track i {
    position: absolute;
    left: -6px;
    width: 15px;
    height: 6px;
    background: var(--ink);
  }

  .cb .track i {
    background: var(--forest);
  }
</style>
