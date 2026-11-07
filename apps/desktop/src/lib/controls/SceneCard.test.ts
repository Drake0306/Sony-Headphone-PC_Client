import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import SceneCard from "./SceneCard.svelte";
import type { Scene } from "../scenes";

const desk: Scene = {
  id: "desk",
  name: "At the desk",
  enabled: true,
  trigger: { kind: "time_range", from: "22:00", to: "07:00" },
  actions: { nc: "Ambient", ambient_level: 14, voice_focus: true, ult: 1, eq_preset: null, volume: null, dsee_auto_off: null },
  hotkey: null,
};

describe("SceneCard", () => {
  it("scene_card_shows_sentence_chips_and_active", () => {
    const { container, getByText } = render(SceneCard, { scene: desk, active: true });
    expect(getByText("At the desk")).toBeInTheDocument();
    expect(getByText("Between 22:00 and 07:00")).toBeInTheDocument();
    const chips = [...container.querySelectorAll(".then span")].map((c) => c.textContent);
    expect(chips).toEqual(["Ambient 14", "Voice focus", "ULT 1"]);
    expect(getByText("ULT 1")).toHaveClass("u");
    expect(getByText("Ambient 14")).not.toHaveClass("u");
    expect(getByText("Active")).toBeInTheDocument();
    expect(container.querySelector(".scene")).toHaveClass("on");
  });

  it("scene_card_labels_disabled_and_idle_scenes", () => {
    const off = render(SceneCard, { scene: { ...desk, enabled: false }, active: false });
    expect(off.getByText("Off")).toBeInTheDocument();
    expect(off.container.querySelector(".scene")).not.toHaveClass("on");
    off.unmount();
    const idle = render(SceneCard, { scene: desk, active: false });
    expect(idle.getByText("On")).toBeInTheDocument();
  });

  it("scene_card_click_selects_when_selectable", async () => {
    const onselect = vi.fn();
    const { getByRole } = render(SceneCard, { scene: desk, active: false, onselect });
    await fireEvent.click(getByRole("button", { name: /At the desk/ }));
    expect(onselect).toHaveBeenCalled();
  });
});
