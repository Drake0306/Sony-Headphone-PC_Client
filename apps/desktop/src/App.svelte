<script lang="ts">
  import { onMount } from "svelte";
  import { connect, getState, onState, openMainWindow, setNc, setUlt, type NcModeArg } from "./lib/ipc";
  import {
    codecLabel,
    colourway,
    emptyState,
    isPending,
    refusedReason,
    type DeviceState,
    type NcAsmState,
    type NcMode,
    type UltLevel,
  } from "./lib/state";

  const view = new URLSearchParams(location.search).get("view") === "main" ? "main" : "popover";

  const NOISE: { label: string; mode: NcMode; arg: NcModeArg }[] = [
    { label: "NC", mode: "NoiseCancelling", arg: "nc" },
    { label: "AMB", mode: "Ambient", arg: "ambient" },
    { label: "OFF", mode: "Off", arg: "off" },
  ];
  const ULT: { label: string; level: UltLevel; arg: 0 | 1 | 2 }[] = [
    { label: "OFF", level: "Off", arg: 0 },
    { label: "1", level: "Ult1", arg: 1 },
    { label: "2", level: "Ult2", arg: 2 },
  ];
  const VOICE: { label: string; on: boolean }[] = [
    { label: "ON", on: true },
    { label: "OFF", on: false },
  ];
  const TICKS = Array.from({ length: 20 }, (_, i) => i + 1);
  const REFUSED_SHOWN_MS = 4000;

  let st = $state<DeviceState>(emptyState());
  // What the user just asked for, shown (striped) while the device has not answered.
  let wantNc = $state<NcAsmState | null>(null);
  let wantUlt = $state<UltLevel | null>(null);
  // The last refused request, outlined in orange with its reason for a few seconds.
  let ncRefused = $state<{ mode: NcMode; reason: string } | null>(null);
  let ultRefused = $state<{ level: UltLevel; reason: string } | null>(null);
  let lastSeen = $state<{ at: number; battery: number | null } | null>(null);
  let now = $state(Date.now());

  const ncShown = $derived(isPending(st.nc.status) && wantNc ? wantNc : st.nc.value);
  const ultShown = $derived(isPending(st.eq_ult.status) && wantUlt ? wantUlt : (st.eq_ult.value?.ult ?? null));
  const ncOff = $derived(st.nc.status === "Unavailable");
  // The level bar only applies in Ambient; elsewhere it shows the remembered level, inert.
  const levelOff = $derived(ncOff || ncShown?.mode !== "Ambient");
  const ultOff = $derived(st.eq_ult.status === "Unavailable");
  const subtitle = $derived(
    [colourway(st.color), codecLabel(st.codec.value)].filter((part) => part !== "").join(" · "),
  );
  const battery = $derived(st.battery.value?.[0] ?? null);

  function receive(next: DeviceState) {
    if (st.connected && !next.connected) {
      lastSeen = { at: Date.now(), battery: st.battery.value?.[0] ?? null };
    }
    const ncReason = refusedReason(next.nc.status);
    if (ncReason !== null && wantNc) {
      const refused = { mode: wantNc.mode, reason: ncReason };
      ncRefused = refused;
      setTimeout(() => ncRefused === refused && (ncRefused = null), REFUSED_SHOWN_MS);
    }
    if (!isPending(next.nc.status)) wantNc = null;
    const ultReason = refusedReason(next.eq_ult.status);
    if (ultReason !== null && wantUlt) {
      const refused = { level: wantUlt, reason: ultReason };
      ultRefused = refused;
      setTimeout(() => ultRefused === refused && (ultRefused = null), REFUSED_SHOWN_MS);
    }
    if (!isPending(next.eq_ult.status)) wantUlt = null;
    st = next;
  }

  const clampLevel = (level: number | undefined) => Math.min(20, Math.max(1, level ?? 1));

  function requestNc(arg: NcModeArg, next: NcAsmState) {
    wantNc = next;
    ncRefused = null;
    setNc(arg, next.level, next.voice_focus).catch((e) => {
      wantNc = null;
      console.error(e);
    });
  }

  function pickNoise(choice: (typeof NOISE)[number]) {
    const cur = st.nc.value;
    requestNc(choice.arg, { mode: choice.mode, level: clampLevel(cur?.level), voice_focus: cur?.voice_focus ?? false });
  }

  function pickLevel(level: number) {
    requestNc("ambient", { mode: "Ambient", level, voice_focus: st.nc.value?.voice_focus ?? false });
  }

  function pickVoice(on: boolean) {
    requestNc("ambient", { mode: "Ambient", level: clampLevel(ncShown?.level), voice_focus: on });
  }

  function pickUlt(choice: (typeof ULT)[number]) {
    wantUlt = choice.level;
    ultRefused = null;
    setUlt(choice.arg).catch((e) => {
      wantUlt = null;
      console.error(e);
    });
  }

  function ago(at: number): string {
    const minutes = Math.floor((now - at) / 60000);
    if (minutes < 1) return "just now";
    if (minutes < 60) return minutes === 1 ? "1 minute ago" : `${minutes} minutes ago`;
    const hours = Math.floor(minutes / 60);
    return hours === 1 ? "1 hour ago" : `${hours} hours ago`;
  }

  onMount(() => {
    let heard = false;
    const unlisten = onState((s) => {
      heard = true;
      receive(s);
    });
    getState().then((s) => heard || receive(s));
    const clock = setInterval(() => (now = Date.now()), 30000);
    // Focus rings only for keyboard users: a Tab press turns them on, a click turns them off.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Tab") document.body.classList.add("kbd");
    };
    const onPointer = () => document.body.classList.remove("kbd");
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      clearInterval(clock);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
      unlisten.then((stop) => stop());
    };
  });
