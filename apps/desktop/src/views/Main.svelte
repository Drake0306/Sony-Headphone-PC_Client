<!--
  The main window shell: nav tabs, the connection status, and one screen per
  tab chosen from location.hash (#sound ...; Overview by default). The
  Advanced equalizer opens from Sound in place of the screen, with the nav
  hidden; it is not a tab or a hash, so a reload or any tab change goes back
  to the screens.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { getRuntimeInfo, onNavigate, OPEN_TAB_KEY, watchState } from "../lib/ipc";
  import { settings } from "../lib/settings";
  import { emptyState, statusLabel, type DeviceState } from "../lib/state";
  import Advanced from "./Advanced.svelte";
  import AdvancedEq from "./AdvancedEq.svelte";
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
    { id: "advanced", label: "Advanced" },
  ] as const;
  type Tab = (typeof TABS)[number]["id"];

  // Advanced is listed (last) only while Settings enables it.
  const tabs = $derived(TABS.filter((t) => t.id !== "advanced" || $settings?.show_advanced === true));
  const hashTab = (): Tab => TABS.find((t) => `#${t.id}` === location.hash)?.id ?? "overview";

  // The popover asks for a tab with the "navigate" event, and leaves it in
  // localStorage for a window it has just created (`openMainWindowOn`).
  function show(id: string) {
    // Same tab or not, a request from the popover leaves the Advanced equalizer.
    advancedEq = false;
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

  let requested = $state<Tab>(hashTab());
  const tab = $derived<Tab>(tabs.some((t) => t.id === requested) ? requested : "overview");
  let advancedEq = $state(false);
  const eqOpen = $derived(advancedEq && tab === "sound");
  let st = $state<DeviceState>(emptyState());
  let fake = $state(false);

  const status = $derived(statusLabel(st.connected, st.firmware, fake));

  // Scroll position of the content column, for the bar's hairline.
  let scroller = $state<HTMLElement | null>(null);
  let scrolled = $state(false);
  $effect(() => {
    void tab;
    void advancedEq;
    scrolled = (scroller?.scrollTop ?? 0) > 0;
  });

  onMount(() => {
    const onHash = () => {
      advancedEq = false;
      requested = hashTab();
    };
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
  <!-- The bar is glass: content scrolling under it shows blurred, never
       sharp, and a hairline appears along its foot once anything has scrolled
       beneath. The Advanced equalizer takes the whole area and never scrolls,
       so there the bar is plain and only the status remains. -->
  <header class="bar" class:scrolled class:plain={eqOpen}>
    {#if !eqOpen}
      <nav class="tabs">
        {#each tabs as t (t.id)}
          <a href={`#${t.id}`} class:on={tab === t.id} aria-current={tab === t.id ? "page" : undefined}>{t.label}</a>
        {/each}
      </nav>
    {:else}
      <span></span>
    {/if}
    <span class="status lab">{status}</span>
  </header>
  <main bind:this={scroller} onscroll={() => (scrolled = (scroller?.scrollTop ?? 0) > 0)}>
    {#if eqOpen}
      <AdvancedEq {st} onback={() => (advancedEq = false)} />
    {:else if tab === "sound"}
      <Sound {st} onadvanced={() => (advancedEq = true)} />
    {:else if tab === "noise"}
      <Noise {st} />
    {:else if tab === "devices"}
      <Devices {st} />
    {:else if tab === "scenes"}
      <Scenes {st} />
    {:else if tab === "settings"}
      <Settings {st} />
    {:else if tab === "advanced"}
      <Advanced {st} />
    {:else}
      <Overview {st} />
    {/if}
  </main>
</div>

<style>
  .win {
    position: relative;
    height: 100vh;
    background: var(--wash), var(--paper);
  }

  .bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 2;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px 0 32px;
    /* Paper at 72 %, the rest blurred content; a flat paper first for a
       WebKit without color-mix. */
    background: var(--paper);
    background: color-mix(in srgb, var(--paper) 72%, transparent);
    -webkit-backdrop-filter: blur(16px) saturate(1.3);
    backdrop-filter: blur(16px) saturate(1.3);
    border-bottom: 1px solid transparent;
    transition: border-color 120ms ease-out;
  }

  .bar.scrolled {
    border-bottom-color: var(--hair);
  }

  .bar.plain {
    background: none;
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
    pointer-events: none;
  }

  .tabs {
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
    pointer-events: none;
  }

  /* The content column scrolls inside the window so the paper never ends
     above tall screens such as the Advanced panel. */
  main {
    height: 100%;
    overflow-y: auto;
  }
</style>
