<!--
  Advanced (shown when Settings enables it): the support functions the
  headphones declare or a probe proved on the left with the raw command,
  the live frame log and the opcode probe on the right. Frames stream only
  while this screen is mounted.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Boxes from "../lib/controls/Boxes.svelte";
  import FrameList from "../lib/controls/FrameList.svelte";
  import {
    afterMark,
    canRunProbe,
    hexGroups,
    isSetOpcode,
    parseHex,
    probeEstimate,
    probeRow,
    rawNote,
    type CapabilityRow,
    type FrameEntry,
    type ProbeEvent,
    type TableName,
  } from "../lib/advanced";
  import {
    capabilityList,
    onFrames,
    onProbe,
    onRaw,
    onRecording,
    probeStart,
    probeStop,
    recordFrames,
    sendRaw,
    watchFrames,
  } from "../lib/ipc";
  import type { DeviceState } from "../lib/state";

  let { st }: { st: DeviceState } = $props();

  const TABLES: { label: string; value: TableName }[] = [
    { label: "T1", value: "T1" },
    { label: "T2", value: "T2" },
  ];
  const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

  // Capabilities: re-read when the link, the model or the declared/proven
  // lists change; an older reply arriving late is dropped.
  let rows = $state<CapabilityRow[]>([]);
  let request = 0;
  // A string, so a new state object with the same facts does not refetch.
  const capsKey = $derived(
    [st.connected, st.model, st.capabilities_t1.length, st.capabilities_t2.length, st.proven.length].join("|"),
  );
  $effect(() => {
    void capsKey;
    const mine = ++request;
    capabilityList()
      .then((r) => mine === request && (rows = r))
      .catch((e) => console.error(e));
  });
  const byTable = $derived(TABLES.map((t) => ({ table: t.value, rows: rows.filter((r) => r.table === t.value) })));

  // Raw command.
  let table = $state<TableName>("T1");
  let hex = $state("");
  let confirm = $state("");
  let sending = $state(false);
  let result = $state<{ ok: boolean; text: string } | null>(null);
  const bytes = $derived(parseHex(hex));
  const needsConfirm = $derived(bytes !== null && isSetOpcode(table, bytes));

  // The command only hands the bytes to the broker; the "raw" event that
  // follows says whether the session queued them.
  function send() {
    if (sending || hex.trim() === "") return;
    sending = true;
    result = null;
    sendRaw(table === "T1" ? "t1" : "t2", hex, needsConfirm ? confirm : "")
      .then(() => (confirm = ""))
      .catch((e) => (result = { ok: false, text: errText(e) }))
      .finally(() => (sending = false));
  }

  // Frame log.
  let frames = $state<FrameEntry[]>([]);
  let mark = $state<FrameEntry | null>(null);
  const shown = $derived(afterMark(frames, mark));
  // The time shown as +0: the first frame seen, or the first after Clear.
  let origin = $state<number | null>(null);

  function takeFrames(f: FrameEntry[]) {
    frames = f;
    if (origin === null) origin = afterMark(f, mark)[0]?.t_ms ?? null;
  }

  function clear() {
    mark = frames.at(-1) ?? null;
    origin = null;
  }

  let recording = $state<string | null>(null);
  let recordError = $state<string | null>(null);

  function toggleRecord() {
    recordError = null;
    recordFrames(recording === null)
      .then((path) => (recording = path))
      .catch((e) => (recordError = errText(e)));
  }

  // Probe: the backend's last "probe" event (re-sent when watching starts).
  let probe = $state<ProbeEvent | null>(null);
  let probeConfirm = $state("");
  let probeError = $state<string | null>(null);
  const probeState = $derived(probe?.state ?? "Idle");
  const canRun = $derived(canRunProbe(st.connected, probeConfirm, probeState));
  const probeRows = $derived((probe?.results ?? []).map(probeRow));
  const estimate = $derived(probe ? probeEstimate(probe.total, probe.reply_ms, probe.gap_ms) : null);
  let copied = $state(false);

  function copyPath(path: string) {
    navigator.clipboard
      .writeText(path)
      .then(() => (copied = true))
      .catch((e) => (probeError = errText(e)));
  }

  function runProbe() {
    if (!canRun) return;
    probeError = null;
    probeStart(probeConfirm)
      .then(() => (probeConfirm = ""))
      .catch((e) => (probeError = errText(e)));
  }

  function stopProbe() {
    probeStop().catch((e) => (probeError = errText(e)));
  }

  onMount(() => {
    let alive = true;
    const offFrames = onFrames(takeFrames);
    const offRecording = onRecording((e) => (recording = e.path));
    const offRaw = onRaw((e) => (result = { ok: e.ok, text: rawNote(e) }));
    const offProbe = onProbe((e) => {
      if (e.file !== probe?.file) copied = false;
      probe = e;
    });
    // Watch only once listening, and never after the screen has gone.
    offFrames
      .then(() => (alive ? watchFrames(true) : null))
      .catch((e) => console.error(e));
    return () => {
      alive = false;
      watchFrames(false).catch((e) => console.error(e));
      for (const off of [offFrames, offRecording, offRaw, offProbe]) off.then((stop) => stop()).catch(() => {});
    };
  });
</script>

