import { describe, expect, it } from 'vitest'
import { overlayOf } from './glb'
import { keysOf } from './views'

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
      { mark: 'inside', label: 'inside' },
      { mark: 'quiet', label: 'outside' },
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
      { mark: 'dot', label: 'point and its normal' },
      { mark: 'quiet', label: 'no normal: on a crease' },
    ])
  })
})
