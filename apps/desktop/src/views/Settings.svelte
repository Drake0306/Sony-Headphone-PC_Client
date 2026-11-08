<!--
  Settings: the headphone settings the ULT WEAR declares on the left, the
  app's own settings and facts on the right.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import Boxes from "../lib/controls/Boxes.svelte";
  import LinkList from "../lib/controls/LinkList.svelte";
  import Row from "../lib/controls/Row.svelte";
  import Toggle from "../lib/controls/Toggle.svelte";
  import { battery, historyNote } from "../lib/battery";
  import { control } from "../lib/control.svelte";
  import {
    clearBatteryHistory,
    setAutoPowerOff,
    setQuickAccess,
    setSettings,
    setSidetone,
    setTouchPanel,
    setVoiceAssistant,
    setVoiceGuideVolume,
    setWearPause,
  } from "../lib/ipc";
  import { hotkeyList, scenes } from "../lib/scenes";
  import { settings } from "../lib/settings";
  import { checkNow, installNow, later, percent, update, visible } from "../lib/update";
  import { AUTO_POWER_OFF, type AppSettings, type DeviceState, type Status, type Theme } from "../lib/state";

  let { st }: { st: DeviceState } = $props();

  const VOICE_GUIDE = [
    { label: "−2", value: -2, title: "Quiet" },
    { label: "−1", value: -1, title: "Softer" },
    { label: "0", value: 0, title: "Default" },
    { label: "+1", value: 1, title: "Louder" },
    { label: "+2", value: 2, title: "Loud" },
  ];
  const ASSISTANTS = [
    { label: "Phone", value: 0x30 },
    { label: "Google", value: 0x31 },
    { label: "Alexa", value: 0x32 },
    { label: "Off", value: 0xff },
  ];
  const THEMES: { label: string; value: Theme }[] = [
    { label: "Paper", value: "paper" },
    { label: "Ink", value: "ink" },
    { label: "System", value: "system" },
  ];
  const PENDING: Status = { Pending: { since_ms: 0 } };
  const hex = (id: number) => `0x${id.toString(16).toUpperCase().padStart(2, "0")}`;
  const mac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");
  const hotkeys = $derived(hotkeyList($scenes.scenes, mac, $scenes.hotkey_errors));

  const autoOff = control<number>(() => st.auto_power_off);
  const wear = control<boolean>(() => st.wear_pause);
  const touch = control<boolean>(() => st.touch_panel);
  const sidetone = control<boolean>(() => st.sidetone);
  const guide = control<number>(() => st.voice_guide_volume);
  const assistant = control<number>(() => st.voice_assistant);
  // Quick Access reads as "off" when both slots are empty, else "set".
  const quick = control<"off" | "set">(() => ({
    value: st.quick_access.value ? (st.quick_access.value.every((f) => f === 0) ? "off" : "set") : null,
    status: st.quick_access.status,
  }));

  const quickOptions = $derived([
    { label: "Off", value: "off" as const },
    ...(st.quick_access.value ?? []).flatMap((id, i) =>
      id === 0 ? [] : [{ label: `Slot ${i + 1}: ${hex(id)}`, value: "set" as const, inert: true, title: "not decodable yet" }],
    ),
  ]);
  const assistantOptions = $derived.by(() => {
    const cur = st.voice_assistant.value;
    if (cur === null || ASSISTANTS.some((a) => a.value === cur)) return ASSISTANTS;
    return [...ASSISTANTS, { label: cur === 0x34 ? "Sony" : hex(cur), value: cur }];
  });

  // App settings: pending while `set_settings` runs, confirmed by the "settings" event.
  let saving = $state<Record<string, boolean>>({});
  const appSource =
    <T,>(key: keyof AppSettings) =>
    () => ({
      value: ($settings?.[key] ?? null) as T | null,
      status: $settings === null ? ("Unavailable" as Status) : saving[key] ? PENDING : ("Confirmed" as Status),
    });
  const login = control<boolean>(appSource("launch_at_login"));
  const dock = control<boolean>(appSource("hide_dock_icon"));
  const theme = control<Theme>(appSource("theme"));
  const advanced = control<boolean>(appSource("show_advanced"));
  const autoCheck = control<boolean>(appSource("auto_check_updates"));

  // Updates: the release card shows while one is offered (not put off with
  // Later) or an install is under way.
  let checking = $state(false);
  const phase = $derived($update.phase);
  const installing = $derived(
    phase?.phase === "downloading" || phase?.phase === "installing" || phase?.phase === "restarting",
  );
  const pct = $derived(percent(phase));
  const checkNote = $derived(
    phase?.phase === "up_to_date" ? "Up to date" : phase?.phase === "failed" ? phase.error : undefined,
  );
  async function check() {
    checking = true;
    await checkNow();
    checking = false;
  }

  function save<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    const cur = $settings;
    if (!cur) return Promise.reject(new Error("settings not loaded"));
    saving[key] = true;
    return setSettings({ ...cur, [key]: value }).finally(() => (saving[key] = false));
  }

  let version = $state("");
  onMount(() => {
    getVersion()
      .then((v) => (version = v))
      .catch((e) => console.error(e));
  });
