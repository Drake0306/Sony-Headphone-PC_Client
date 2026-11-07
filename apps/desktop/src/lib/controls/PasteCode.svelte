<!--
  Paste a code: a modal with a field, the code decoded as it is typed (the
  error in muted text, or a preview of the values), and Apply. A Sound code
  (six bands, ULT, maybe a slot) goes to the headphones only through the
  Confirm dialog, in one EQ write into the slot it names (Manual, Custom 1 or
  Custom 2) or else the preset Send best fit would use. An Advanced code (ten
  bands) is saved into the app's curve through `savecurve` and nothing is
  sent; `onsaved` then lets the owner show it. Escape, Close or a click on the
  backdrop leaves without writing.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import Confirm from "./Confirm.svelte";
  import { faderText } from "./Fader.svelte";
  import {
    ADVANCED_BAND_LABELS,
    DEVICE_BAND_LABELS,
    fromRaw,
    shapeProblem,
    targetPreset,
    targetProblem,
    type TargetPreset,
  } from "../eqCurve";
  import { setEqCurve, setEqPresetBands } from "../ipc";
  import { decode, slotPreset, type Decoded, type SoundCode } from "../presetCode";
  import { isPending, presetName, type DeviceState } from "../state";

  interface Props {
    st: DeviceState;
    onclose: () => void;
    /** Saves an Advanced code's ten bands in the app; `set_eq_curve` by default. */
    savecurve?: (bands: number[]) => Promise<unknown>;
    /** After an Advanced code was saved. */
    onsaved?: (bands: number[]) => void;
  }

  let { st, onclose, savecurve = setEqCurve, onsaved }: Props = $props();

  let panel = $state<HTMLElement>();
  let field = $state<HTMLInputElement>();
  let text = $state("");
  let asking = $state(false);
  let working = $state(false);
  let problem = $state<string | null>(null);
  const id = `paste-${Math.random().toString(36).slice(2)}`;

  const decoded = $derived<Decoded | null>(text.trim() === "" ? null : decode(text));
  const sound = $derived(decoded && "kind" in decoded && decoded.kind === "sound" ? decoded : null);
  const advanced = $derived(decoded && "kind" in decoded && decoded.kind === "advanced" ? decoded : null);
  const error = $derived(decoded && "error" in decoded ? decoded.error : null);

  const current = $derived(st.eq_ult.value?.preset ?? null);
  /** The slot the code names, else the Send best fit rule. */
  function target(code: SoundCode): TargetPreset {
    const named = slotPreset(code.slot);
    if (named === null) return targetPreset(current);
    const switches = current !== named;
    return {
      id: named,
      label: switches ? `${presetName(named)} (the headphones will switch to it)` : presetName(named),
      switches,
    };
  }
  const soundTarget = $derived(sound ? target(sound) : null);
  /** Why a Sound code cannot go to the headphones now, or null. */
  const soundBlock = $derived.by(() => {
    if (!sound || !soundTarget) return null;
    if (!st.connected || st.eq_ult.status === "Unavailable" || st.eq_ult.value === null) {
      return "Connect the headphones to apply this preset.";
    }
    return shapeProblem(st.eq_capability) ?? targetProblem(soundTarget, st.eq_capability);
  });
  const busy = $derived(isPending(st.eq_ult.status));
  const applyOff = $derived(working || (!sound && !advanced) || (sound !== null && (soundBlock !== null || busy)));

  const level = (v: number) => faderText(v, 0);
  const ultText = (n: number) => (n === 0 ? "ULT off" : `ULT ${n}`);

  onMount(() => {
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    field?.focus();
    return () => {
      if (before?.isConnected) before.focus();
    };
  });

  function key(e: KeyboardEvent) {
    // While the confirmation is open it handles the keys.
    if (asking) return;
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key !== "Tab" || !panel) return;
    const stops = [...panel.querySelectorAll<HTMLElement>("input, button:not([aria-disabled='true'])")];
    const at = stops.indexOf(document.activeElement as HTMLElement);
    e.preventDefault();
    stops[(at + (e.shiftKey ? -1 : 1) + stops.length) % stops.length]?.focus();
  }

  async function apply() {
    if (applyOff) return;
    problem = null;
    if (sound) {
      asking = true;
      return;
    }
    if (!advanced) return;
    const bands = [...advanced.bands];
    working = true;
    try {
      await savecurve(bands);
      onsaved?.(bands);
      onclose();
    } catch (e) {
      problem = `Not saved: ${e}`;
      console.error(e);
    } finally {
      working = false;
    }
  }

  async function send() {
    asking = false;
    if (!sound || !soundTarget || soundBlock !== null || busy) return;
    working = true;
    try {
      await setEqPresetBands(soundTarget.id, sound.ult, [...sound.bands]);
      onclose();
    } catch (e) {
      problem = `Not sent: ${e}`;
      console.error(e);
    } finally {
      working = false;
    }
  }
