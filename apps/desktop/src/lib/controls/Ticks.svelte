<!-- The tick bar (ambient level). `onpick` receives the 1-based level clicked. -->
<script lang="ts">
  import { isPending, refusedReason, type Status } from "../state";

  interface Props {
    count: number;
    value: number | null;
    status: Status;
    disabled?: boolean;
    onpick: (level: number) => void;
    size?: "sm" | "lg";
    /** Accessible name prefix for each tick, e.g. "Ambient level" → "Ambient level 3". */
    label?: string;
  }

  let { count, value, status, disabled = false, onpick, size = "sm", label = "Level" }: Props = $props();

  const levels = $derived(Array.from({ length: count }, (_, i) => i + 1));
  const off = $derived(disabled || status === "Unavailable");
  const busy = $derived(isPending(status));
  const reason = $derived(refusedReason(status));
</script>

<div
  class="ticks"
  class:lg={size === "lg"}
  class:pending={busy}
  class:refused={reason !== null}
  class:unavailable={off}
  title={reason ?? undefined}
>
  {#each levels as level (level)}
    <button
      class:on={level <= (value ?? 0)}
      aria-label={`${label} ${level}`}
      aria-disabled={off}
      tabindex={off ? -1 : 0}
      onclick={off ? undefined : () => busy || onpick(level)}
    ></button>
  {/each}
</div>