</script>

<div class="page">
  <div class="two">
    <div>
      <div class="block">
        <h4>Headphones</h4>
        <Row label="Auto power off" note="When idle or removed">
          <div class="fit">
            <Boxes
              options={AUTO_POWER_OFF}
              value={autoOff.shown}
              attempted={autoOff.refused?.value ?? null}
              status={autoOff.status}
              onpick={(v) => autoOff.request(v, () => setAutoPowerOff(v))}
            />
          </div>
        </Row>
        <Row label="Pause when taken off">
          <Toggle
            on={wear.shown}
            status={wear.status}
            label="Pause when taken off"
            onpick={(on) => wear.request(on, () => setWearPause(on))}
          />
        </Row>
        <Row label="Touch panel">
          <Toggle
            on={touch.shown}
            status={touch.status}
            label="Touch panel"
            onpick={(on) => touch.request(on, () => setTouchPanel(on))}
          />
        </Row>
        <Row label="Sidetone" note="Hear your own voice on calls">
          <Toggle
            on={sidetone.shown}
            status={sidetone.status}
            label="Sidetone"
            onpick={(on) => sidetone.request(on, () => setSidetone(on))}
          />
        </Row>
        <Row label="Voice guide" note="Volume of the spoken prompts">
          <Boxes
            options={VOICE_GUIDE}
            value={guide.shown}
            attempted={guide.refused?.value ?? null}
            status={guide.status}
            width="200px"
            onpick={(v) => guide.request(v, () => setVoiceGuideVolume(v))}
          />
        </Row>
        <Row label="Quick Access" note="Double-tap action">
          <div class="fit">
            <Boxes
              options={quickOptions}
              value={quick.shown}
              attempted={quick.refused?.value ?? null}
              status={quick.status}
              onpick={(v) => v === "off" && quick.request("off", () => setQuickAccess([0, 0]))}
            />
          </div>
        </Row>
        <Row label="Voice assistant">
          <Boxes
            options={assistantOptions}
            value={assistant.shown}
            attempted={assistant.refused?.value ?? null}
            status={assistant.status}
            width={assistantOptions.length > 4 ? "250px" : "200px"}
            onpick={(v) => assistant.request(v, () => setVoiceAssistant(v))}
          />
        </Row>
      </div>
    </div>
    <div>
      <div class="block">
        <h4>This app</h4>
        <Row label="Open at login">
          <Toggle
            on={login.shown}
            status={login.status}
            label="Open at login"
            onpick={(on) => login.request(on, () => save("launch_at_login", on))}
          />
        </Row>
        <Row label="Show in menu bar" note="The popover lives here">
          <Toggle readonly on={true} label="Show in menu bar" title="Hairline always lives in the menu bar" />
        </Row>
        <Row label="Hide Dock icon">
          <Toggle
            on={dock.shown}
            status={dock.status}
            label="Hide Dock icon"
            onpick={(on) => dock.request(on, () => save("hide_dock_icon", on))}
          />
        </Row>
        <Row label="Hotkeys" note="Set per scene in Scenes">
          <div class="hotkeys">
            {#each hotkeys as h, i (i)}
              <div>
                <span class="keys" class:refused={h.error !== null} title={h.error ?? undefined}
                  >{#each h.keys as k, j (j)}<span>{k}</span>{/each}</span
                >{h.label}{#if h.error}<small class="refusal">{h.error}</small>{/if}
              </div>
            {/each}
          </div>
        </Row>
        <Row label="Battery history" note={historyNote($battery)}>
          {#if $battery.samples > 0}
            <button class="btn" onclick={() => clearBatteryHistory().catch((e) => console.error(e))}>Clear</button>
          {/if}
        </Row>
        <Row label="Appearance">
          <Boxes
            options={THEMES}
            value={theme.shown}
            status={theme.status}
            width="200px"
            onpick={(t) => theme.request(t, () => save("theme", t))}
          />
        </Row>
        <Row label="Advanced panel" note="Frame log, capability list, raw commands">
          <Toggle
            on={advanced.shown}
            status={advanced.status}
            label="Advanced panel"
            onpick={(on) => advanced.request(on, () => save("show_advanced", on))}
          />
        </Row>
      </div>
      <div class="block">
        <h4>Updates</h4>
        <Row label={`Hairline ${$update.current}`} note={checkNote}>
          <button class="btn" disabled={checking || installing} onclick={check}>{checking ? "Checking…" : "Check now"}</button>
        </Row>
        {#if $update.available && ($visible || installing)}
          <div class="release">
            <div>
              Hairline {$update.available.version} is available{#if $update.available.date}<small
                  >{$update.available.date}</small
                >{/if}
            </div>
            {#if $update.available.notes}<div class="notes">{$update.available.notes}</div>{/if}
            {#if phase?.phase === "downloading"}
              <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct ?? undefined}>
                <span style:width={`${pct ?? 0}%`}></span>
              </div>
              <p class="note">Downloading…{pct !== null ? ` ${pct} %` : ""}</p>
            {:else if phase?.phase === "installing"}
              <p class="note">Installing…</p>
            {:else if phase?.phase === "restarting"}
              <p class="note">Restarting…</p>
            {:else}
              <div class="btnrow">
                <button class="btn" onclick={() => later()}>Later</button>
                <button class="btn fill" onclick={() => installNow()}>Update now</button>
              </div>
            {/if}
          </div>
        {/if}
        <Row label="Check for updates automatically" note="On launch and every 6 hours">
          <Toggle
            on={autoCheck.shown}
            status={autoCheck.status}
            label="Check for updates automatically"
            onpick={(on) => autoCheck.request(on, () => save("auto_check_updates", on))}
          />
        </Row>
      </div>
      <div class="block">
        <LinkList
          items={[
            { label: "App version", value: version },
            { label: "App protocol", value: "MDR v2 · tables 1+2" },
            { label: "Not affiliated with Sony", value: "" },
          ]}
        />
      </div>
    </div>
  </div>
</div>

<style>
  /* Boxes sized to their labels (seven auto-off values, Quick Access slots). */
  .fit :global(.boxes) {
    grid-auto-columns: auto;
  }

  .fit :global(.boxes button) {
    padding: 6px 7px;
  }

  .hotkeys {
    display: grid;
    gap: 6px;
  }

  .hotkeys div {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .release {
    padding: 9px 0;
    border-bottom: 1px solid var(--hair);
    font-size: 13px;
  }

  .release small {
    margin-left: 8px;
    color: var(--muted);
    font-size: 11px;
  }

  /* The release notes: about six lines, then scroll. */
  .notes {
    margin-top: 6px;
    max-height: calc(6 * 1.45em);
    overflow-y: auto;
    white-space: pre-line;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .bar {
    margin-top: 10px;
    height: 2px;
    background: var(--hair);
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--ink);
  }

  /* Why a scene's hotkey was refused: orange, like the refused chips. */
  .refusal {
    color: var(--ult);
    font-size: 11px;
  }
</style>
