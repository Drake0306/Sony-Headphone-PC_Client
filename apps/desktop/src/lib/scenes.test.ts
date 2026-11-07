import { describe, expect, it } from "vitest";
import {
  accelerator,
  actionChips,
  hotkeyChips,
  hotkeyError,
  eqPresetChoices,
  EQ_PRESETS,
  hotkeyList,
  lacksModifier,
  WINDOW_ERROR_KEY,
  WINDOW_SHORTCUT,
  modeBoxes,
  moveScene,
  newScene,
  relevantScenes,
  triggerSentence,
  validateScene,
  validateScenes,
  type Actions,
  type Scene,
} from "./scenes";

const KEEP: Actions = {
  nc: null,
  ambient_level: null,
  voice_focus: null,
  ult: null,
  eq_preset: null,
  volume: null,
  dsee_auto_off: null,
};

const scene = (id: string, over: Partial<Scene> = {}): Scene => ({
  id,
  name: id,
  enabled: true,
  trigger: { kind: "manual" },
  actions: { ...KEEP },
  hotkey: null,
  ...over,
});

describe("triggerSentence", () => {
  it("trigger_sentence_reads_each_kind", () => {
    expect(triggerSentence({ kind: "headphones_connected" })).toBe("Headphones connect");
    expect(triggerSentence({ kind: "time_range", from: "22:00", to: "07:00" })).toBe("Between 22:00 and 07:00");
    expect(triggerSentence({ kind: "frontmost_app", name: "Zoom" })).toBe("Zoom is in front");
    expect(triggerSentence({ kind: "mic_in_use" })).toBe("Microphone is in use");
    expect(triggerSentence({ kind: "manual" })).toBe("By hand only");
  });

  it("trigger_sentence_without_an_app_name_never_starts_blank", () => {
    expect(triggerSentence({ kind: "frontmost_app", name: "" })).toBe("An app is in front");
    expect(triggerSentence({ kind: "frontmost_app", name: "  " })).toBe("An app is in front");
  });
});

describe("actionChips", () => {
  it("action_chips_keep_everything_is_empty", () => {
    expect(actionChips(KEEP)).toEqual([]);
  });

  it("action_chips_follow_the_fixed_order", () => {
    const chips = actionChips({
      nc: "NoiseCancelling",
      ambient_level: null,
      voice_focus: true,
      ult: 1,
      eq_preset: 0x16,
      volume: 12,
      dsee_auto_off: true,
    });
    expect(chips).toEqual([
      { label: "NC" },
      { label: "Voice focus" },
      { label: "ULT 1", ult: true },
      { label: "Bass Boost" },
      { label: "Volume 12" },
      { label: "DSEE auto" },
    ]);
  });

  it("action_chips_merge_ambient_mode_and_level", () => {
    expect(actionChips({ ...KEEP, nc: "Ambient", ambient_level: 14 })).toEqual([{ label: "Ambient 14" }]);
    expect(actionChips({ ...KEEP, nc: "Ambient" })).toEqual([{ label: "Ambient" }]);
    expect(actionChips({ ...KEEP, ambient_level: 5 })).toEqual([{ label: "Ambient 5" }]);
    expect(actionChips({ ...KEEP, nc: "Off" })).toEqual([{ label: "Off" }]);
    expect(actionChips({ ...KEEP, nc: "NoiseCancelling", ambient_level: 3 })).toEqual([
      { label: "NC" },
      { label: "Ambient 3" },
    ]);
  });

  it("action_chips_name_the_off_values", () => {
    expect(actionChips({ ...KEEP, voice_focus: false, ult: 0, dsee_auto_off: false, volume: 0 })).toEqual([
      { label: "Voice focus off" },
      { label: "ULT off" },
      { label: "Volume 0" },
      { label: "DSEE off" },
    ]);
    expect(actionChips({ ...KEEP, ult: 2 })).toEqual([{ label: "ULT 2", ult: true }]);
  });
});

