import { describe, expect, it } from 'vitest'
import { overlayOf } from './glb'
import type { Vec3 } from './vec'
import {
  camera,
  frameOf,
  HOME,
  homeOf,
  overlayAt,
  overlayGeometry,
  panBy,
  project,
  tumble,
  zoomAt,
} from './scan'

describe('overlayGeometry', () => {
  it('draws a box as its twelve edges, each from a corner to the next', () => {
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        box: [
          [0, 0, 0],
          [1, 2, 3],
        ],
        plane: null,
      },
      { lo: [0, 0, 0], hi: [1, 1, 1] },
    )
    expect(g.box.length / 6).toBe(12)
    for (let i = 0; i < g.box.length; i += 6) {
      const changed = [0, 1, 2].filter(k => g.box[i + k] !== g.box[i + 3 + k])
      expect(changed.length).toBe(1)
    }
  })

  it('draws a plane as the polygon it cuts from the solid’s box, grown a little, and its outline', () => {
    const within = { lo: [-2, -2, 0], hi: [2, 2, 10] } as const
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        points: [[1, 2, 3]],
        plane: { origin: [0, 0, 5], normal: [0, 0, 2] },
      },
      { lo: [...within.lo], hi: [...within.hi] },
    )
    // A level plane through a box cuts a rectangle: four sides, drawn as four triangles.
    expect(g.planeEdges.length / 6).toBe(4)
    expect(g.plane.length / 9).toBe(4)
    for (let i = 2; i < g.plane.length; i += 3) expect(g.plane[i]).toBeCloseTo(5)
    // Past the box by 8% of its 4 mm width each way.
    expect(Math.max(...g.planeEdges.filter((_, i) => i % 3 === 0))).toBeCloseTo(2.32)
    expect(Array.from(g.points)).toEqual([1, 2, 3])
  })

  it('draws no plane that misses the solid’s box', () => {
    const g = overlayGeometry(
      { ...overlayOf({}), plane: { origin: [0, 0, 50], normal: [0, 0, 1] } },
      { lo: [0, 0, 0], hi: [1, 1, 1] },
    )
    expect(g.plane.length + g.planeEdges.length).toBe(0)
  })

  it('draws nothing where the case lays nothing over its views', () => {
    const g = overlayGeometry(undefined, { lo: [0, 0, 0], hi: [1, 1, 1] })
    const parts = [g.plane, g.planeEdges, g.box, g.points, g.lines, g.dashes, g.inside, g.quiet]
    expect(parts.reduce((n, p) => n + p.length, 0)).toBe(0)
  })
})

// Pairs of points, as LINES draws them.
const segments = (a: Float32Array) =>
  Array.from({ length: a.length / 6 }, (_, i) => [
    [a[6 * i]!, a[6 * i + 1]!, a[6 * i + 2]!],
    [a[6 * i + 3]!, a[6 * i + 4]!, a[6 * i + 5]!],
  ])

