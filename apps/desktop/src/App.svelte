<!-- Router: `?view=main` is the main window, anything else the popover. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { settings } from "./lib/settings";
  import { applyTheme } from "./lib/theme";
  import Main from "./views/Main.svelte";
  import Popover from "./views/Popover.svelte";

  const view = new URLSearchParams(location.search).get("view") === "main" ? "main" : "popover";

  $effect(() => {
    if ($settings) applyTheme($settings.theme);
  });

  onMount(() => {
    // The main window is opaque paper edge to edge; the popover stays transparent.
    if (view === "main") document.body.classList.add("main");
    // Focus rings only for keyboard users: a Tab press turns them on, a click turns them off.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Tab") document.body.classList.add("kbd");
    };
    const onPointer = () => document.body.classList.remove("kbd");
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
  });
</script>

{#if view === "main"}
  <Main />
{:else}
  <Popover />
{/if}