describe("validateScene", () => {
  it("validate_scene_accepts_a_good_scene", () => {
    expect(validateScene(scene("a"))).toBeNull();
    expect(validateScene(newScene())).toBeNull();
    expect(
      validateScene(
        scene("a", {
          trigger: { kind: "time_range", from: "7:05", to: "23:59" },
          actions: { nc: "Ambient", ambient_level: 20, voice_focus: true, ult: 2, eq_preset: 0xa2, volume: 30, dsee_auto_off: false },
        }),
      ),
    ).toBeNull();
  });

  it("validate_scene_mirrors_the_backend_messages", () => {
    expect(validateScene(scene("x", { name: " " }))).toBe('scene " ": name is empty');
    expect(validateScene(scene("Late", { trigger: { kind: "time_range", from: "25:00", to: "07:00" } }))).toBe(
      'scene "Late": from "25:00" is not HH:MM',
    );
    expect(validateScene(scene("Late", { trigger: { kind: "time_range", from: "22:00", to: "7:5" } }))).toBe(
      'scene "Late": to "7:5" is not HH:MM',
    );
    expect(validateScene(scene("X", { trigger: { kind: "frontmost_app", name: "  " } }))).toBe(
      'scene "X": app name is empty',
    );
    expect(validateScene(scene("X", { actions: { ...KEEP, ambient_level: 21 } }))).toBe(
      'scene "X": ambient_level 21 is outside 1..=20',
    );
    expect(validateScene(scene("X", { actions: { ...KEEP, ambient_level: 0 } }))).toBe(
      'scene "X": ambient_level 0 is outside 1..=20',
    );
    expect(validateScene(scene("X", { actions: { ...KEEP, ult: 3 } }))).toBe('scene "X": ult 3 is outside 0..=2');
    expect(validateScene(scene("X", { actions: { ...KEEP, eq_preset: 0x42 } }))).toBe(
      'scene "X": eq_preset 0x42 is not a known preset',
    );
    expect(validateScene(scene("X", { actions: { ...KEEP, volume: 31 } }))).toBe(
      'scene "X": volume 31 is outside 0..=30',
    );
  });

  it("validate_scene_needs_a_modifier_on_the_hotkey", () => {
    for (const hotkey of ["1", "Shift+A", "Space"]) {
      expect(validateScene(scene("X", { hotkey }))).toBe(`scene "X": hotkey ${JSON.stringify(hotkey)} needs Ctrl, Alt or Super`);
      expect(lacksModifier(hotkey)).toBe(true);
    }
    for (const hotkey of ["Alt+1", "F5", "Ctrl+Shift+A", "super+space", "CmdOrCtrl+K"]) {
      expect(validateScene(scene("X", { hotkey }))).toBeNull();
      expect(lacksModifier(hotkey)).toBe(false);
    }
  });

  it("validate_scene_rejects_the_reserved_window_id", () => {
    expect(validateScene(scene(WINDOW_ERROR_KEY, { name: "X" }))).toBe('scene "X": id "__window" is reserved');
  });

  it("validate_scenes_rejects_a_repeated_id", () => {
    expect(validateScenes([scene("a"), scene("b")])).toBeNull();
    expect(validateScenes([scene("a"), scene("a", { name: "X" })])).toBe('scene "X": id "a" is used twice');
    expect(validateScenes([scene("a"), scene("b", { name: "" })])).toBe('scene "": name is empty');
  });
});

describe("eqPresetChoices", () => {
  it("eq_preset_choices_prefer_the_device_list", () => {
    expect(eqPresetChoices([0x00, 0x10, 0x16], null)).toEqual([0x00, 0x10, 0x16]);
    expect(eqPresetChoices([0x00, 0x10, 0x16], 0x10)).toEqual([0x00, 0x10, 0x16]);
    // An id the backend does not accept is left out.
    expect(eqPresetChoices([0x00, 0x42], null)).toEqual([0x00]);
  });

  it("eq_preset_choices_fall_back_and_keep_the_saved_value", () => {
    expect(eqPresetChoices(null, null)).toEqual(EQ_PRESETS);
    expect(eqPresetChoices([], 0x11)).toEqual(EQ_PRESETS);
    expect(eqPresetChoices([0x00, 0x10], 0xa1)).toEqual([0x00, 0x10, 0xa1]);
  });
});

