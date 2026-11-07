<!--
  One scene card: name, Active / On / Off label, the trigger sentence and the
  action chips (ULT chips orange). With `onselect` the card is a button;
  `children` (e.g. Up / Down / Apply) render under the chips and do not select.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { actionChips, triggerSentence, type Scene } from "../scenes";

  interface Props {
    scene: Scene;
    active: boolean;
    /** Open in the editor. */
    selected?: boolean;
    onselect?: () => void;
    children?: Snippet;
  }

  let { scene, active, selected = false, onselect, children }: Props = $props();

  const chips = $derived(actionChips(scene.actions));
  const label = $derived(active ? "Active" : scene.enabled ? "On" : "Off");

  // Clicks and keys on the card's own buttons do not select it.
  const own = (e: Event) => !(e.target as Element | null)?.closest("button");

  function onclick(e: MouseEvent) {
    if (own(e)) onselect?.();
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.key === "Enter" || e.key === " ") && own(e)) {
      e.preventDefault();
      onselect?.();
    }
  }
</script>

{#if onselect}
  <div class="scene pick" class:on={active} class:sel={selected} role="button" tabindex="0" aria-pressed={selected} {onclick} {onkeydown}>
    {@render body()}
  </div>
{:else}
  <div class="scene" class:on={active}>{@render body()}</div>
{/if}

{#snippet body()}
  <span>{scene.name}</span><span class="lab" class:act={active}>{label}</span>
  <span class="when">{triggerSentence(scene.trigger)}</span>
  {#if chips.length > 0}
    <div class="then">
      {#each chips as chip, i (i)}<span class:u={chip.ult}>{chip.label}</span>{/each}
    </div>
  {/if}
  {#if children}<div class="tools">{@render children()}</div>{/if}
{/snippet}

<style>
  .pick {
    cursor: pointer;
  }

  .sel {
    box-shadow: inset 3px 0 0 var(--ink);
  }

  .act {
    color: var(--ink);
  }

  .tools {
    grid-column: 1 / -1;
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }
</style>
