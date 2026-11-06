<!--
  The boxed switch (NC / AMB / OFF, ULT, presets...). Draws the shared status
  rules: Confirmed normal, Pending striped and inert, Refused outlined in
  orange with the reason as a tooltip, Unavailable faded and inert.
-->
<script lang="ts" generics="T">
  import { isPending, refusedReason, type Status } from "../state";

  interface Option {
    label: string;
    value: T;
    /** The active box is the orange ULT one. */
    orange?: boolean;
    /** Tooltip for the box (a refusal reason takes its place). */
    title?: string;
    /** This box alone is faded and inert (e.g. a value shown but not settable). */
    inert?: boolean;
  }

  interface Props {
    options: Option[];
    value: T | null;
    status: Status;
    /** Inert and faded, like Unavailable (e.g. a control that does not apply in this mode). */
    disabled?: boolean;
    onpick: (v: T) => void;
    width?: string;
    size?: "sm" | "lg";
    /** The value whose request was refused; defaults to the active value. */
    attempted?: T | null;
  }

  let { options, value, status, disabled = false, onpick, width, size = "sm", attempted = null }: Props = $props();

  const off = $derived(disabled || status === "Unavailable");
  const busy = $derived(isPending(status));
  const reason = $derived(refusedReason(status));
  const outlined = $derived(attempted ?? value);
</script>

<div class="boxes" class:lg={size === "lg"} class:pending={busy} class:unavailable={off} style:width>
  {#each options as option (option.label)}
    {@const refused = reason !== null && option.value === outlined}
    {@const dead = off || option.inert === true}
    <button
      class:on={option.value === value}
      class:orange={option.orange}
      class:refused
      class:inert={option.inert}
      title={refused ? reason : option.title}
      aria-pressed={option.value === value}
      aria-disabled={dead}
      tabindex={dead ? -1 : 0}
      onclick={dead ? undefined : () => busy || onpick(option.value)}>{option.label}</button
    >
  {/each}
</div>
