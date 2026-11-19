import { describe, expect, it } from 'vitest'
import { overlayOf, type GlbView, type Shape } from './glb'
import { keysOf, toneKeyOf } from './views'

const tri = (n: number): Shape => ({
  positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
  normals: new Float32Array(9),
  triangles: new Uint32Array(n ? [0, 1, 2] : []),
  edges: new Float32Array(0),
})

describe('toneKeyOf', () => {
  const view = (alt: string | undefined, altFaces: number): GlbView => ({
    name: 'loose faces',
    alt,
    solids: [
      { role: 'main', shape: tri(1) },
      { role: 'alt', shape: tri(altFaces) },
    ],
  })

  it('names what the second tone marks, where the view draws it', () => {
    expect(toneKeyOf(view('written inside out', 1))).toBe('written inside out')
  })

  it('keys nothing for a view with no second tone, or a file that names none', () => {
    expect(toneKeyOf(view('written inside out', 0))).toBeNull()
    expect(toneKeyOf(view(undefined, 1))).toBeNull()
  })
})

describe('keysOf', () => {
  it('names each mark a query’s answers are drawn with, once each', () => {
    const o = overlayOf({
      sides: [
        { at: [0, 0, 0], side: 'in' },
        { at: [1, 0, 0], side: 'in' },
        { at: [2, 0, 0], side: 'out' },
      ],
    })
    expect(keysOf(o, true)).toEqual([
      { mark: 'inside', label: 'just inside' },
      { mark: 'quiet', label: 'just outside' },
    ])
  })

  it('keys a drawing’s seen and hidden lines whether or not points are shown', () => {
    const o = overlayOf({
      lines: [
        {
          seen: true,
          pts: [
            [0, 0, 0],
            [1, 0, 0],
          ],
        },
        {
          seen: false,
          pts: [
            [0, 0, 0],
            [0, 1, 0],
          ],
        },
      ],
      closest: [{ at: [0, 0, 0], foot: [0, 0, 1], distance: 1 }],
    })
    expect(keysOf(o, false)).toEqual([
      { mark: 'seen', label: 'seen' },
      { mark: 'hidden', label: 'hidden' },
    ])
    expect(keysOf(o, true).map(k => k.label)).toEqual([
      'seen',
      'hidden',
      'point asked about',
      'nearest point',
    ])
  })

  it('keys a point with no normal apart from one with an arrow', () => {
    const o = overlayOf({
      normals: [
        { at: [0, 0, 0], n: [0, 0, 1] },
        { at: [1, 0, 0], n: null },
      ],
    })
    expect(keysOf(o, true)).toEqual([
      { mark: 'arrow', label: 'normal' },
      { mark: 'quiet', label: 'no normal: on a crease' },
    ])
  })

  it('keys a curvature case’s point, and each way its ticks bend', () => {
    const o = overlayOf({
      points: [[0, 0, 1]],
      curvature: [{ foot: [0, 0, 1], n: [0, 0, 1], d1: [1, 0, 0], k: [1, 0] }],
    })
    expect(keysOf(o, true)).toEqual([
      { mark: 'dot', label: 'point asked about' },
      { mark: 'convex', label: 'bends outward' },
      { mark: 'flat', label: 'flat' },
    ])
  })
})
