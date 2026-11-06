<!--
  Devices: this Mac and Multipoint on the left; the paired list and pairing
  mode (which the ULT WEAR does not expose) and the headphone's facts on the right.
-->
<script lang="ts">
  import LinkList from "../lib/controls/LinkList.svelte";
  import Row from "../lib/controls/Row.svelte";
  import Toggle from "../lib/controls/Toggle.svelte";
  import { control } from "../lib/control.svelte";
  import { setMultipoint } from "../lib/ipc";
  import { codecLabel, colourway, isPlaying, modelNumber, type DeviceState } from "../lib/state";

  let { st }: { st: DeviceState } = $props();

  const noop = () => {};
  const multipoint = control<boolean>(() => st.multipoint);
  const playing = $derived(isPlaying(st.playback));
  const meta = $derived([codecLabel(st.codec.value), "this Mac"].filter(Boolean).join(" · "));
  const facts = $derived([
    { label: "Model", value: modelNumber(st.model) },
    { label: "Firmware", value: st.firmware ?? "" },
    { label: "Bluetooth", value: "5.2 · channel 18" },
    { label: "Colourway", value: colourway(st.color) },
  ]);
</script>

<div class="page">
  <div class="two">
    <div>
      <div class="block">
        <h4>Connected now</h4>
        <div class="devcard" class:on={st.connected}>
          <div>This Mac<span class="lab">{st.connected ? meta : "not connected"}</span></div>
          {#if st.connected && playing}
            <span class="live lab">▶ Playing</span>
          {:else if st.connected}
            <span class="lab">Idle</span>
          {/if}
        </div>
      </div>
      <div class="block">
        <h4>Multipoint</h4>
        <Row label="Connect to two devices" note="Two devices at once">
          <Toggle
            on={multipoint.shown}
            status={multipoint.status}
            label="Connect to two devices"
            onpick={(on) => multipoint.request(on, () => setMultipoint(on))}
          />
        </Row>
        <Row label="Switch to whichever starts playing" note="Host-side rule · modes module">
          <Toggle on={null} status="Unavailable" label="Switch to whichever starts playing" onpick={noop} />
        </Row>
      </div>
    </div>
    <div>
      <div class="block unavailable-block">
        <h4>Paired</h4>
        <LinkList items={[{ label: "This Mac", value: st.connected ? "Connected" : "" }]} />
      </div>
      <div class="block">
        <h4>Pair a new device</h4>
        <p class="hint">Unavailable: the ULT WEAR does not expose its device list.</p>
        <button class="btn fill unavailable" aria-disabled="true" tabindex="-1">Enter pairing mode</button>
      </div>
      <div class="block">
        <h4>This headphone</h4>
        <LinkList items={facts} />
      </div>
    </div>
  </div>
</div>

<style>
  .devcard {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
    align-items: center;
    margin-bottom: 8px;
    padding: 10px 12px;
    border: 1px solid var(--hair);
    font-size: 13px;
  }

  .devcard.on {
    border-color: var(--ink);
  }

  .devcard div .lab {
    display: block;
    margin-top: 2px;
  }

  .hint {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
