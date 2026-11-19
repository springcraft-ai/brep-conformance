// Zebra stripes, drawn as reflection lines: the face keeps its shading, and thin bright lines are
// laid over it where its normal reflects the view onto the bars of a striped sky. A line crosses
// from face to face unbroken only where the faces' normals agree, so a crease shows as a step and
// a change of curvature as a kink.
import type { Vec3 } from './vec'

// Lines across the half turn from the sky straight below to straight above.
export const DENSITIES = { few: 6, normal: 12, many: 24 } as const
export type Density = keyof typeof DENSITIES

// A line's half-width, as a share of the space from one line to the next.
export const LINE = 0.09

const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2]

// Where a normal falls among the lines, a line on each whole number. ZEBRA_GLSL is the same in the
// face shader. The lines run across the screen's diagonal: a surface curved only about the lines'
// direction reflects one phase everywhere, and a part's edges run up and across the screen far
// more than diagonally.
export function phaseOf(n: Vec3, toward: Vec3, up: Vec3, right: Vec3, stripes: number): number {
  const s = dot(n, toward) < 0 ? -1 : 1
  const m: Vec3 = [n[0] * s, n[1] * s, n[2] * s]
  const k = 2 * dot(toward, m)
  const r = [0, 1, 2].map(i => k * m[i]! - toward[i]!) as Vec3
  const across = (dot(r, up) + dot(r, right)) * Math.SQRT1_2
  return (Math.asin(Math.min(1, Math.max(-1, across))) * stripes) / Math.PI
}

// How bright the line is at a phase: 1 on a whole number, fading softly either side.
export function lineOf(phase: number): number {
  const d = phase - Math.round(phase)
  return Math.exp(-(d * d) / (LINE * LINE))
}

// The same, widened where a pixel spans more phase than a line is wide, so a line never breaks up
// into dots where it bunches.
export const ZEBRA_GLSL = `
float zebra(vec3 n, vec3 toward, vec3 up, vec3 right, float stripes) {
  if (dot(n, toward) < 0.0) n = -n;
  vec3 r = 2.0 * dot(toward, n) * n - toward;
  float across = (dot(r, up) + dot(r, right)) * 0.70710678;
  float phase = asin(clamp(across, -1.0, 1.0)) * stripes / 3.14159265;
  float w = max(${LINE.toFixed(3)}, 0.6 * fwidth(phase));
  float d = phase - floor(phase + 0.5);
  return exp(-(d * d) / (w * w)) * ${LINE.toFixed(3)} / w;
}`
