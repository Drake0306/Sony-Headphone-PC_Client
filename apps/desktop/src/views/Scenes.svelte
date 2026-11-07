<!--
  Scenes: the list on the left (first match wins; Up / Down set the priority,
  Apply runs a scene by hand) and the editor for one scene on the right. The
  editor works on a local draft; Save scene and Delete send the whole list.
-->
<script lang="ts">
  import Boxes from "../lib/controls/Boxes.svelte";
  import Row from "../lib/controls/Row.svelte";
  import SceneCard from "../lib/controls/SceneCard.svelte";
  import Ticks from "../lib/controls/Ticks.svelte";
  import Toggle from "../lib/controls/Toggle.svelte";
  import {
    accelerator,
    AMBIENT_MAX,
    applyScene,
    EQ_PRESETS,
    keyLabel,
    moveScene,
    newScene,
    saveScenes,
    scenes,
    triggerSentence,
    validateScene,
    VOLUME_MAX,
    type Actions,
    type Scene,
    type Trigger,
    type TriggerKind,
  } from "../lib/scenes";
  import { presetName, type NcMode } from "../lib/state";

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
  const EQ: { label: string; value: number | Keep }[] = [
    ...EQ_PRESETS.map((id) => ({ label: presetName(id), value: id })),
    { label: "Keep", value: KEEP },
  ];
  const mac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");

  const clone = (s: Scene): Scene => JSON.parse(JSON.stringify(s));

  let draft = $state<Scene | null>(null);
  let picked = false;
  let picking = $state(false);
  let recording = $state(false);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let listError = $state<string | null>(null);
  let applying = $state<string | null>(null);

  const list = $derived($scenes.scenes);
  const isNew = $derived(draft !== null && !list.some((s) => s.id === draft?.id));
  const hotkeyParts = $derived(draft?.hotkey ? draft.hotkey.split("+") : []);

  // Open the first scene once the list arrives.
  $effect(() => {
    if (!picked && list.length > 0) {
      picked = true;
      select(list[0]);
    }
  });

  // Apply is pending until the next "scenes" event.
  $effect(() => {
    void $scenes;
    applying = null;
  });

  function select(s: Scene | null) {
    draft = s ? clone(s) : null;
    picking = false;
    recording = false;
    error = null;
  }

  function create() {
    select(newScene());
    picking = true;
  }

  function act<K extends keyof Actions>(key: K, value: Actions[K]) {
    if (draft) draft.actions[key] = value;
  }

  const orKeep = <T,>(v: T | null): T | Keep => (v === null ? KEEP : v);
  const unKeep = <T,>(v: T | Keep): T | null => (v === KEEP ? null : (v as T));

  function setKind(kind: TriggerKind) {
    if (!draft || draft.trigger.kind === kind) return;
    const t: Trigger =
      kind === "time_range"
        ? { kind, ...RANGE_START }
        : kind === "frontmost_app"
          ? { kind, name: "" }
          : { kind };
    draft.trigger = t;
  }

  function setRange(field: "from" | "to", value: string) {
    if (draft?.trigger.kind === "time_range") draft.trigger[field] = value;
  }

  function setApp(name: string) {
    if (draft?.trigger.kind === "frontmost_app") draft.trigger.name = name;
  }

  function record(e: KeyboardEvent) {
    if (!recording || !draft) return;
    if (e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey) return;
    e.preventDefault();
    if (e.code === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
      recording = false;
      return;
    }
    const hotkey = accelerator(e);
    if (hotkey === null) return;
    draft.hotkey = hotkey;
    recording = false;
  }

  async function send(next: Scene[]): Promise<string | null> {
    saving = true;
    const result = await saveScenes(next);
    saving = false;
    return result;
  }

  async function save() {
    if (!draft || saving) return;
    const scene = $state.snapshot(draft) as Scene;
    error = validateScene(scene);
    if (error !== null) return;
    const next = isNew ? [...list, scene] : list.map((s) => (s.id === scene.id ? scene : s));
    error = await send(next);
  }

  async function remove() {
    if (!draft || saving) return;
    const id = draft.id;
    if (isNew) {
      select(list[0] ?? null);
      return;
    }
    const at = list.findIndex((s) => s.id === id);
    const next = list.filter((s) => s.id !== id);
    error = await send(next);
    if (error === null) select(next[Math.min(at, next.length - 1)] ?? null);
  }

  async function move(id: string, dir: -1 | 1) {
    if (saving) return;
    listError = await send(moveScene(list, id, dir));
  }

  function apply(id: string) {
    listError = null;
    applying = id;
    applyScene(id).catch((e) => {
      applying = null;
      listError = String(e);
    });
  }
</script>

