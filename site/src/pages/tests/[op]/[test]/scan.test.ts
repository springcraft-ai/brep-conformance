import { describe, expect, it } from 'vitest'
import { camera, frameOf, HOME, project } from './scan'

const empty = new Float32Array(0)

describe('frameOf', () => {
  it('fits the shape and the marks, and gives a flat frame some depth', () => {
    const f = frameOf({
      positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
      normals: empty,
      triangles: new Uint32Array([0, 1, 2]),
      edges: new Float32Array([0, 0, 0, 4, 2, 0]),
      smooth: new Float32Array([0, 0, 0, 0, 5, 0, 0, 0, 1, 0, 0, 1]),
      marks: [{ p: [-3, 1, 0], held: true }],
    })
    expect(f.lo).toEqual([-3, 0, -1])
    expect(f.hi).toEqual([4, 5, 1])
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
    const c = camera({ yaw: 0, pitch: 0 }, frame, 100, 100)
    expect(project(c, 0, 0, 1)[1]).toBeLessThan(50)
    expect(project(c, 1, 0, 0)[2]).toBeGreaterThan(project(c, -1, 0, 0)[2])
  })
})