</script>

{#if view === "main"}
  <main class="win">
    <p class="lab">Hairline</p>
    <p>Overview coming in Phase 2.</p>
  </main>
{:else if st.connected}
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
      <div class="boxes wide" class:pending={isPending(st.nc.status)} class:unavailable={ncOff}>
        {#each NOISE as choice (choice.mode)}
          <button
            class:on={ncShown?.mode === choice.mode}
            class:refused={ncRefused?.mode === choice.mode}
            title={ncRefused?.mode === choice.mode ? ncRefused.reason : undefined}
            disabled={ncOff}
            onclick={() => pickNoise(choice)}>{choice.label}</button
          >
        {/each}
      </div>
    </div>
    <div class="row">
      <span class="lab">Level {ncShown?.level ?? "--"}</span>
      <div class="ticks" class:pending={isPending(st.nc.status)} class:unavailable={levelOff}>
        {#each TICKS as level (level)}
          <button
            class:on={level <= (ncShown?.level ?? 0)}
            aria-label={`Ambient level ${level}`}
            aria-disabled={levelOff}
            tabindex={levelOff ? -1 : 0}
            onclick={levelOff ? undefined : () => pickLevel(level)}
          ></button>
        {/each}
      </div>
    </div>
    {#if ncShown?.mode === "Ambient"}
      <div class="row">
        <span class="lab">Voice focus</span>
        <div class="boxes wide" class:pending={isPending(st.nc.status)} class:unavailable={ncOff}>
          {#each VOICE as choice (choice.label)}
            <button class:on={ncShown.voice_focus === choice.on} disabled={ncOff} onclick={() => pickVoice(choice.on)}
              >{choice.label}</button
            >
          {/each}
        </div>
      </div>
    {/if}
    <div class="row">
      <span class="lab">ULT</span>
      <div class="boxes ult wide" class:pending={isPending(st.eq_ult.status)} class:unavailable={ultOff}>
        {#each ULT as choice (choice.level)}
          <button
            class:on={ultShown === choice.level}
            class:refused={ultRefused?.level === choice.level}
            title={ultRefused?.level === choice.level ? ultRefused.reason : undefined}
            disabled={ultOff}
            onclick={() => pickUlt(choice)}>{choice.label}</button
          >
        {/each}
      </div>
    </div>
    {#if st.devices.length > 0}
      <div class="link">
        {#each st.devices as device (device.name)}
          <div>
            <span>{device.name}</span>
            {#if device.playing}<span class="live">▶ Playing</span>{:else}<span>Idle</span>{/if}
          </div>
        {/each}
      </div>
    {/if}
    <button class="row footer" onclick={() => openMainWindow()}>
      <span class="lab">Open window</span><span class="lab">⌥⌘N</span>
    </button>
  </div>
{:else}
  <div class="pop">
    <div class="row"><span class="name">{st.model ?? "ULT WEAR"}</span><span class="lab">Not connected</span></div>
    {#if lastSeen}
      <div class="row">
        <span class="lab">Last seen</span>
        <span class="small">{ago(lastSeen.at)}{lastSeen.battery !== null ? ` · ${lastSeen.battery}%` : ""}</span>
      </div>
    {/if}
    <div class="row">
      <span class="lab">Noise</span>
      <div class="boxes wide unavailable">
        {#each NOISE as choice (choice.mode)}<button disabled>{choice.label}</button>{/each}
      </div>
    </div>
    <div class="row">
      <span class="lab">ULT</span>
      <div class="boxes ult wide unavailable">
        {#each ULT as choice (choice.level)}<button disabled>{choice.label}</button>{/each}
      </div>
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

  .wide {
    width: 190px;
  }

  .link {
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

  .win {
    min-height: 100vh;
    margin: 0;
    padding: 48px 32px 24px;
    background: var(--paper);
  }

  .win p:last-child {
    font-size: 13px;
    color: var(--muted);
  }
</style>