<div class="page">
  <div class="two cols">
    <div class="col">
      <div class="block">
        <h4>Scenes · first match wins</h4>
        {#each list as scene, i (scene.id)}
          <SceneCard
            {scene}
            active={scene.id === $scenes.active}
            selected={scene.id === draft?.id}
            onselect={() => select(scene)}
          >
            <button
              class="btn"
              class:unavailable={i === 0 || saving}
              aria-disabled={i === 0 || saving}
              aria-label={`Move ${scene.name} up`}
              onclick={() => i > 0 && move(scene.id, -1)}>Up</button
            >
            <button
              class="btn"
              class:unavailable={i === list.length - 1 || saving}
              aria-disabled={i === list.length - 1 || saving}
              aria-label={`Move ${scene.name} down`}
              onclick={() => i < list.length - 1 && move(scene.id, 1)}>Down</button
            >
            <button
              class="btn"
              class:unavailable={applying !== null}
              aria-busy={applying === scene.id}
              aria-label={`Apply ${scene.name}`}
              onclick={() => applying === null && apply(scene.id)}>{applying === scene.id ? "Applying" : "Apply"}</button
            >
          </SceneCard>
        {/each}
        {#if listError}<p class="note live">{listError}</p>{/if}
        <p class="note">The first enabled scene whose trigger matches is applied; Up and Down set the order.</p>
        <div class="btnrow"><button class="btn fill" onclick={create}>New scene</button></div>
      </div>
    </div>
    <div class="col">
      {#if draft}
        <div class="block">
          <h4>{isNew ? "New scene" : `Editing · ${draft.name}`}</h4>
          <Row label="Name">
            <input
              class="field name"
              aria-label="Scene name"
              value={draft.name}
              oninput={(e) => draft && (draft.name = e.currentTarget.value)}
            />
          </Row>
          <Row label="Enabled" note="Follows its trigger and shows in the menu bar">
            <Toggle on={draft.enabled} label="Enabled" onpick={(on) => draft && (draft.enabled = on)} />
          </Row>
          <Row label="When" note={triggerSentence(draft.trigger)}>
            <button class="btn" class:fill={picking} onclick={() => (picking = !picking)}>Change</button>
          </Row>
          {#if picking}
            <div class="pick">
              <div class="grid3">
                <Boxes options={TRIGGERS} value={draft.trigger.kind} status="Confirmed" onpick={setKind} />
              </div>
              {#if draft.trigger.kind === "time_range"}
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
              {@render keep(draft.actions.ambient_level === null, () => act("ambient_level", null))}
            </div>
          </Row>
          <Row label="Focus on voice">
            <div class="line">
              <Toggle on={draft.actions.voice_focus} label="Focus on voice" onpick={(on) => act("voice_focus", on)} />
              {@render keep(draft.actions.voice_focus === null, () => act("voice_focus", null))}
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
              {@render keep(draft.actions.volume === null, () => act("volume", null))}
            </div>
          </Row>
          <Row label="DSEE auto-off" note="On lets DSEE Extreme switch itself off">
            <div class="line">
              <Toggle on={draft.actions.dsee_auto_off} label="DSEE auto-off" onpick={(on) => act("dsee_auto_off", on)} />
              {@render keep(draft.actions.dsee_auto_off === null, () => act("dsee_auto_off", null))}
            </div>
          </Row>
          <Row label="Hotkey" note="Apply this scene by hand">
            <div class="line">
              {#if hotkeyParts.length > 0}
                <div class="keys">{#each hotkeyParts as part, i (i)}<span>{keyLabel(part, mac)}</span>{/each}</div>
              {/if}
              <button
                class="btn"
                class:fill={recording}
                onclick={() => (recording = !recording)}
                onkeydown={record}
                onblur={() => (recording = false)}>{recording ? "Press keys" : "Record"}</button
              >
              <button
                class="btn"
                class:unavailable={draft.hotkey === null}
                aria-disabled={draft.hotkey === null}
                onclick={() => draft && (draft.hotkey = null)}>Clear</button
              >
            </div>
          </Row>
          <div class="btnrow">
            <button class="btn" class:unavailable={saving} aria-disabled={saving} onclick={remove}>Delete</button>
            <button class="btn fill" class:unavailable={saving} aria-disabled={saving} onclick={save}>Save scene</button>
          </div>
          {#if error}<p class="note live">{error}</p>{/if}
        </div>
      {/if}
    </div>
  </div>
</div>

{#snippet keep(on: boolean, onpick: () => void)}
  <Boxes options={keepBox} value={on ? KEEP : null} status="Confirmed" width="56px" onpick={onpick} />
{/snippet}

<style>
  .cols {
    grid-template-columns: 1fr 1.3fr;
    height: 100%;
  }

  .col {
    overflow-y: auto;
    padding-right: 4px;
  }

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

  .field {
    border: 1px solid var(--ink);
    border-radius: 0;
    background: var(--box);
    color: var(--ink);
    font-family: "DM Mono", monospace;
    font-size: 11px;
    padding: 5px 6px;
    -webkit-user-select: text;
    user-select: text;
  }

  .field:focus {
    outline: 1px solid var(--ink);
    outline-offset: 1px;
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

  .note.live {
    color: var(--ult);
  }

  .btnrow {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 8px;
  }
</style>
