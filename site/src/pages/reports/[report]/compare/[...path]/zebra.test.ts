import { describe, expect, it } from 'vitest'
import { STRIPES, phaseOf, stripesFor } from './zebra'
import type { Vec3 } from './vec'

const TOWARD: Vec3 = [0, 0, 1]
const UP: Vec3 = [0, 1, 0]
const RIGHT: Vec3 = [1, 0, 0]
const unit = (v: Vec3): Vec3 => {
  const l = Math.hypot(...v)
  return [v[0] / l, v[1] / l, v[2] / l]
}

describe('phaseOf', () => {
  it('is zero for a face turned straight at the viewer', () => {
    expect(phaseOf(TOWARD, TOWARD, UP, RIGHT)).toBeCloseTo(0, 12)
  })

  it('depends only on the normal, so faces that meet tangent carry one stripe across', () => {
    const n = unit([0.1, 0.3, 1])
    expect(phaseOf(n, TOWARD, UP, RIGHT)).toBe(phaseOf([...n], TOWARD, UP, RIGHT))
  })

  it('reads a face from either side alike', () => {
    const n = unit([0.2, -0.4, 1])
    expect(phaseOf([-n[0], -n[1], -n[2]], TOWARD, UP, RIGHT)).toBeCloseTo(
      phaseOf(n, TOWARD, UP, RIGHT),
      12,
    )
  })

  it('runs through every stripe as the normal tips an eighth of a turn either way across them', () => {
    // A reflection turns twice as fast as its normal: tipping the normal an eighth of a turn up
    // the screen's diagonal sweeps the reflection from straight back to along it, half the stripes.
    expect(phaseOf(unit([1, 1, Math.SQRT2]), TOWARD, UP, RIGHT)).toBeCloseTo(STRIPES / 2, 6)
    expect(phaseOf(unit([-1, -1, Math.SQRT2]), TOWARD, UP, RIGHT)).toBeCloseTo(-STRIPES / 2, 6)
  })

  it('stripes a fillet round a vertical edge and one round a level edge alike', () => {
    // Seen from a little above, as the page opens. Striped by height alone, a surface curved only
    // about the screen's up would reflect one height everywhere and come out a single band.
    const toward = unit([0, 0.4, 1])
    const right: Vec3 = [1, 0, 0]
    const up = unit([0, 1, -0.4])
    const spread = (normals: Vec3[]) => {
      const p = normals.map(n => phaseOf(n, toward, up, right))
      return Math.max(...p) - Math.min(...p)
    }
    const quarter = (f: (a: number) => Vec3) =>
      Array.from({ length: 19 }, (_, i) => f((i / 18) * (Math.PI / 2)))
    // Normals turning a quarter round a vertical axis, and round a level one.
    const vertical = spread(quarter(a => [Math.sin(a - Math.PI / 4), 0, Math.cos(a - Math.PI / 4)]))
    const level = spread(quarter(a => [0, Math.sin(a - Math.PI / 4), Math.cos(a - Math.PI / 4)]))
    expect(vertical).toBeGreaterThan(2)
    expect(level).toBeGreaterThan(2)
  })

  it('shows a slight crease: two normals a degree apart differ by a visible part of a stripe', () => {
    const a = (d: number): Vec3 => {
      const t = (d * Math.PI) / 180
      return [Math.sin(t) * Math.SQRT1_2, Math.sin(t) * Math.SQRT1_2, Math.cos(t)]
    }
    const shift = phaseOf(a(21), TOWARD, UP, RIGHT) - phaseOf(a(20), TOWARD, UP, RIGHT)
    expect(shift).toBeGreaterThan(0.1)
  })
})

describe('stripesFor', () => {
  it('offers stripes where how faces meet is the test', () => {
    for (const t of [
      'fillets/variable-radius',
      'fillets/g2-set-back-corner',
      'chamfers/next-to-fillets',
      'constructions/smooth-loft-through-sections',
      'constructions/ruled-loft',
      'constructions/sweep-along-a-spline',
      'constructions/helical-sweep',
      'constructions/height-field-slab',
      'constructions/spline-prism',
      'modify/offset-sphere',
      'modify/shell-rounded-box',
      'modify/shell-after-fillet',
      'parts/gears',
      'step/read-ap214',
      'step/read-nist',
    ])
      expect(stripesFor(t), t).toBe(true)
  })

  it('leaves them off planes and plain primitives, and off refused inputs', () => {
    for (const t of [
      'booleans/overlap',
      'chamfers/prism-side-edges',
      'constructions/box',
      'constructions/extrude',
      'modify/move-faces',
      'queries/closest-point',
      'fillets/rejects-radius-of-zero-or-less',
      'constructions/rejects-loft-heights-out-of-order',
    ])
      expect(stripesFor(t), t).toBe(false)
  })
})
