<!--
  The main window shell: nav tabs, the connection status, and one screen per
  tab chosen from location.hash (#sound ...; Overview by default).
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getRuntimeInfo, onNavigate, OPEN_TAB_KEY, watchState } from "../lib/ipc";
  import { emptyState, statusLabel, type DeviceState } from "../lib/state";
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

  // The popover asks for a tab with the "navigate" event, and leaves it in
  // localStorage for a window it has just created (`openMainWindowOn`).
  function show(id: string) {
    try {
      localStorage.removeItem(OPEN_TAB_KEY);
    } catch {
      // Storage may be blocked; the hash still switches.
    }
    if (TABS.some((t) => t.id === id)) location.hash = `#${id}`;
  }

  function takeRequestedTab() {
    let id: string | null = null;
    try {
      id = localStorage.getItem(OPEN_TAB_KEY);
    } catch {
      return;
    }
    if (id !== null) show(id);
  }

  let tab = $state<Tab>(tabFromHash());
  let st = $state<DeviceState>(emptyState());
  let fake = $state(false);

  const status = $derived(statusLabel(st.connected, st.firmware, fake));

  onMount(() => {
    const onHash = () => (tab = tabFromHash());
    window.addEventListener("hashchange", onHash);
    takeRequestedTab();
    const unlisten = onNavigate(show);
    const stop = watchState((s) => (st = s));
    getRuntimeInfo()
      .then((info) => (fake = info.fake))
      .catch((e) => console.error(e));
    return () => {
      window.removeEventListener("hashchange", onHash);
      unlisten.then((off) => off()).catch(() => {});
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
      <Scenes {st} />
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
