<!--
  Noise: the big switch, the ambient level and Focus on voice on the left;
  the cup-button block (not readable yet) and the Scenes placeholder on the right.
-->
<script lang="ts">
  import Boxes from "../lib/controls/Boxes.svelte";
  import Row from "../lib/controls/Row.svelte";
  import Ticks from "../lib/controls/Ticks.svelte";
  import Toggle from "../lib/controls/Toggle.svelte";
  import { control } from "../lib/control.svelte";
  import * as noise from "../lib/noise";
  import { SCENES_PREVIEW } from "../lib/scenesPreview";
  import { refusedReason, type DeviceState, type NcAsmState, type NcMode } from "../lib/state";

  let { st }: { st: DeviceState } = $props();

  const NOISE: { label: string; value: NcMode }[] = [
    { label: "Noise cancelling", value: "NoiseCancelling" },
    { label: "Ambient", value: "Ambient" },
    { label: "Off", value: "Off" },
  ];
  const CUP = ["Noise cancelling", "Ambient", "Off"];
  const noop = () => {};

  const nc = control<NcAsmState>(() => st.nc);
  const shown = $derived(nc.shown);
  const ambient = $derived(shown?.mode === "Ambient");
  // Voice focus never shows the refusal outline; the noise switch carries it.
  const voiceStatus = $derived(refusedReason(nc.status) !== null ? "Confirmed" : nc.status);
</script>

<div class="page">
  <div class="two">
    <div>
      <div class="block">
        <h4>Noise control</h4>
        <Boxes
          options={NOISE}
          value={shown?.mode ?? null}
          attempted={nc.refused?.value.mode ?? null}
          status={nc.status}
          size="lg"
          onpick={(mode) => noise.pickNoise(nc, st.nc.value, mode)}
        />
      </div>
      <div class="block">
        <h4>Ambient level</h4>
        <div class="level">
          <Ticks
            count={20}
            value={shown?.level ?? null}
            status={nc.status}
            disabled={!ambient}
            size="lg"
            label="Ambient level"
            onpick={(level) => noise.pickLevel(nc, st.nc.value, level)}
          />
          <span class="dot num" class:off={!ambient}>{shown?.level ?? "--"}</span>
        </div>
        <div class="ends"><span class="lab">1 · quiet room</span><span class="lab">20 · street</span></div>
      </div>
      <div class="block">
        <Row label="Focus on voice" note="Keeps speech, trims the rest of the ambient feed">
          <Toggle
            on={shown?.voice_focus ?? null}
            status={voiceStatus}
            disabled={!ambient}
            label="Focus on voice"
            onpick={(on) => noise.pickVoice(nc, shown, on)}
          />
        </Row>
      </div>
    </div>
    <div>
      <div class="block">
        <h4>Cup button cycles through</h4>
        {#each CUP as mode (mode)}
          <Row label={mode}><Toggle on={null} status="Unavailable" label={mode} onpick={noop} /></Row>
        {/each}
        <p class="note">Unavailable: not readable on this model yet.</p>
      </div>
      <div class="block">
        <h4>Automatic</h4>
        {#each SCENES_PREVIEW as card (card.name)}
          <div class="scene">
            <span>{card.name}</span><span class="lab">Preview</span>
            <span class="when">{card.when}</span>
            <div class="then">
              {#each card.then as action (action.label)}<span class:u={action.ult}>{action.label}</span>{/each}
            </div>
          </div>
        {/each}
        <p class="note">Modes arrive after Phase 2.</p>
      </div>
    </div>
  </div>
</div>

<style>
  .level {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .num {
    font-size: 34px;
    line-height: 1;
  }

  .num.off {
    opacity: 0.35;
  }

  .ends {
    display: flex;
    justify-content: space-between;
    margin-top: 6px;
  }
</style>
