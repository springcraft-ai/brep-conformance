// Zebra stripes: each point of a face lit by the bars of a striped sky, as its normal reflects the
// view. A stripe crosses from face to face unbroken only where the faces' normals agree, so a
// crease shows as a jump and a change of curvature as a kink.
import type { Vec3 } from './vec'

// Stripes across the half turn from the sky straight below to straight above.
export const STRIPES = 12

const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2]

// Where a normal falls among the stripes: whole numbers bound them, light where the fraction is
// below a quarter or above three quarters. ZEBRA_GLSL is the same in the face shader.
export function phaseOf(n: Vec3, toward: Vec3, up: Vec3): number {
  const s = dot(n, toward) < 0 ? -1 : 1
  const m: Vec3 = [n[0] * s, n[1] * s, n[2] * s]
  const k = 2 * dot(toward, m)
  const r = [0, 1, 2].map(i => k * m[i]! - toward[i]!) as Vec3
  return (Math.asin(Math.min(1, Math.max(-1, dot(r, up)))) * STRIPES) / Math.PI
}

export const ZEBRA_GLSL = `
const float STRIPES = ${STRIPES.toFixed(1)};
float zebra(vec3 n, vec3 toward, vec3 up) {
  if (dot(n, toward) < 0.0) n = -n;
  vec3 r = 2.0 * dot(toward, n) * n - toward;
  float phase = asin(clamp(dot(r, up), -1.0, 1.0)) * STRIPES / 3.14159265;
  float w = fwidth(phase);
  return smoothstep(-w, w, abs(fract(phase) - 0.5) - 0.25);
}`

// Tests whose subject is how faces meet or how a free-form surface runs: blends, lofts, sweeps,
// slabs, offsets, parts and imported parts. Elsewhere, planes and plain primitives, they add
// nothing, and an input the contract refuses has no result to stripe.
const STRIPED = [
  /^fillets\//,
  /^chamfers\/next-to-fillets/,
  /^constructions\/(height-field-slab|helical-sweep|ruled-loft|smooth-loft|spline-prism|sweep)/,
  /^modify\/(offset|shell-after-fillet|shell-rounded-box)/,
  /^parts\//,
  /^step\/read-/,
]

export function stripesFor(test: string): boolean {
  return !test.includes('/rejects-') && STRIPED.some(r => r.test(test))
}
