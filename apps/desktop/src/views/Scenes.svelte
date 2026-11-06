<!-- Scenes: static placeholder cards until the modes module lands. -->
<script lang="ts">
  import { SCENES_PREVIEW } from "../lib/scenesPreview";
</script>

<div class="page">
  <div class="two">
    <div>
      <div class="block">
        <h4>Scenes · first match wins</h4>
        {#each SCENES_PREVIEW as card (card.name)}
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
