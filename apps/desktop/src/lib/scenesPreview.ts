// The placeholder scene cards shown on Noise and Scenes until the modes module lands.

export interface SceneAction {
  label: string;
  /** Drawn orange, like an active ULT box. */
  ult?: boolean;
}

export interface SceneCard {
  name: string;
  when: string;
  then: SceneAction[];
}

export const SCENES_PREVIEW: SceneCard[] = [
  {
    name: "At the desk",
    when: "When this Mac is playing",
    then: [{ label: "Ambient 14" }, { label: "Voice focus" }, { label: "ULT 1", ult: true }],
  },
  { name: "On a call", when: "When Zoom or FaceTime has the mic", then: [{ label: "NC" }, { label: "ULT off" }] },
];
