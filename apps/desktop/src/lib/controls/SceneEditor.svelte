<!--
  The editor for one scene, on its own working copy of `draft` (the parent
  re-creates it with {#key} to open another scene). Save scene hands the
  copy to `onSave`; Delete calls `onDelete`; `error` shows under the buttons.
-->
<script lang="ts">
  import Boxes from "./Boxes.svelte";
  import Row from "./Row.svelte";
  import Ticks from "./Ticks.svelte";
  import Toggle from "./Toggle.svelte";
  import {
    accelerator,
    AMBIENT_MAX,
    eqPresetChoices,
    hotkeyChips,
    isModifierPress,
    triggerSentence,
    VOLUME_MAX,
    type Actions,
    type Scene,
    type Trigger,
    type TriggerKind,
  } from "../scenes";
  import { presetName, type NcMode } from "../state";

  interface Props {
    draft: Scene;
    /** Not in the saved list yet. */
    isNew?: boolean;
    /** Preset ids the headphones report; null or empty uses the known list. */
    devicePresets?: number[] | null;
    /** A save or delete is in flight. */
    busy?: boolean;
    error?: string | null;
    /** Why the saved hotkey was not registered. */
    hotkeyError?: string | null;
    onSave: (scene: Scene) => void;
    onDelete: () => void;
  }

  let {
    draft: initial,
    isNew = false,
    devicePresets = null,
    busy = false,
    error = null,
    hotkeyError = null,
    onSave,
    onDelete,
  }: Props = $props();

  type Keep = "keep";
  const KEEP: Keep = "keep";
  const keepBox = [{ label: "Keep", value: KEEP }];
  const TRIGGERS: { label: string; value: TriggerKind }[] = [
    { label: "Manual", value: "manual" },
    { label: "Headphones connect", value: "headphones_connected" },
    { label: "Time range", value: "time_range" },
    { label: "App in front", value: "frontmost_app" },
    { label: "Microphone in use", value: "mic_in_use" },
  ];
  // Starting values when a scene switches to a time range (the seeded "Late" scene's hours).
  const RANGE_START = { from: "22:00", to: "07:00" };
  const NOISE: { label: string; value: NcMode | Keep }[] = [
    { label: "NC", value: "NoiseCancelling" },
    { label: "AMB", value: "Ambient" },
    { label: "OFF", value: "Off" },
    { label: "Keep", value: KEEP },
  ];
  const ULT: { label: string; value: number | Keep; orange?: boolean }[] = [
    { label: "OFF", value: 0, orange: true },
    { label: "1", value: 1, orange: true },
    { label: "2", value: 2, orange: true },
    { label: "Keep", value: KEEP },
  ];
  const EQ: { label: string; value: number | Keep }[] = $derived([
    ...eqPresetChoices(devicePresets, initial.actions.eq_preset).map((id) => ({ label: presetName(id), value: id })),
    { label: "Keep", value: KEEP },
  ]);
  // Ticks start at 1, so volume 0 has its own box.
  const VOLUME_BOXES: { label: string; value: 0 | Keep }[] = [
    { label: "0", value: 0 },
    { label: "Keep", value: KEEP },
  ];
  const mac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

  // svelte-ignore state_referenced_locally
  let draft = $state<Scene>(JSON.parse(JSON.stringify(initial)));
  // svelte-ignore state_referenced_locally
  let picking = $state(isNew);
  let recording = $state(false);
  // The last press while recording had no Ctrl, Alt or Super (and was not F1–F24).
  let needsModifier = $state(false);

  const hotkeyParts = $derived(hotkeyChips(draft.hotkey, mac));
  // The error is about the saved hotkey; a newly recorded one has not been tried yet.
  const refused = $derived(hotkeyError !== null && draft.hotkey === initial.hotkey ? hotkeyError : null);

  const orKeep = <T,>(v: T | null): T | Keep => (v === null ? KEEP : v);
  const unKeep = <T,>(v: T | Keep): T | null => (v === KEEP ? null : (v as T));

  function act<K extends keyof Actions>(key: K, value: Actions[K]) {
    draft.actions[key] = value;
    // Voice focus belongs to Ambient; any other noise choice drops it to Keep.
    if (key === "nc" && value !== "Ambient") draft.actions.voice_focus = null;
  }

  function setKind(kind: TriggerKind) {
    if (draft.trigger.kind === kind) return;
    const t: Trigger =
      kind === "time_range" ? { kind, ...RANGE_START } : kind === "frontmost_app" ? { kind, name: "" } : { kind };
    draft.trigger = t;
  }

  function setRange(field: "from" | "to", value: string) {
    if (draft.trigger.kind === "time_range") draft.trigger[field] = value;
  }

  function setApp(name: string) {
    if (draft.trigger.kind === "frontmost_app") draft.trigger.name = name;
  }

  // A clicked button is not focused in macOS WebKit, so the recorder listens on the window.
  $effect(() => {
    needsModifier = false;
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        recording = false;
        return;
      }
      const hotkey = accelerator(e);
      if (hotkey === null) {
        if (!isModifierPress(e)) needsModifier = true;
        return;
      }
      draft.hotkey = hotkey;
      recording = false;
    };
    const stop = () => (recording = false);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", stop);
    };
  });

  function save() {
    if (!busy) onSave($state.snapshot(draft) as Scene);
  }
