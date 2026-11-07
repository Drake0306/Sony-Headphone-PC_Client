import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import SceneEditor from "./SceneEditor.svelte";
import { newScene, type Scene } from "../scenes";

const setup = () => {
  const onSave = vi.fn<(s: Scene) => void>();
  const r = render(SceneEditor, { draft: newScene(), onSave, onDelete: () => {} });
  return { ...r, onSave };
};

describe("SceneEditor", () => {
  it("scene_editor_records_a_hotkey_from_the_window", async () => {
    const { getByText, getByLabelText, onSave } = setup();
    await fireEvent.click(getByText("Record"));
    expect(getByText("Press keys")).toBeInTheDocument();
    await fireEvent.keyDown(window, { code: "AltLeft", key: "Alt", altKey: true });
    expect(getByText("Press keys")).toBeInTheDocument();
    await fireEvent.keyDown(window, { code: "Digit1", key: "1", altKey: true, metaKey: true });
    const chips = [...getByLabelText("Hotkey").querySelectorAll("span")].map((c) => c.textContent);
    expect(chips).toHaveLength(3);
    expect(chips[2]).toBe("1");
    expect(getByText("Record")).toBeInTheDocument();
    await fireEvent.click(getByText("Save scene"));
    expect(onSave.mock.calls[0][0].hotkey).toBe("Alt+Super+1");
  });

  it("scene_editor_waits_for_a_modifier_and_says_so", async () => {
    const { getByText, getByLabelText, queryByText, queryByLabelText } = setup();
    await fireEvent.click(getByText("Record"));
    expect(queryByText("Add Ctrl, Alt or Super")).toBeNull();
    await fireEvent.keyDown(window, { code: "KeyA", key: "A", shiftKey: true });
    expect(getByText("Press keys")).toBeInTheDocument();
    expect(getByText("Add Ctrl, Alt or Super")).toBeInTheDocument();
    expect(queryByLabelText("Hotkey")).toBeNull();
    await fireEvent.keyDown(window, { code: "F5", key: "F5" });
    expect(getByText("Record")).toBeInTheDocument();
    expect(queryByText("Add Ctrl, Alt or Super")).toBeNull();
    expect(getByLabelText("Hotkey")).toBeInTheDocument();
  });

  it("scene_editor_notes_that_headphones_connect_holds", async () => {
    const { getByText, queryByText } = setup();
    const note = "Holds for as long as they stay connected, so put it below more specific scenes.";
    await fireEvent.click(getByText("Change"));
    expect(queryByText(note)).toBeNull();
    await fireEvent.click(getByText("Headphones connect"));
    expect(getByText(note)).toHaveClass("note");
  });

  it("scene_editor_offers_the_device_presets_and_the_saved_one", async () => {
    const draft = { ...newScene(), actions: { ...newScene().actions, eq_preset: 0xa1 } };
    const r = render(SceneEditor, { draft, devicePresets: [0x00, 0x16], onSave: () => {}, onDelete: () => {} });
    expect(r.getByText("Off")).toBeInTheDocument();
    expect(r.getByText("Bass Boost")).toBeInTheDocument();
    expect(r.getByText("Custom 1")).toBeInTheDocument();
    expect(r.queryByText("Bright")).toBeNull();
  });

  it("scene_editor_escape_and_blur_stop_recording", async () => {
    const { getByText, queryByLabelText } = setup();
    await fireEvent.click(getByText("Record"));
    await fireEvent.keyDown(window, { code: "Escape", key: "Escape" });
    expect(getByText("Record")).toBeInTheDocument();
    await fireEvent.click(getByText("Record"));
    await fireEvent.blur(window);
    expect(getByText("Record")).toBeInTheDocument();
    await fireEvent.keyDown(window, { code: "KeyA", key: "a", ctrlKey: true });
    expect(queryByLabelText("Hotkey")).toBeNull();
  });

  it("scene_editor_volume_zero_and_keep_boxes", async () => {
    const { getByText, getByLabelText, getAllByText, onSave } = setup();
    await fireEvent.click(getByLabelText("Volume 12"));
    await fireEvent.click(getByText("0", { selector: "button" }));
    await fireEvent.click(getByText("Save scene"));
    expect(onSave.mock.calls[0][0].actions.volume).toBe(0);
    expect(getByLabelText("Volume 1")).not.toHaveClass("on");
    expect(getByText("0", { selector: "button" })).toHaveClass("on");
    // The volume row's Keep box is the last Keep in the row order before DSEE.
    const keeps = getAllByText("Keep");
    const volumeKeep = keeps.find((k) => k.closest(".boxes")?.contains(getByText("0", { selector: "button" })));
    await fireEvent.click(volumeKeep!);
    await fireEvent.click(getByText("Save scene"));
    expect(onSave.mock.calls[1][0].actions.volume).toBeNull();
  });

  it("scene_editor_shows_a_refused_hotkey", async () => {
    const draft = { ...newScene(), hotkey: "Alt+Super+1" };
    const { getByText, getByLabelText, queryByText } = render(SceneEditor, {
      draft,
      hotkeyError: 'already used by scene "Desk"',
      onSave: () => {},
      onDelete: () => {},
    });
    expect(getByLabelText("Hotkey")).toHaveClass("refused");
    expect(getByText('already used by scene "Desk"')).toHaveClass("live");
    // A newly recorded hotkey is not the refused one.
    await fireEvent.click(getByText("Record"));
    await fireEvent.keyDown(window, { code: "Digit2", key: "2", altKey: true, metaKey: true });
    expect(getByLabelText("Hotkey")).not.toHaveClass("refused");
    expect(queryByText('already used by scene "Desk"')).toBeNull();
  });
});
