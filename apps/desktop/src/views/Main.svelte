<!--
  The main window shell: nav tabs, the connection status, and one body per
  tab chosen from location.hash (#sound ...; Overview by default). The real
  screens replace the placeholders in later tasks.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { watchState } from "../lib/ipc";
  import { emptyState, type DeviceState } from "../lib/state";

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
  const title = $derived(TABS.find((t) => t.id === tab)?.label ?? "");

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
  <main class="page">
    {#if tab === "scenes"}
      <h3>Scenes</h3>
      <p class="sub">Scenes arrive with the modes module.</p>
    {:else}
      <h3>{title}</h3>
      <p class="sub">This screen is on its way.</p>
    {/if}
  </main>
</div>

<style>
  .win {
    position: relative;
    min-height: 100vh;
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

  .page {
    padding: 48px 32px 24px;
  }

  h3 {
    margin: 0 0 4px;
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .sub {
    margin: 0 0 18px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
