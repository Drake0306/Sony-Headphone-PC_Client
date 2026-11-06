<!--
  The square on/off toggle. `onpick` receives the value to switch to.
  `readonly` makes it an indicator: the live value at full opacity with a
  muted knob, not focusable and not clickable.
-->
<script lang="ts">
  import { isPending, refusedReason, type Status } from "../state";

  interface Props {
    on: boolean | null;
    status?: Status;
    disabled?: boolean;
    onpick?: (on: boolean) => void;
    /** Accessible name when no visible label is associated. */
    label?: string;
    /** Show the value only; nothing here can change it. */
    readonly?: boolean;
    /** Tooltip for a read-only indicator. */
    title?: string;
  }

  let {
    on,
    status = "Confirmed",
    disabled = false,
    onpick = () => {},
    label,
    readonly = false,
    title = "Reported by the headphones",
  }: Props = $props();

  const off = $derived(readonly ? on === null : disabled || status === "Unavailable");
  const busy = $derived(!readonly && isPending(status));
  const reason = $derived(readonly ? null : refusedReason(status));
</script>

<button
  class="toggle"
  class:on={on === true}
  class:pending={busy}
  class:refused={reason !== null}
  class:unavailable={off}
  class:readonly
  role="switch"
  aria-checked={on === true}
  aria-label={label}
  title={readonly ? title : (reason ?? undefined)}
  aria-disabled={off || readonly}
  tabindex={off || readonly ? -1 : 0}
  onclick={off || readonly ? undefined : () => busy || onpick(!on)}
></button>
