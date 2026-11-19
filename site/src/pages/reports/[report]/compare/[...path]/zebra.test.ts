import { describe, expect, it } from 'vitest'
import { DENSITIES, LINE, lineOf, phaseOf } from './zebra'
import type { Vec3 } from './vec'

const TOWARD: Vec3 = [0, 0, 1]
const UP: Vec3 = [0, 1, 0]
const RIGHT: Vec3 = [1, 0, 0]
const N = DENSITIES.normal
const unit = (v: Vec3): Vec3 => {
  const l = Math.hypot(...v)
  return [v[0] / l, v[1] / l, v[2] / l]
}

describe('phaseOf', () => {
  it('is zero for a face turned straight at the viewer', () => {
    expect(phaseOf(TOWARD, TOWARD, UP, RIGHT, N)).toBeCloseTo(0, 12)
  })

  it('depends only on the normal, so faces that meet tangent carry one line across', () => {
    const n = unit([0.1, 0.3, 1])
    expect(phaseOf(n, TOWARD, UP, RIGHT, N)).toBe(phaseOf([...n], TOWARD, UP, RIGHT, N))
  })

  it('reads a face from either side alike', () => {
    const n = unit([0.2, -0.4, 1])
    const back: Vec3 = [-n[0], -n[1], -n[2]]
    expect(phaseOf(back, TOWARD, UP, RIGHT, N)).toBeCloseTo(phaseOf(n, TOWARD, UP, RIGHT, N), 12)
  })

  it('runs through half the lines as the normal tips an eighth of a turn across them', () => {
    // A reflection turns twice as fast as its normal: tipping the normal an eighth of a turn up
    // the screen's diagonal sweeps the reflection from straight back to along it.
    for (const stripes of Object.values(DENSITIES)) {
      const n = unit([1, 1, Math.SQRT2])
      expect(phaseOf(n, TOWARD, UP, RIGHT, stripes)).toBeCloseTo(stripes / 2, 6)
    }
  })

  it('lines a fillet round a vertical edge and one round a level edge alike', () => {
    // Seen from a little above, as the page opens. Lined by height alone, a surface curved only
    // about the screen's up would reflect one height everywhere and show no line.
    const toward = unit([0, 0.4, 1])
    const up = unit([0, 1, -0.4])
    const spread = (normals: Vec3[]) => {
      const p = normals.map(n => phaseOf(n, toward, up, RIGHT, N))
      return Math.max(...p) - Math.min(...p)
    }
    const quarter = (f: (a: number) => Vec3) =>
      Array.from({ length: 19 }, (_, i) => f((i / 18) * (Math.PI / 2)))
    const t = (a: number) => a - Math.PI / 4
    expect(spread(quarter(a => [Math.sin(t(a)), 0, Math.cos(t(a))]))).toBeGreaterThan(2)
    expect(spread(quarter(a => [0, Math.sin(t(a)), Math.cos(t(a))]))).toBeGreaterThan(2)
  })

  it('shows a slight crease: normals a degree apart differ by a visible part of a line', () => {
    const a = (d: number): Vec3 => {
      const r = (d * Math.PI) / 180
      return [Math.sin(r) * Math.SQRT1_2, Math.sin(r) * Math.SQRT1_2, Math.cos(r)]
    }
    const shift = phaseOf(a(21), TOWARD, UP, RIGHT, N) - phaseOf(a(20), TOWARD, UP, RIGHT, N)
    // More than a line's own width: the line visibly steps at the crease.
    expect(shift).toBeGreaterThan(LINE)
  })
})

describe('lineOf', () => {
  it('is brightest on each whole phase and fades away from it', () => {
    for (const p of [-2, 0, 1, 7]) expect(lineOf(p)).toBeCloseTo(1, 12)
    expect(lineOf(0.5 * LINE)).toBeGreaterThan(0.5)
    expect(lineOf(2 * LINE)).toBeLessThan(0.05)
  })

  it('leaves most of each period unlit, so a flat face keeps its shading', () => {
    const lit = Array.from({ length: 1000 }, (_, i) => lineOf(i / 1000)).filter(x => x > 0.1)
    expect(lit.length / 1000).toBeLessThan(0.35)
  })
})
