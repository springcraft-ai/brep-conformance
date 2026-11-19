export type Axis = 0 | 1 | 2

// The tests whose results hold walls, voids or parts that their outside hides, each with the axis
// its section is cut across: y through a turned part's axis or down a box's walls, z across an L
// drawn in x and y and pushed along z.
export const SECTIONS: Record<string, Axis> = {
  'booleans/overlap/solid-inside-solid': 1,
  'booleans/overlap/solid-with-a-void': 1,
  'booleans/touching/sphere-in-a-cone': 1,
  'booleans/touching/sphere-in-a-cylinder': 1,
  'modify/shell': 1,
  'modify/shell-after-fillet': 1,
  'modify/shell-ball-or-cone': 1,
  'modify/shell-frustum': 1,
  'modify/shell-l-prism-open-at-its-step': 2,
  'modify/shell-rounded-box': 1,
  'modify/shell-rounded-puck': 1,
  'modify/thicken-faces': 1,
  'parts/housings/enclosure': 1,
}

export function sectionOf(test: string): Axis | null {
  return SECTIONS[test] ?? null
}