</script>

<svelte:window onkeydown={key} />

{#if asking && sound && soundTarget}
  <Confirm
    title="Apply this preset to the headphones?"
    body={`Hairline writes these six bands and ${ultText(sound.ult)} into ${soundTarget.label}.`}
    details={DEVICE_BAND_LABELS.map((label, k) => ({ label, value: level(fromRaw(sound?.bands[k] ?? 10)) }))}
    confirmLabel="Apply"
    onconfirm={send}
    oncancel={() => (asking = false)}
  />
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
    <div class="panel" role="dialog" aria-modal="true" aria-labelledby={`${id}-title`} bind:this={panel}>
      <h4 id={`${id}-title`}>Paste a code</h4>
      <input
        class="field"
        type="text"
        aria-label="Share code"
        placeholder="XXXXX-XXXXX-XXXXX"
        spellcheck="false"
        autocomplete="off"
        bind:this={field}
        bind:value={text}
      />
      <div class="result" aria-live="polite">
        {#if error}
          <p class="note">{error}</p>
        {:else if sound}
          <dl class="preview six">
            {#each DEVICE_BAND_LABELS as label, k (label)}
              <div><dt class="lab">{label}</dt><dd class="dot">{level(fromRaw(sound.bands[k]))}</dd></div>
            {/each}
          </dl>
          <p class="note">{ultText(sound.ult)} · into {soundTarget?.label}</p>
          {#if soundBlock}<p class="note">{soundBlock}</p>{/if}
        {:else if advanced}
          <dl class="preview ten">
            {#each ADVANCED_BAND_LABELS as label, k (label)}
              <div><dt class="lab">{label}</dt><dd class="dot">{level(advanced.bands[k])}</dd></div>
            {/each}
          </dl>
          <p class="note">Saved into the Advanced equalizer; nothing is sent to the headphones.</p>
        {/if}
        {#if problem}<p class="note">{problem}</p>{/if}
      </div>
      <div class="actions">
        <button class="btn" onclick={onclose}>Close</button>
        <button
          class="btn fill"
          class:unavailable={applyOff}
          aria-disabled={applyOff}
          tabindex={applyOff ? -1 : 0}
          onclick={apply}>Apply</button
        >
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    background: color-mix(in srgb, var(--paper) 72%, transparent);
  }

  .panel {
    width: 100%;
    max-width: 520px;
    padding: 20px 22px 18px;
    border: 1px solid var(--ink);
    background: var(--paper);
    color: var(--ink);
  }

  h4 {
    margin: 0 0 12px;
    font-size: 15px;
    font-weight: 600;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    font-size: 14px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .field::placeholder {
    letter-spacing: 0.08em;
  }

  .result {
    min-height: 64px;
  }

  .note {
    margin: 8px 0 0;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .preview {
    display: grid;
    gap: 6px;
    margin: 12px 0 0;
    padding: 10px 0;
    border-top: 1px solid var(--hair);
    border-bottom: 1px solid var(--hair);
  }

  .preview.six {
    grid-template-columns: repeat(6, 1fr);
  }

  .preview.ten {
    grid-template-columns: repeat(10, 1fr);
  }

  .preview div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: center;
    text-align: center;
  }

  .preview dt {
    color: var(--ink);
  }

  .preview dd {
    margin: 0;
    font-size: 16px;
    line-height: 18px;
  }

  .actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 16px;
  }
</style>