describe("newScene", () => {
  it("new_scene_is_off_manual_and_keeps_everything", () => {
    const s = newScene();
    expect(s.id).toMatch(/^[0-9a-f-]{36}$/);
    expect(s.enabled).toBe(false);
    expect(s.trigger).toEqual({ kind: "manual" });
    expect(s.actions).toEqual(KEEP);
    expect(s.hotkey).toBeNull();
    expect(s.name.trim()).not.toBe("");
    expect(newScene().id).not.toBe(s.id);
  });
});

describe("moveScene", () => {
  const list = [scene("a"), scene("b"), scene("c")];
  const ids = (l: Scene[]) => l.map((s) => s.id);

  it("move_scene_swaps_with_the_neighbour", () => {
    expect(ids(moveScene(list, "b", -1))).toEqual(["b", "a", "c"]);
    expect(ids(moveScene(list, "b", 1))).toEqual(["a", "c", "b"]);
    expect(ids(list)).toEqual(["a", "b", "c"]);
  });

  it("move_scene_stops_at_the_ends_and_ignores_unknown_ids", () => {
    expect(ids(moveScene(list, "a", -1))).toEqual(["a", "b", "c"]);
    expect(ids(moveScene(list, "c", 1))).toEqual(["a", "b", "c"]);
    expect(ids(moveScene(list, "z", 1))).toEqual(["a", "b", "c"]);
  });
});