</script>

<div class="block">
  <h4>{isNew ? "New scene" : `Editing · ${draft.name}`}</h4>
  <Row label="Name">
    <input
      class="field name"
      aria-label="Scene name"
      value={draft.name}
      oninput={(e) => (draft.name = e.currentTarget.value)}
    />
  </Row>
  <Row label="Enabled" note="Follows its trigger and shows in the menu bar">
    <Toggle on={draft.enabled} label="Enabled" onpick={(on) => (draft.enabled = on)} />
  </Row>
  <Row label="When" note={triggerSentence(draft.trigger)}>
    <button class="btn" class:fill={picking} onclick={() => (picking = !picking)}>Change</button>
  </Row>
  {#if picking}
    <div class="pick">
      <div class="grid3">
        <Boxes options={TRIGGERS} value={draft.trigger.kind} status="Confirmed" onpick={setKind} />
      </div>
      {#if draft.trigger.kind === "headphones_connected"}
        <p class="note">Holds for as long as they stay connected, so put it below more specific scenes.</p>
      {:else if draft.trigger.kind === "time_range"}
        <div class="fields">
          <span class="lab">From</span>
          <input
            class="field hhmm"
            aria-label="From"
            placeholder="HH:MM"
            inputmode="numeric"
            value={draft.trigger.from}
            oninput={(e) => setRange("from", e.currentTarget.value)}
          />
          <span class="lab">To</span>
          <input
            class="field hhmm"
            aria-label="To"
            placeholder="HH:MM"
            inputmode="numeric"
            value={draft.trigger.to}
            oninput={(e) => setRange("to", e.currentTarget.value)}
          />
        </div>
      {:else if draft.trigger.kind === "frontmost_app"}
        <div class="fields">
          <span class="lab">App</span>
          <input
            class="field app"
            aria-label="App name"
            placeholder="App name as the Dock shows it"
            value={draft.trigger.name}
            oninput={(e) => setApp(e.currentTarget.value)}
          />
        </div>
      {/if}
    </div>
  {/if}
  <Row label="Noise">
    <Boxes
      options={NOISE}
      value={orKeep(draft.actions.nc)}
      status="Confirmed"
      width="210px"
      onpick={(v) => act("nc", unKeep(v))}
    />
  </Row>
  <Row label="Ambient level">
    <div class="line">
      <Ticks
        count={AMBIENT_MAX}
        value={draft.actions.ambient_level}
        status="Confirmed"
        label="Ambient level"
        onpick={(v) => act("ambient_level", v)}
      />
      <span class="lab num">{draft.actions.ambient_level ?? ""}</span>
      <Boxes
        options={keepBox}
        value={draft.actions.ambient_level === null ? KEEP : null}
        status="Confirmed"
        width="56px"
        onpick={() => act("ambient_level", null)}
      />
    </div>
  </Row>
  <Row label="Focus on voice" note={draft.actions.nc === "Ambient" ? undefined : "Needs Noise set to Ambient"}>
    <div class="line">
      <Toggle
        on={draft.actions.voice_focus}
        disabled={draft.actions.nc !== "Ambient"}
        label="Focus on voice"
        onpick={(on) => act("voice_focus", on)}
      />
      <Boxes
        options={keepBox}
        value={draft.actions.voice_focus === null ? KEEP : null}
        status="Confirmed"
        width="56px"
        onpick={() => act("voice_focus", null)}
      />
    </div>
  </Row>
  <Row label="ULT">
    <Boxes
      options={ULT}
      value={orKeep(draft.actions.ult)}
      status="Confirmed"
      width="210px"
      onpick={(v) => act("ult", unKeep(v))}
    />
  </Row>
  <div class="stack">
    <span>Equalizer</span>
    <div class="grid4">
      <Boxes
        options={EQ}
        value={orKeep(draft.actions.eq_preset)}
        status="Confirmed"
        onpick={(v) => act("eq_preset", unKeep(v))}
      />
    </div>
  </div>
  <Row label="Volume">
    <div class="line">
      <Ticks
        count={VOLUME_MAX}
        value={draft.actions.volume}
        status="Confirmed"
        label="Volume"
        onpick={(v) => act("volume", v)}
      />
      <span class="lab num">{draft.actions.volume ?? ""}</span>
      <Boxes
        options={VOLUME_BOXES}
        value={draft.actions.volume === null ? KEEP : draft.actions.volume === 0 ? 0 : null}
        status="Confirmed"
        width="84px"
        onpick={(v) => act("volume", unKeep(v))}
      />
    </div>
  </Row>
  <Row label="DSEE auto-off" note="On lets DSEE Extreme switch itself off">
    <div class="line">
      <Toggle on={draft.actions.dsee_auto_off} label="DSEE auto-off" onpick={(on) => act("dsee_auto_off", on)} />
      <Boxes
        options={keepBox}
        value={draft.actions.dsee_auto_off === null ? KEEP : null}
        status="Confirmed"
        width="56px"
        onpick={() => act("dsee_auto_off", null)}
      />
    </div>
  </Row>
  <Row label="Hotkey" note="Apply this scene by hand">
    <div class="line">
      {#if hotkeyParts.length > 0}
        <div class="keys" class:refused={refused !== null} aria-label="Hotkey" title={refused ?? undefined}>
          {#each hotkeyParts as part, i (i)}<span>{part}</span>{/each}
        </div>
      {/if}
      <button class="btn" class:fill={recording} onclick={() => (recording = !recording)}
        >{recording ? "Press keys" : "Record"}</button
      >
      <button
        class="btn"
        class:unavailable={draft.hotkey === null}
        aria-disabled={draft.hotkey === null}
        onclick={() => (draft.hotkey = null)}>Clear</button
      >
    </div>
  </Row>
  {#if recording && needsModifier}<p class="note">{mac ? "Add ⌃, ⌥ or ⌘" : "Add Ctrl, Alt or Super"}</p>{/if}
  {#if refused}<p class="note live">{refused}</p>{/if}
  <div class="btnrow">
    <button class="btn" class:unavailable={busy} aria-disabled={busy} onclick={() => busy || onDelete()}>Delete</button>
    <button class="btn fill" class:unavailable={busy} aria-disabled={busy} onclick={save}>Save scene</button>
  </div>
  {#if error}<p class="note live">{error}</p>{/if}
</div>

<style>
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .num {
    min-width: 16px;
    text-align: right;
  }

  .pick {
    padding: 4px 0 10px;
    border-bottom: 1px solid var(--hair);
  }

  .grid3 :global(.boxes),
  .grid4 :global(.boxes) {
    grid-auto-flow: row;
  }

  .grid3 :global(.boxes) {
    grid-template-columns: repeat(3, 1fr);
  }

  .grid4 :global(.boxes) {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .fields {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }

  .stack {
    display: grid;
    gap: 8px;
    padding: 9px 0;
    border-bottom: 1px solid var(--hair);
    font-size: 13px;
  }

  .name {
    width: 200px;
  }

  .hhmm {
    width: 64px;
  }

  .app {
    flex: 1;
  }
</style>
