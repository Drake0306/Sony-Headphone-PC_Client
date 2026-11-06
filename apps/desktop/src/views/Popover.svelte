<!-- The menu-bar popover (Phase 1 layout) on the shared controls. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Boxes from "../lib/controls/Boxes.svelte";
  import LinkList from "../lib/controls/LinkList.svelte";
  import Ticks from "../lib/controls/Ticks.svelte";
  import { connect, getRuntimeInfo, openMainWindow, setUlt, watchState } from "../lib/ipc";
  import * as noise from "../lib/noise";
  import { displayStatus, pending, shownValue } from "../lib/pending";
  import {
    codecLabel,
    colourway,
    emptyState,
    refusedReason,
    withFake,
    type DeviceState,
    type NcAsmState,
    type NcMode,
    type UltLevel,
  } from "../lib/state";

  const NOISE: { label: string; value: NcMode }[] = [
    { label: "NC", value: "NoiseCancelling" },
    { label: "AMB", value: "Ambient" },
    { label: "OFF", value: "Off" },
  ];
  // Phase 1 draws every active ULT box orange, OFF included.
  const ULT: { label: string; value: UltLevel; orange: boolean }[] = [
    { label: "OFF", value: "Off", orange: true },
    { label: "1", value: "Ult1", orange: true },
    { label: "2", value: "Ult2", orange: true },
  ];
  const ULT_ARG: Record<UltLevel, 0 | 1 | 2> = { Off: 0, Ult1: 1, Ult2: 2 };
  const VOICE: { label: string; value: boolean }[] = [
    { label: "ON", value: true },
    { label: "OFF", value: false },
  ];
  const noop = () => {};

  let st = $state<DeviceState>(emptyState());
  const nc = pending<NcAsmState>();
  const ult = pending<UltLevel>();
  let lastSeen = $state<{ at: number; battery: number | null } | null>(null);
  let now = $state(Date.now());
  let fake = $state(false);

  const ncShown = $derived(shownValue($nc, st.nc.status, st.nc.value));
  const ncStatus = $derived(displayStatus($nc, st.nc.status));
  const ultShown = $derived(shownValue($ult, st.eq_ult.status, st.eq_ult.value?.ult ?? null));
  const ultStatus = $derived(displayStatus($ult, st.eq_ult.status));
  // Voice focus never shows the refusal outline; the Noise switch carries it.
  const voiceStatus = $derived(refusedReason(ncStatus) !== null ? "Confirmed" : ncStatus);
  const subtitle = $derived(
    withFake([colourway(st.color), codecLabel(st.codec.value)].filter((part) => part !== "").join(" · "), fake),
  );
  const battery = $derived(st.battery.value?.[0] ?? null);
  const devices = $derived(
    st.devices.map((d) => ({ label: d.name, value: d.playing ? "▶ Playing" : "Idle", live: d.playing })),
  );

  function receive(next: DeviceState) {
    if (st.connected && !next.connected) {
      lastSeen = { at: Date.now(), battery: st.battery.value?.[0] ?? null };
    }
    nc.receive(next.nc.status);
    ult.receive(next.eq_ult.status);
    st = next;
  }

  const pickNoise = (mode: NcMode) => noise.pickNoise(nc, st.nc.value, mode);
  const pickLevel = (level: number) => noise.pickLevel(nc, st.nc.value, level);
  const pickVoice = (on: boolean) => noise.pickVoice(nc, ncShown, on);

  function pickUlt(level: UltLevel) {
    ult.request(level, () => setUlt(ULT_ARG[level]));
  }

  function ago(at: number): string {
    const minutes = Math.floor((now - at) / 60000);
    if (minutes < 1) return "just now";
    if (minutes < 60) return minutes === 1 ? "1 minute ago" : `${minutes} minutes ago`;
    const hours = Math.floor(minutes / 60);
    return hours === 1 ? "1 hour ago" : `${hours} hours ago`;
  }

  onMount(() => {
    const stop = watchState(receive);
    getRuntimeInfo()
      .then((info) => (fake = info.fake))
      .catch((e) => console.error(e));
    const clock = setInterval(() => (now = Date.now()), 30000);
    return () => {
      clearInterval(clock);
      stop();
    };
  });
</script>

{#if st.connected}
  <div class="pop">
    <div class="row"><span class="name">{st.model ?? "ULT WEAR"}</span><span class="lab">{subtitle}</span></div>
    <div class="row">
      <span class="lab">Battery</span>
      {#if battery !== null}
        <span class="dot big">{battery}<small>%</small></span>
      {:else}
        <span class="dot big">--</span>
      {/if}
    </div>
    <div class="row">
      <span class="lab">Noise</span>
      <Boxes
        options={NOISE}
        value={ncShown?.mode ?? null}
        attempted={$nc.refused?.value.mode ?? null}
        status={ncStatus}
        width="190px"
        onpick={pickNoise}
      />
    </div>
    <div class="row">
      <span class="lab">Level {ncShown?.level ?? "--"}</span>
      <Ticks
        count={20}
        value={ncShown?.level ?? null}
        status={ncStatus}
        disabled={ncShown?.mode !== "Ambient"}
        label="Ambient level"
        onpick={pickLevel}
      />
    </div>
    {#if ncShown?.mode === "Ambient"}
      <div class="row">
        <span class="lab">Voice focus</span>
        <Boxes options={VOICE} value={ncShown.voice_focus} status={voiceStatus} width="190px" onpick={pickVoice} />
      </div>
    {/if}
    <div class="row">
      <span class="lab">ULT</span>
      <Boxes
        options={ULT}
        value={ultShown}
        attempted={$ult.refused?.value ?? null}
        status={ultStatus}
        width="190px"
        onpick={pickUlt}
      />
    </div>
    {#if devices.length > 0}
      <div class="devices"><LinkList items={devices} /></div>
    {/if}
    <button class="row footer" onclick={() => openMainWindow()}>
      <span class="lab">Open window</span><span class="lab">⌥⌘N</span>
    </button>
  </div>
{:else}
  <div class="pop">
    <div class="row"><span class="name">{st.model ?? "ULT WEAR"}</span><span class="lab">{withFake("Not connected", fake)}</span></div>
    {#if lastSeen}
      <div class="row">
        <span class="lab">Last seen</span>
        <span class="small">{ago(lastSeen.at)}{lastSeen.battery !== null ? ` · ${lastSeen.battery}%` : ""}</span>
      </div>
    {/if}
    <div class="row">
      <span class="lab">Noise</span>
      <Boxes options={NOISE} value={null} status="Unavailable" width="190px" onpick={noop} />
    </div>
    <div class="row">
      <span class="lab">ULT</span>
      <Boxes options={ULT} value={null} status="Unavailable" width="190px" onpick={noop} />
    </div>
    <div class="row hint">
      <span>Turn the headphones on, or reconnect from the Bluetooth menu. Controls return the moment they link.</span>
    </div>
    <div class="btnrow"><button class="btn fill" onclick={() => connect()}>Connect</button></div>
  </div>
{/if}

<style>
  .pop {
    width: 340px;
    background: var(--paper);
    border: 1px solid var(--hair);
    border-radius: 10px;
    padding: 14px 16px;
  }

  .name {
    font-weight: 600;
  }

  .devices {
    margin-top: 4px;
  }

  .footer {
    width: 100%;
    background: none;
    border: none;
    padding-top: 10px;
    cursor: pointer;
  }

  .small {
    font-size: 12px;
  }

  .hint {
    display: block;
    padding-top: 12px;
    font-size: 12px;
    color: var(--muted);
  }

  .btnrow {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 8px;
  }
</style>
