<!--
  The main window shell: nav tabs, the connection status, and one screen per
  tab chosen from location.hash (#sound ...; Overview by default).
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { watchState } from "../lib/ipc";
  import { emptyState, type DeviceState } from "../lib/state";
  import Devices from "./Devices.svelte";
  import Noise from "./Noise.svelte";
  import Overview from "./Overview.svelte";
  import Scenes from "./Scenes.svelte";
  import Settings from "./Settings.svelte";
  import Sound from "./Sound.svelte";

  const TABS = [
    { id: "overview", label: "Overview" },
    { id: "sound", label: "Sound" },
    { id: "noise", label: "Noise" },
    { id: "devices", label: "Devices" },
    { id: "scenes", label: "Scenes" },
    { id: "settings", label: "Settings" },
  ] as const;
  type Tab = (typeof TABS)[number]["id"];

  const tabFromHash = (): Tab => TABS.find((t) => `#${t.id}` === location.hash)?.id ?? "overview";

  let tab = $state<Tab>(tabFromHash());
  let st = $state<DeviceState>(emptyState());

  const status = $derived(
    st.connected ? ["Connected", st.firmware ? `FW ${st.firmware}` : ""].filter(Boolean).join(" · ") : "Not connected",
  );

  onMount(() => {
    const onHash = () => (tab = tabFromHash());
    window.addEventListener("hashchange", onHash);
    const stop = watchState((s) => (st = s));
    return () => {
      window.removeEventListener("hashchange", onHash);
      stop();
    };
  });
</script>

<div class="win">
  <nav class="tabs">
    {#each TABS as t (t.id)}
      <a href={`#${t.id}`} class:on={tab === t.id} aria-current={tab === t.id ? "page" : undefined}>{t.label}</a>
    {/each}
  </nav>
  <span class="status lab">{status}</span>
  <main>
    {#if tab === "sound"}
      <Sound {st} />
    {:else if tab === "noise"}
      <Noise {st} />
    {:else if tab === "devices"}
      <Devices {st} />
    {:else if tab === "scenes"}
      <Scenes />
    {:else if tab === "settings"}
      <Settings {st} />
    {:else}
      <Overview {st} />
    {/if}
  </main>
</div>

<style>
  .win {
    position: relative;
    height: 100vh;
    background: var(--paper);
  }

  .tabs {
    position: absolute;
    top: 13px;
    left: 32px;
    display: flex;
    gap: 18px;
  }

  .tabs a {
    font-family: "DM Mono", monospace;
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    text-decoration: none;
  }

  .tabs a.on {
    color: var(--ink);
    text-decoration: underline;
    text-underline-offset: 4px;
  }

  .status {
    position: absolute;
    top: 12px;
    right: 16px;
  }

  main {
    height: 100%;
  }
</style>
