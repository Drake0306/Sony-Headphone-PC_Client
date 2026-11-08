<!-- The menu-bar popover (Phase 1 layout) on the shared controls. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Boxes from "../lib/controls/Boxes.svelte";
  import LinkList from "../lib/controls/LinkList.svelte";
  import Ticks from "../lib/controls/Ticks.svelte";
  import { connect, getRuntimeInfo, openMainWindow, openMainWindowOn, setUlt, watchState } from "../lib/ipc";
  import * as noise from "../lib/noise";
  import { displayStatus, pending, shownValue } from "../lib/pending";
  import { applyScene, hotkeyChips, modeBoxes, scenes, WINDOW_ERROR_KEY, WINDOW_SHORTCUT } from "../lib/scenes";
  import {
    codecLabel,
    colourway,
    emptyState,
    refusedReason,
    withFake,
    type DeviceState,
    type NcAsmState,
    type NcMode,
    type Status,
    type UltLevel,
  } from "../lib/state";
  import { visible as updateVisible } from "../lib/update";

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
  const mac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");
  // ⌥⌘N on a Mac, Alt+Super+N elsewhere (hotkeys.rs registers it).
  const windowKeys = hotkeyChips(WINDOW_SHORTCUT, mac).join(mac ? "" : "+");
  // Hidden when macOS refused it (Settings says why).
  const windowRefused = $derived($scenes.hotkey_errors?.[WINDOW_ERROR_KEY] !== undefined);
  const MORE = "more";
  const PENDING: Status = { Pending: { since_ms: 0 } };

  let st = $state<DeviceState>(emptyState());
  const nc = pending<NcAsmState>();
  const ult = pending<UltLevel>();
  // A scene applied here is pending until the next "scenes" event.
  const mode = pending<string>();
  let lastSeen = $state<{ at: number; battery: number | null } | null>(null);
  let now = $state(Date.now());
  let fake = $state(false);

  const ncShown = $derived(shownValue($nc, st.nc.status, st.nc.value));
  const ncStatus = $derived(displayStatus($nc, st.nc.status));
  const ultShown = $derived(shownValue($ult, st.eq_ult.status, st.eq_ult.value?.ult ?? null));
  const ultStatus = $derived(displayStatus($ult, st.eq_ult.status));
  // Voice focus and the level never show the refusal outline; the Noise switch carries it.
  const followerStatus = $derived(refusedReason(ncStatus) !== null ? "Confirmed" : ncStatus);
  const subtitle = $derived(
    withFake([colourway(st.color), codecLabel(st.codec.value)].filter((part) => part !== "").join(" · "), fake),
  );
  const battery = $derived(st.battery.value?.[0] ?? null);
  const devices = $derived(
    st.devices.map((d) => ({ label: d.name, value: d.playing ? "▶ Playing" : "Idle", live: d.playing })),
  );

  const boxes = $derived(modeBoxes($scenes.scenes));
  const modeOptions = $derived.by(() => {
    const { shown, more } = boxes;
    return [
      ...shown.map((s) => ({ label: s.name, value: s.id, title: s.name })),
      ...(more ? [{ label: "…", value: MORE, title: "All scenes" }] : []),
    ];
  });
  const modeStatus = $derived<Status>(
    $mode.want !== null ? PENDING : $mode.refused ? { Refused: { reason: $mode.refused.reason } } : "Confirmed",
  );

  $effect(() => {
    void $scenes;
    mode.receive("Confirmed");
  });

  function pickMode(id: string) {
    if (id === MORE) {
      openMainWindowOn("scenes").catch((e) => console.error(e));
      return;
    }
    // A failed apply is shown as refused on that box (held for a few seconds).
    mode.request(id, () =>
      applyScene(id).catch((e) => mode.receive({ Refused: { reason: typeof e === "string" ? e : String(e) } })),
    );
  }

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
    <div class="row"><span class="name">{st.model ?? ""}</span><span class="lab">{subtitle}</span></div>
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
        status={followerStatus}
        disabled={ncShown?.mode !== "Ambient"}
        label="Ambient level"
        onpick={pickLevel}
      />
    </div>
    {#if ncShown?.mode === "Ambient"}
      <div class="row">
        <span class="lab">Voice focus</span>
        <Boxes options={VOICE} value={ncShown.voice_focus} status={followerStatus} width="190px" onpick={pickVoice} />
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
    {@render modeRow()}
    {#if devices.length > 0}
      <div class="devices"><LinkList items={devices} /></div>
    {/if}
    {@render updateRow()}
    <button class="row footer" onclick={() => openMainWindow()}>
      <span class="lab">Open window</span>{#if !windowRefused}<span class="lab">{windowKeys}</span>{/if}
    </button>
  </div>
{:else}
  <div class="pop">
    <div class="row"><span class="name">{st.model ?? ""}</span><span class="lab">{withFake("Not connected", fake)}</span></div>
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
    {@render modeRow()}
    <div class="row hint">
      <span>Turn the headphones on, or reconnect from the Bluetooth menu. Controls return the moment they link.</span>
    </div>
    <div class="btnrow"><button class="btn fill" onclick={() => connect()}>Connect</button></div>
    {@render updateRow()}
  </div>
{/if}

{#snippet updateRow()}
  {#if $updateVisible}
    <button class="row footer update" onclick={() => openMainWindowOn("settings").catch((e) => console.error(e))}>
      <span class="lab">Update available · open Hairline</span>
    </button>
  {/if}
{/snippet}

{#snippet modeRow()}
  {#if boxes.shown.length > 0}
    <div class="row mode">
      <span class="lab">Mode</span>
      <Boxes
        options={modeOptions}
        value={$mode.want ?? $scenes.active}
        attempted={$mode.refused?.value ?? null}
        status={modeStatus}
        width="190px"
        onpick={pickMode}
      />
    </div>
  {/if}
{/snippet}

<style>
  .pop {
    width: 340px;
    background: var(--paper);
    border: 1px solid var(--hair);
    border-radius: 10px;
    padding: 14px 16px;
  }

  .mode :global(.boxes) {
    grid-auto-columns: minmax(0, 1fr);
  }

  .mode :global(.boxes button) {
    overflow: hidden;
    text-overflow: ellipsis;
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
</style>
