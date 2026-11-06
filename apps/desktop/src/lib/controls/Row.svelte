<!--
  A list row. `overview`: mono label · content · aside (the Overview's three
  columns). `setting`: label with an optional note · content (· aside).
-->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    note?: string;
    kind?: "overview" | "setting";
    children?: Snippet;
    aside?: Snippet;
  }

  let { label, note, kind = "setting", children, aside }: Props = $props();
</script>

<div class="r {kind}" class:aside={aside !== undefined}>
  {#if kind === "overview"}
    <span class="lab">{label}</span>
  {:else}
    <span>{label}{#if note}<small>{note}</small>{/if}</span>
  {/if}
  <div class="content">{@render children?.()}</div>
  {#if aside}<span class="sm">{@render aside()}</span>{/if}
</div>

<style>
  .r {
    display: grid;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid var(--hair);
    font-size: 13px;
  }

  .r:last-child {
    border-bottom: none;
  }

  .overview {
    grid-template-columns: 96px 1fr auto;
    padding: 10px 0;
  }

  .setting {
    grid-template-columns: 1fr auto;
    padding: 9px 0;
  }

  .setting.aside {
    grid-template-columns: 1fr auto auto;
  }

  .setting .content {
    justify-self: end;
  }

  small {
    display: block;
    color: var(--muted);
    font-size: 11px;
  }

  .sm {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }
</style>
