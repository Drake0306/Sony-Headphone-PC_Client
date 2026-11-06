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
  import { control } from "../lib/control.svelte";
  import {
    setAutoPowerOff,
    setQuickAccess,
    setSettings,
    setSidetone,
    setTouchPanel,
    setVoiceAssistant,
    setVoiceGuideVolume,
    setWearPause,
  } from "../lib/ipc";
  import { settings } from "../lib/settings";
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
  const noop = () => {};

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
        <Row label="Reset headphones" note="Phase 4">
          <button class="btn unavailable" aria-disabled="true" tabindex="-1">Reset…</button>
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
          <Toggle on={true} status="Unavailable" label="Show in menu bar" onpick={noop} />
        </Row>
        <Row label="Hide Dock icon">
          <Toggle
            on={dock.shown}
            status={dock.status}
            label="Hide Dock icon"
            onpick={(on) => dock.request(on, () => save("hide_dock_icon", on))}
          />
        </Row>
        <Row label="Hotkeys" note="Phase 4" unavailable />
        <Row label="Battery history" note="Phase 4">
          <Toggle on={null} status="Unavailable" label="Battery history" onpick={noop} />
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
        <Row label="Advanced" note="Phase 4">
          <button class="btn unavailable" aria-disabled="true" tabindex="-1">Open…</button>
        </Row>
      </div>
      <div class="block">
        <LinkList
          items={[
            { label: "App version", value: version },
            { label: "Protocol", value: "MDR v2 · tables 1+2" },
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
</style>
