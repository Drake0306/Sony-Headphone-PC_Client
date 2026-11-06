<!-- Scenes: static placeholder cards until the modes module lands. -->
<script lang="ts">
  const CARDS: { name: string; when: string; then: { label: string; ult?: boolean }[] }[] = [
    {
      name: "At the desk",
      when: "This Mac is playing",
      then: [{ label: "Ambient 14" }, { label: "Voice focus" }, { label: "ULT 1", ult: true }],
    },
    { name: "On a call", when: "Zoom or FaceTime has the mic", then: [{ label: "NC" }, { label: "ULT off" }] },
  ];
</script>

<div class="page">
  <div class="two">
    <div>
      <div class="block">
        <h4>Scenes · first match wins</h4>
        {#each CARDS as card (card.name)}
          <div class="scene">
            <span>{card.name}</span><span class="lab">Preview</span>
            <span class="when">{card.when}</span>
            <div class="then">
              {#each card.then as action (action.label)}<span class:u={action.ult}>{action.label}</span>{/each}
            </div>
          </div>
        {/each}
        <p class="note">Modes arrive after Phase 2.</p>
      </div>
    </div>
  </div>
</div>