describe('overlayGeometry, a query’s answers', () => {
  const within: { lo: Vec3; hi: Vec3 } = { lo: [0, 0, 0], hi: [10, 0, 0] }

  it('draws a drawing’s seen stretches as lines and its hidden ones as dashes along them', () => {
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        lines: [
          {
            seen: true,
            pts: [
              [0, 0, 0],
              [0, 4, 0],
              [0, 4, 4],
            ],
          },
          {
            seen: false,
            pts: [
              [0, 0, 0],
              [10, 0, 0],
            ],
          },
        ],
      },
      within,
    )
    expect(segments(g.lines)).toEqual([
      [
        [0, 0, 0],
        [0, 4, 0],
      ],
      [
        [0, 4, 0],
        [0, 4, 4],
      ],
    ])
    const dashes = segments(g.dashes)
    expect(dashes.length).toBeGreaterThan(5)
    // Every dash lies on the hidden stretch, and they leave gaps between them.
    for (const [p, q] of dashes) for (const v of [p!, q!]) expect([v[1], v[2]]).toEqual([0, 0])
    const drawn = dashes.reduce((s, [p, q]) => s + q![0]! - p![0]!, 0)
    expect(drawn).toBeGreaterThan(3)
    expect(drawn).toBeLessThan(7)
  })

  it('gives its guide lines again as dashes, for where the solid hides them', () => {
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        closest: [{ at: [0, 0, 0], foot: [10, 0, 0], distance: 10 }],
      },
      within,
    )
    const dashes = segments(g.hiddenLines)
    expect(dashes.length).toBeGreaterThan(5)
    for (const [p, q] of dashes) for (const v of [p!, q!]) expect([v[1], v[2]]).toEqual([0, 0])
    const drawn = dashes.reduce((s, [p, q]) => s + q![0]! - p![0]!, 0)
    expect(drawn).toBeGreaterThan(3)
    expect(drawn).toBeLessThan(7)
  })

  it('draws a closest-point query as its point, the nearest point, and the segment between', () => {
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        closest: [
          { at: [1, 2, 3], foot: [1, 2, 0], distance: 3 },
          { at: [5, 5, 5], foot: null, distance: 2 },
        ],
      },
      within,
    )
    expect(Array.from(g.points)).toEqual([1, 2, 3, 5, 5, 5])
    expect(Array.from(g.inside)).toEqual([1, 2, 0])
    expect(segments(g.lines)).toEqual([
      [
        [1, 2, 3],
        [1, 2, 0],
      ],
    ])
  })

  it('draws each classified point in its side’s colour', () => {
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        sides: [
          { at: [1, 0, 0], side: 'in' },
          { at: [2, 0, 0], side: 'on' },
          { at: [3, 0, 0], side: 'out' },
        ],
      },
      within,
    )
    expect(Array.from(g.inside)).toEqual([1, 0, 0])
    expect(Array.from(g.points)).toEqual([2, 0, 0])
    expect(Array.from(g.quiet)).toEqual([3, 0, 0])
  })

  it('spreads a probe of three points across the surface, joined by a stick, so they show apart', () => {
    const g = overlayOf({
      sides: [
        { at: [5, 0, 0], side: 'on' },
        { at: [5, 0, -0.01], side: 'in' },
        { at: [5, 0, 0.01], side: 'out' },
        { at: [8, 0, 0], side: 'on' },
      ],
    })
    const o = overlayGeometry(g, within)
    // On the surface where it is; inside and outside spread along the line through the three.
    expect(Array.from(o.points)).toEqual([5, 0, 0, 8, 0, 0])
    const [ix, iy, iz] = Array.from(o.inside)
    const [ox, oy, oz] = Array.from(o.quiet)
    expect([ix, iy, ox, oy]).toEqual([5, 0, 5, 0])
    expect(iz!).toBeLessThan(-0.2)
    expect(oz!).toBeCloseTo(-iz!)
    // The stick is part of the mark: faint where hidden like an arrow, never dashed.
    expect(o.lines.length + o.hiddenLines.length).toBe(0)
    expect(segments(o.sticks)).toEqual([
      [
        [5, 0, iz!],
        [5, 0, oz!],
      ],
    ])
  })

  it('draws a normal as an arrow from its point, and a point with none apart', () => {
    const g = overlayGeometry(
      {
        ...overlayOf({}),
        normals: [
          { at: [1, 1, 1], n: [0, 0, 1] },
          { at: [4, 4, 4], n: null },
        ],
      },
      within,
    )
    // A normal is drawn by its arrow alone, which starts at its point.
    expect(g.points.length).toBe(0)
    expect(Array.from(g.quiet)).toEqual([4, 4, 4])
    expect(g.lines.length).toBe(0)
    // One segment from the point along its normal; the renderer draws the arrow round it.
    const [shaft, ...rest] = segments(g.arrows)
    expect(rest.length).toBe(0)
    expect(shaft![0]).toEqual([1, 1, 1])
    expect(shaft![1]![0]).toBeCloseTo(1)
    expect(shaft![1]![1]).toBeCloseTo(1)
    expect(shaft![1]![2]).toBeGreaterThan(1.2)
  })
})

describe('overlayAt', () => {
  it('gives every place the overlay marks, so the view can take them all in', () => {
    const at = overlayAt({
      ...overlayOf({}),
      points: [[1, 0, 0]],
      box: [
        [0, 0, 0],
        [2, 2, 2],
      ],
      plane: { origin: [5, 5, 5], normal: [0, 0, 1] },
      lines: [
        {
          seen: false,
          pts: [
            [3, 0, 0],
            [4, 0, 0],
          ],
        },
      ],
      closest: [{ at: [6, 0, 0], foot: [7, 0, 0], distance: 1 }],
      sides: [{ at: [8, 0, 0], side: 'out' }],
      normals: [{ at: [9, 0, 0], n: null }],
    })
    const xs = at.map(p => p[0]).sort((a, b) => a - b)
    expect(xs).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
  })
})

