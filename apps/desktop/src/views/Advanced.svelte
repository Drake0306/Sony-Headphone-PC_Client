<!--
  Advanced (shown when Settings enables it): the support functions the
  headphones declare or a probe proved on the left with the raw command,
  the live frame log on the right. Frames stream only while this screen is
  mounted.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Boxes from "../lib/controls/Boxes.svelte";
  import FrameList from "../lib/controls/FrameList.svelte";
  import {
    afterMark,
    hexGroups,
    isSetOpcode,
    parseHex,
    type CapabilityRow,
    type FrameEntry,
    type TableName,
  } from "../lib/advanced";
  import { capabilityList, onFrames, onRecording, recordFrames, sendRaw, watchFrames } from "../lib/ipc";
  import type { DeviceState } from "../lib/state";

  let { st }: { st: DeviceState } = $props();

  const TABLES: { label: string; value: TableName }[] = [
    { label: "T1", value: "T1" },
    { label: "T2", value: "T2" },
  ];
  const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

  // Capabilities: re-read whenever the device state changes.
  let rows = $state<CapabilityRow[]>([]);
  $effect(() => {
    void st;
    capabilityList()
      .then((r) => (rows = r))
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

  function send() {
    if (sending || hex.trim() === "") return;
    sending = true;
    sendRaw(table === "T1" ? "t1" : "t2", hex, needsConfirm ? confirm : "")
      .then(() => {
        result = { ok: true, text: "Sent" };
        confirm = "";
      })
      .catch((e) => (result = { ok: false, text: errText(e) }))
      .finally(() => (sending = false));
  }

  // Frame log.
  let frames = $state<FrameEntry[]>([]);
  let mark = $state<FrameEntry | null>(null);
  const shown = $derived(afterMark(frames, mark));
  let origin = $state<number | null>(null);
  $effect(() => {
    if (origin === null && shown.length > 0) origin = shown[0].t_ms;
  });

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

  onMount(() => {
    const offFrames = onFrames((f) => (frames = f));
    const offRecording = onRecording((e) => (recording = e.path));
    offFrames
      .then(() => watchFrames(true))
      .catch((e) => console.error(e));
    return () => {
      watchFrames(false).catch((e) => console.error(e));
      for (const off of [offFrames, offRecording]) off.then((stop) => stop()).catch(() => {});
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
            class="mono"
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
            class="mono confirm"
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

  .caps div.faded {
    opacity: 0.35;
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

  input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--ink);
    border-radius: 0;
    background: var(--box);
    color: var(--ink);
    font-family: "DM Mono", monospace;
    font-size: 11px;
    outline: none;
    -webkit-user-select: text;
    user-select: text;
  }

  input::placeholder {
    color: var(--muted);
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
