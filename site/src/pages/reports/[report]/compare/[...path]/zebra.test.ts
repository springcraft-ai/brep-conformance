import { describe, expect, it } from 'vitest'
import { STRIPES, phaseOf, stripesFor } from './zebra'
import type { Vec3 } from './vec'

const TOWARD: Vec3 = [0, 0, 1]
const UP: Vec3 = [0, 1, 0]
const unit = (v: Vec3): Vec3 => {
  const l = Math.hypot(...v)
  return [v[0] / l, v[1] / l, v[2] / l]
}

describe('phaseOf', () => {
  it('is zero for a face turned straight at the viewer', () => {
    expect(phaseOf(TOWARD, TOWARD, UP)).toBeCloseTo(0, 12)
  })

  it('depends only on the normal, so faces that meet tangent carry one stripe across', () => {
    const n = unit([0.1, 0.3, 1])
    expect(phaseOf(n, TOWARD, UP)).toBe(phaseOf([...n], TOWARD, UP))
  })

  it('reads a face from either side alike', () => {
    const n = unit([0.2, -0.4, 1])
    expect(phaseOf([-n[0], -n[1], -n[2]], TOWARD, UP)).toBeCloseTo(phaseOf(n, TOWARD, UP), 12)
  })

  it('runs through every stripe as the normal tips an eighth of a turn down and up', () => {
    // A reflection turns twice as fast as its normal: tipping the normal an eighth of a turn up
    // from straight on sweeps the reflection from level to straight up, half the stripes.
    expect(phaseOf(unit([0, 1, 1]), TOWARD, UP)).toBeCloseTo(STRIPES / 2, 6)
    expect(phaseOf(unit([0, -1, 1]), TOWARD, UP)).toBeCloseTo(-STRIPES / 2, 6)
  })

  it('shows a slight crease: two normals a degree apart differ by a visible part of a stripe', () => {
    const a = (d: number): Vec3 => [0, Math.sin((d * Math.PI) / 180), Math.cos((d * Math.PI) / 180)]
    const shift = phaseOf(a(21), TOWARD, UP) - phaseOf(a(20), TOWARD, UP)
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
