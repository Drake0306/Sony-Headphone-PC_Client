<!--
  Scenes: the list on the left (first match wins; Up / Down set the priority,
  Apply runs a scene by hand) and the editor for one scene on the right.
  Save scene and Delete send the whole list.
-->
<script lang="ts">
  import SceneCard from "../lib/controls/SceneCard.svelte";
  import SceneEditor from "../lib/controls/SceneEditor.svelte";
  import { applyScene, moveScene, newScene, saveScenes, scenes, validateScene, type Scene } from "../lib/scenes";

  let draft = $state<Scene | null>(null);
  // Bumped on every open, so the editor starts a fresh working copy.
  let opened = $state(0);
  let picked = false;
  let saving = $state(false);
  let error = $state<string | null>(null);
  let listError = $state<string | null>(null);
  let applying = $state<string | null>(null);

  const list = $derived($scenes.scenes);
  const isNew = $derived(draft !== null && !list.some((s) => s.id === draft?.id));

  // Open the first scene once the list arrives.
  $effect(() => {
    if (!picked && list.length > 0) {
      picked = true;
      open(list[0]);
    }
  });

  // Apply is pending until the next "scenes" event.
  $effect(() => {
    void $scenes;
    applying = null;
  });

  function open(s: Scene | null) {
    draft = s;
    opened += 1;
    error = null;
  }

  async function send(next: Scene[]): Promise<string | null> {
    saving = true;
    const result = await saveScenes(next);
    saving = false;
    return result;
  }

  async function save(scene: Scene) {
    if (saving) return;
    error = validateScene(scene);
    if (error !== null) return;
    const next = isNew ? [...list, scene] : list.map((s) => (s.id === scene.id ? scene : s));
    error = await send(next);
    if (error === null) draft = scene;
  }

  async function remove() {
    if (!draft || saving) return;
    const id = draft.id;
    if (isNew) {
      open(list[0] ?? null);
      return;
    }
    const at = list.findIndex((s) => s.id === id);
    const next = list.filter((s) => s.id !== id);
    error = await send(next);
    if (error === null) open(next[Math.min(at, next.length - 1)] ?? null);
  }

  async function move(id: string, dir: -1 | 1) {
    if (saving) return;
    listError = await send(moveScene(list, id, dir));
  }

  function apply(id: string) {
    listError = null;
    applying = id;
    applyScene(id).catch((e) => {
      applying = null;
      listError = String(e);
    });
  }
</script>

<div class="page">
  <div class="two cols">
    <div class="col">
      <div class="block">
        <h4>Scenes · first match wins</h4>
        {#each list as scene, i (scene.id)}
          <SceneCard
            {scene}
            active={scene.id === $scenes.active}
            selected={scene.id === draft?.id}
            onselect={() => open(scene)}
          >
            <button
              class="btn"
              class:unavailable={i === 0 || saving}
              aria-disabled={i === 0 || saving}
              aria-label={`Move ${scene.name} up`}
              onclick={() => i > 0 && move(scene.id, -1)}>Up</button
            >
            <button
              class="btn"
              class:unavailable={i === list.length - 1 || saving}
              aria-disabled={i === list.length - 1 || saving}
              aria-label={`Move ${scene.name} down`}
              onclick={() => i < list.length - 1 && move(scene.id, 1)}>Down</button
            >
            <button
              class="btn"
              class:unavailable={applying !== null}
              aria-busy={applying === scene.id}
              aria-label={`Apply ${scene.name}`}
              onclick={() => applying === null && apply(scene.id)}>{applying === scene.id ? "Applying" : "Apply"}</button
            >
          </SceneCard>
        {/each}
        {#if listError}<p class="note live">{listError}</p>{/if}
        <p class="note">The first enabled scene whose trigger matches is applied; Up and Down set the order.</p>
        <div class="btnrow"><button class="btn fill" onclick={() => open(newScene())}>New scene</button></div>
      </div>
    </div>
    <div class="col">
      {#if draft}
        {#key opened}
          <SceneEditor {draft} {isNew} busy={saving} {error} onSave={save} onDelete={remove} />
        {/key}
      {/if}
    </div>
  </div>
</div>

<style>
  .cols {
    grid-template-columns: 1fr 1.3fr;
    height: 100%;
  }

  .col {
    overflow-y: auto;
    padding-right: 4px;
  }
</style>
