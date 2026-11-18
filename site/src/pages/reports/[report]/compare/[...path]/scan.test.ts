import { describe, expect, it } from 'vitest'
import { camera, frameOf, HOME, overlayGeometry, panBy, project, tumble, zoomAt } from './scan'

describe('overlayGeometry', () => {
  it('draws a box as its twelve edges, each from a corner to the next', () => {
    const g = overlayGeometry(
      {
        points: [],
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
        points: [[1, 2, 3]],
        box: null,
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
      { points: [], box: null, plane: { origin: [0, 0, 50], normal: [0, 0, 1] } },
      { lo: [0, 0, 0], hi: [1, 1, 1] },
    )
    expect(g.plane.length + g.planeEdges.length).toBe(0)
  })

  it('draws nothing where the case lays nothing over its views', () => {
    const g = overlayGeometry(undefined, { lo: [0, 0, 0], hi: [1, 1, 1] })
    expect(g.plane.length + g.planeEdges.length + g.box.length + g.points.length).toBe(0)
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