<div class="page">
  <div class="two">
    <div>
      <div class="block">
        <h4>Capabilities</h4>
        {#each byTable as t (t.table)}
          <div class="lab tname">{t.table}</div>
          <div class="link caps">
            {#each t.rows as r (r.id)}
              <div class:faded={!r.declared && !r.proven}>
                <span>{hexGroups([r.id])}</span>
                <span class="name">{r.name ?? "--"}</span>
                <span class="chips"
                  >{#if r.declared}<span>DECLARED</span>{/if}{#if r.proven}<span>PROVEN</span>{/if}</span
                >
              </div>
            {/each}
          </div>
        {/each}
      </div>
      <div class="block">
        <h4>Raw command</h4>
        <div class="raw">
          <Boxes options={TABLES} value={table} status="Confirmed" width="90px" onpick={(t) => (table = t)} />
          <input
            class="field"
            aria-label="Raw bytes"
            placeholder="66 17"
            spellcheck="false"
            autocomplete="off"
            bind:value={hex}
            onkeydown={(e) => e.key === "Enter" && send()}
          />
        </div>
        {#if needsConfirm}
          <input
            class="field confirm"
            aria-label="Confirm"
            placeholder="type SEND"
            spellcheck="false"
            autocomplete="off"
            bind:value={confirm}
            onkeydown={(e) => e.key === "Enter" && send()}
          />
        {/if}
        <div class="btnrow">
          <button class="btn fill" class:unavailable={sending || hex.trim() === ""} onclick={send}>Send</button>
        </div>
        {#if result}
          <p class="note" class:live={!result.ok}>{result.text}</p>
        {/if}
      </div>
    </div>
    <div>
      <div class="block">
        <h4>Frame log</h4>
        <FrameList entries={shown} origin={origin ?? 0} />
        <div class="btnrow">
          <button class="btn" onclick={clear}>Clear</button>
          <button class="btn" class:fill={recording !== null} onclick={toggleRecord}
            >{recording === null ? "Record" : "Stop"}</button
          >
        </div>
        {#if recording !== null}
          <p class="note path">{recording}</p>
        {/if}
        {#if recordError}
          <p class="note live">{recordError}</p>
        {/if}
      </div>
      <div class="block probe">
        <h4>Probe</h4>
        <p class="note">The probe sends read-only GET requests one at a time. Type RUN.</p>
        {#if probeState === "Running"}
          <div class="btnrow">
            <button class="btn fill" onclick={stopProbe}>Stop</button>
          </div>
        {:else}
          {#if estimate}
            <p class="note">{estimate}</p>
          {/if}
          <input
            class="field confirm"
            aria-label="Confirm probe"
            placeholder="type RUN"
            spellcheck="false"
            autocomplete="off"
            bind:value={probeConfirm}
            onkeydown={(e) => e.key === "Enter" && runProbe()}
          />
          <div class="btnrow">
            <button class="btn fill" class:unavailable={!canRun} onclick={runProbe}>Run</button>
          </div>
        {/if}
        {#if probe && probe.state !== "Idle"}
          <p class="note">{probe.done} / {probe.total}</p>
        {/if}
        {#if probeError}
          <p class="note live">{probeError}</p>
        {/if}
        {#if probeRows.length > 0}
          <div class="link results">
            {#each probeRows as row (row.key)}
              <div class:faded={row.chip === "SILENT"}>
                <span>{row.key}</span>
                <span class="name">{row.name}</span>
                <span class="chips">{#if row.chip}<span>{row.chip}</span>{/if}</span>
                <span class="reply">{row.reply}</span>
              </div>
            {/each}
          </div>
        {/if}
        {#if probe?.state === "Done" && probe.file}
          {@const file = probe.file}
          <p class="note path">{file}</p>
          <div class="btnrow">
            <button class="btn" onclick={() => copyPath(file)}>{copied ? "Copied" : "Copy path"}</button>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .tname {
    margin-top: 8px;
  }

  .caps div {
    display: grid;
    grid-template-columns: 28px 1fr auto;
    gap: 10px;
    align-items: center;
  }

  .caps div.faded,
  .results div.faded {
    opacity: 0.35;
  }

  .results {
    max-height: 320px;
    overflow-y: auto;
  }

  .results div {
    display: grid;
    grid-template-columns: 64px 1fr auto;
    gap: 2px 10px;
    align-items: center;
    font-size: 11px;
  }

  .results span:first-child,
  .reply {
    font-family: "DM Mono", monospace;
  }

  .reply {
    grid-column: 2 / -1;
    color: var(--muted);
    overflow-wrap: anywhere;
    -webkit-user-select: text;
    user-select: text;
  }

  .reply:empty {
    display: none;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chips {
    display: flex;
    gap: 4px;
  }

  .chips span {
    padding: 1px 5px;
    border: 1px solid var(--hair);
    font-size: 9px;
  }

  .raw {
    display: flex;
    gap: 8px;
    align-items: stretch;
  }

  .raw .field {
    flex: 1;
    min-width: 0;
  }

  .confirm {
    display: block;
    width: 100%;
    margin-top: 6px;
  }

  .path {
    font-family: "DM Mono", monospace;
    overflow-wrap: anywhere;
    -webkit-user-select: text;
    user-select: text;
  }
</style>
