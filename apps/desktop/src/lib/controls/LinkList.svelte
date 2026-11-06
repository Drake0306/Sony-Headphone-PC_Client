<!-- The mono two-column list (devices, facts). A `live` value is drawn orange. -->
<script lang="ts">
  interface Item {
    label: string;
    value: string;
    live?: boolean;
  }

  let { items }: { items: Item[] } = $props();
</script>

<div class="link">
  {#each items as item, i (i)}
    <div>
      <span>{item.label}</span>
      <span class:live={item.live}>{item.value}</span>
    </div>
  {/each}
</div>