describe('homeOf', () => {
  it('opens a drawing looking along its direction', () => {
    const look: [number, number, number] = [1, -2, 2]
    const v = homeOf({ ...overlayOf({}), look })
    const c = camera(v, { lo: [0, 0, 0], hi: [1, 1, 1] }, 100, 100)
    for (let k = 0; k < 3; k++) expect(c.toward[k]).toBeCloseTo(look[k]! / 3)
  })
})

const empty = new Float32Array(0)

describe('frameOf', () => {
  it('fits both subjects, the ghosts and the marks, and gives a flat frame some depth', () => {
    const f = frameOf({
      positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
      normals: empty,
      triangles: new Uint32Array([0, 1, 2]),
      edges: new Float32Array([0, 0, 0, 4, 2, 0]),
      alt: {
        positions: new Float32Array([0, 0, 6]),
        normals: empty,
        triangles: new Uint32Array(0),
        edges: empty,
      },
      ghosts: [
        {
          positions: new Float32Array([0, 5, 0]),
          normals: empty,
          triangles: new Uint32Array(0),
          edges: empty,
          tone: 0,
        },
      ],
      marks: [{ p: [-3, 1, 0], held: true }],
    })
    expect(f.lo).toEqual([-3, 0, 0])
    expect(f.hi).toEqual([4, 5, 6])
  })
})

describe('camera and project', () => {
  const frame = {
    lo: [-1, -1, -1] as [number, number, number],
    hi: [1, 1, 1] as [number, number, number],
  }

  it('puts the frame’s centre at the middle of the view and fits the frame inside it', () => {
    const c = camera(HOME, frame, 400, 200)
    const [x, y] = project(c, 0, 0, 0)
    expect(x).toBeCloseTo(200)
    expect(y).toBeCloseTo(100)
    const corners: [number, number, number][] = [
      [-1, -1, -1],
      [1, 1, 1],
      [1, -1, 1],
    ]
    for (const p of corners) {
      const [px, py] = project(c, ...p)
      expect(Math.abs(px - 200)).toBeLessThan(100)
      expect(Math.abs(py - 100)).toBeLessThan(100)
    }
  })

  it('draws z up and nearer points with greater depth', () => {
    const c = camera({ ...HOME, yaw: 0, pitch: 0 }, frame, 100, 100)
    expect(project(c, 0, 0, 1)[1]).toBeLessThan(50)
    expect(project(c, 1, 0, 0)[2]).toBeGreaterThan(project(c, -1, 0, 0)[2])
  })

  it('zooms about a screen point, which stays on the same point of the model', () => {
    const c = camera(HOME, frame, 400, 200)
    const p: [number, number, number] = [0.5, -0.3, 0.2]
    const [x, y] = project(c, ...p)
    const z = camera(zoomAt(HOME, c, x, y, 3), frame, 400, 200)
    expect(z.scale).toBeCloseTo(c.scale * 3)
    expect(project(z, ...p)[0]).toBeCloseTo(x)
    expect(project(z, ...p)[1]).toBeCloseTo(y)
  })

  it('pans so the model follows the pointer, at any zoom, keeping depth', () => {
    const v = { ...HOME, zoom: 2.5 }
    const c = camera(v, frame, 400, 200)
    const before = project(c, 0, 0, 0)
    const after = project(camera(panBy(v, c, 30, -12), frame, 400, 200), 0, 0, 0)
    expect(after[0] - before[0]).toBeCloseTo(30)
    expect(after[1] - before[1]).toBeCloseTo(-12)
    expect(after[2]).toBeCloseTo(before[2])
  })

  it('tumbles over the top, and turns across the same way upside down', () => {
    const over = tumble(HOME, 0, 200, Math.PI / 200)
    expect(Math.cos(over.pitch)).toBeLessThan(0)
    expect(tumble(over, 10, 0, 0.01).yaw).toBeGreaterThan(over.yaw)
    expect(tumble(HOME, 10, 0, 0.01).yaw).toBeLessThan(HOME.yaw)
  })
})
