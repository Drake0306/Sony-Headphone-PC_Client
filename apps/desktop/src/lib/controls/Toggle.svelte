<!-- The square on/off toggle. `onpick` receives the value to switch to. -->
<script lang="ts">
  import { isPending, refusedReason, type Status } from "../state";

  interface Props {
    on: boolean | null;
    status: Status;
    disabled?: boolean;
    onpick: (on: boolean) => void;
    /** Accessible name when no visible label is associated. */
    label?: string;
  }

  let { on, status, disabled = false, onpick, label }: Props = $props();

  const off = $derived(disabled || status === "Unavailable");
  const busy = $derived(isPending(status));
  const reason = $derived(refusedReason(status));
</script>

<button
  class="toggle"
  class:on={on === true}
  class:pending={busy}
  class:refused={reason !== null}
  class:unavailable={off}
  role="switch"
  aria-checked={on === true}
  aria-label={label}
  title={reason ?? undefined}
  aria-disabled={off}
  tabindex={off ? -1 : 0}
  onclick={off ? undefined : () => busy || onpick(!on)}
></button>