describe("accelerator", () => {
  const key = (code: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
    code,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...mods,
  });

  it("accelerator_orders_modifiers_then_key", () => {
    expect(accelerator(key("Digit1", { altKey: true, metaKey: true }))).toBe("Alt+Super+1");
    expect(accelerator(key("KeyM", { metaKey: true, shiftKey: true, ctrlKey: true, altKey: true }))).toBe(
      "Ctrl+Alt+Shift+Super+M",
    );
    expect(accelerator(key("F5"))).toBe("F5");
    expect(accelerator(key("ArrowUp", { ctrlKey: true }))).toBe("Ctrl+ArrowUp");
  });

  it("accelerator_waits_for_ctrl_alt_or_super_unless_a_function_key", () => {
    expect(accelerator(key("Digit1"))).toBeNull();
    expect(accelerator(key("KeyA", { shiftKey: true }))).toBeNull();
    expect(accelerator(key("Space"))).toBeNull();
    expect(accelerator(key("Digit1", { altKey: true }))).toBe("Alt+1");
    expect(accelerator(key("F5"))).toBe("F5");
    expect(accelerator(key("F24", { shiftKey: true }))).toBe("Shift+F24");
    expect(accelerator(key("KeyA", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+A");
  });

  it("accelerator_ignores_a_bare_modifier", () => {
    for (const code of ["ShiftLeft", "ControlRight", "AltLeft", "MetaRight", "OSLeft"]) {
      expect(accelerator(key(code, { shiftKey: true }))).toBeNull();
    }
  });
});

describe("relevantScenes", () => {
  it("relevant_scenes_put_the_active_one_first_then_the_first_enabled", () => {
    const list = [scene("a", { enabled: false }), scene("b"), scene("c"), scene("d")];
    expect(relevantScenes({ scenes: list, active: "c" }).map((s) => s.id)).toEqual(["c", "b"]);
    expect(relevantScenes({ scenes: list, active: "b" }).map((s) => s.id)).toEqual(["b", "c"]);
    expect(relevantScenes({ scenes: list, active: null }).map((s) => s.id)).toEqual(["b"]);
    expect(relevantScenes({ scenes: [scene("a", { enabled: false })], active: null })).toEqual([]);
  });
});

describe("modeBoxes", () => {
  it("mode_boxes_list_enabled_scenes_up_to_four", () => {
    const list = [scene("a"), scene("b", { enabled: false }), scene("c")];
    expect(modeBoxes(list)).toEqual({ shown: [list[0], list[2]], more: false });
  });

  it("mode_boxes_add_more_only_past_four", () => {
    const list = ["a", "b", "c", "d", "e"].map((id) => scene(id));
    const boxes = modeBoxes(list);
    expect(boxes.shown.map((s) => s.id)).toEqual(["a", "b", "c", "d"]);
    expect(boxes.more).toBe(true);
    expect(modeBoxes([scene("a", { enabled: false })])).toEqual({ shown: [], more: false });
    expect(modeBoxes(["a", "b", "c", "d"].map((id) => scene(id))).more).toBe(false);
  });
});

describe("hotkeys", () => {
  it("hotkey_chips_use_mac_symbols_on_a_mac", () => {
    expect(hotkeyChips("Ctrl+Alt+Shift+Super+1", true)).toEqual(["⌃", "⌥", "⇧", "⌘", "1"]);
    expect(hotkeyChips("Alt+Super+ArrowUp", false)).toEqual(["Alt", "Super", "ArrowUp"]);
    expect(hotkeyChips(null, true)).toEqual([]);
  });

  it("hotkey_error_reads_the_refusal_for_a_scene", () => {
    const v = { scenes: [], active: null, hotkey_errors: { b: 'already used by scene "A"' } };
    expect(hotkeyError(v, "b")).toBe('already used by scene "A"');
    expect(hotkeyError(v, "a")).toBeNull();
    expect(hotkeyError({ scenes: [], active: null }, "b")).toBeNull();
  });

  it("hotkey_list_is_the_window_shortcut_then_enabled_scenes_with_a_hotkey", () => {
    const list = [
      scene("a", { name: "Desk", hotkey: "Alt+Super+1" }),
      scene("b", { enabled: false, hotkey: "Alt+Super+2" }),
      scene("c"),
      scene("d", { name: "Walk", hotkey: "Ctrl+F5" }),
    ];
    expect(hotkeyList(list, true, {})).toEqual([
      { label: "Open window", keys: ["⌥", "⌘", "N"], error: null },
      { label: "Desk", keys: ["⌥", "⌘", "1"], error: null },
      { label: "Walk", keys: ["⌃", "F5"], error: null },
    ]);
    expect(hotkeyList([scene("c")], true, undefined)).toEqual([
      { label: "Open window", keys: ["⌥", "⌘", "N"], error: null },
    ]);
  });

  it("hotkey_list_marks_a_refused_scene_hotkey", () => {
    const list = [scene("a", { name: "Desk", hotkey: "Alt+Super+1" }), scene("d", { name: "Walk", hotkey: "Alt+Super+1" })];
    expect(hotkeyList(list, true, { d: 'already used by scene "Desk"' })).toEqual([
      { label: "Open window", keys: ["⌥", "⌘", "N"], error: null },
      { label: "Desk", keys: ["⌥", "⌘", "1"], error: null },
      { label: "Walk", keys: ["⌥", "⌘", "1"], error: 'already used by scene "Desk"' },
    ]);
  });

  it("hotkey_list_marks_a_refused_window_shortcut", () => {
    const list = [scene("a", { name: "Desk", hotkey: "Alt+Super+1" })];
    expect(hotkeyList(list, true, { [WINDOW_ERROR_KEY]: "HotKey already registered" })).toEqual([
      { label: "Open window", keys: ["⌥", "⌘", "N"], error: "HotKey already registered" },
      { label: "Desk", keys: ["⌥", "⌘", "1"], error: null },
    ]);
  });

  it("window_shortcut_matches_hotkeys_rs", () => {
    // hotkeys.rs WINDOW_SHORTCUT is asserted to the same literal in tests/hotkeys.rs.
    expect(WINDOW_SHORTCUT).toBe("Alt+Super+N");
  });
});
