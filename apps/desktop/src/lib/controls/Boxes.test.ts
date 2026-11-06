import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import Boxes from "./Boxes.svelte";

const options = [
  { label: "OFF", value: 0 },
  { label: "1", value: 1 },
  { label: "2", value: 2 },
];

describe("Boxes", () => {
  it("boxes_render_refused_state", () => {
    const { getByText } = render(Boxes<number>, {
      options,
      value: 1,
      status: { Refused: { reason: "no reply" } },
      onpick: () => {},
    });
    const active = getByText("1");
    expect(active).toHaveClass("on");
    expect(active).toHaveClass("refused");
    expect(active).toHaveAttribute("title", "no reply");
    expect(getByText("2")).not.toHaveClass("refused");
  });

  it("boxes_refused_outlines_the_attempted_box", () => {
    const { getByText } = render(Boxes<number>, {
      options,
      value: 1,
      attempted: 2,
      status: { Refused: { reason: "ULT is off during calls" } },
      onpick: () => {},
    });
    expect(getByText("1")).toHaveClass("on");
    expect(getByText("1")).not.toHaveClass("refused");
    expect(getByText("2")).toHaveClass("refused");
    expect(getByText("2")).toHaveAttribute("title", "ULT is off during calls");
  });

  it("boxes_pending_is_striped_and_inert", async () => {
    const onpick = vi.fn();
    const { container, getByText } = render(Boxes<number>, {
      options,
      value: 2,
      status: { Pending: { since_ms: 0 } },
      onpick,
    });
    expect(container.querySelector(".boxes")).toHaveClass("pending");
    await fireEvent.click(getByText("OFF"));
    expect(onpick).not.toHaveBeenCalled();
  });

  it("boxes_unavailable_is_disabled", async () => {
    const onpick = vi.fn();
    const { container, getByText } = render(Boxes<number>, {
      options,
      value: null,
      status: "Unavailable",
      onpick,
    });
    expect(container.querySelector(".boxes")).toHaveClass("unavailable");
    const off = getByText("OFF");
    expect(off).toHaveAttribute("aria-disabled", "true");
    expect(off).toHaveAttribute("tabindex", "-1");
    await fireEvent.click(off);
    expect(onpick).not.toHaveBeenCalled();
  });

  it("boxes_disabled_prop_behaves_as_unavailable", async () => {
    const onpick = vi.fn();
    const { container, getByText } = render(Boxes<number>, {
      options,
      value: 0,
      status: "Confirmed",
      disabled: true,
      onpick,
    });
    expect(container.querySelector(".boxes")).toHaveClass("unavailable");
    await fireEvent.click(getByText("2"));
    expect(onpick).not.toHaveBeenCalled();
  });

  it("boxes_confirmed_click_picks_the_value", async () => {
    const onpick = vi.fn();
    const { getByText } = render(Boxes<number>, { options, value: 0, status: "Confirmed", onpick });
    await fireEvent.click(getByText("2"));
    expect(onpick).toHaveBeenCalledWith(2);
  });

  it("boxes_orange_option_renders_the_ult_box", () => {
    const { getByText } = render(Boxes<number>, {
      options: [
        { label: "OFF", value: 0 },
        { label: "ULT 1", value: 1, orange: true },
      ],
      value: 1,
      status: "Confirmed",
      onpick: () => {},
    });
    expect(getByText("ULT 1")).toHaveClass("on", "orange");
  });
});
