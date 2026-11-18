import { describe, expect, it } from 'vitest'
import { raster, rasterize, visibleRuns, type Mesh } from './solid'

// A unit pixel grid looking down -z: x and y are pixels, z is depth toward the viewer.
const at = (x: number, y: number, z: number): [number, number, number] => [x, y, z]
const square = (lo: number, hi: number, z: number) => [lo, lo, z, hi, lo, z, hi, hi, z, lo, hi, z]
const mesh = (positions: number[], normals: number[] = []): Mesh => ({
  positions: new Float32Array(positions),
  normals: normals.length ? new Float32Array(normals) : new Float32Array(positions.length),
  triangles: Uint32Array.from(
    Array.from({ length: positions.length / 12 }, (_, q) =>
      [0, 1, 2, 0, 2, 3].map(i => q * 4 + i),
    ).flat(),
  ),
})

describe('rasterize', () => {
  it('keeps the nearer of two surfaces, whichever is drawn first', () => {
    for (const zs of [
      [1, 5],
      [5, 1],
    ]) {
      const r = raster(8, 8)
      rasterize(
        r,
        mesh([...square(0, 8, zs[0]!), ...square(2, 6, zs[1]!)]),
        at,
        [0, 0, 1],
        [200, 100, 50],
      )
      expect(r.depth[4 * 8 + 4]).toBe(5)
      expect(r.depth[0]).toBe(zs[0])
    }
  })

  it('fills a face with no cracks along its diagonal and leaves the rest clear', () => {
    const r = raster(8, 8)
    rasterize(r, mesh(square(2, 6, 0)), at, [0, 0, 1], [200, 100, 50])
    for (let y = 0; y < 8; y++) {
      for (let x = 0; x < 8; x++) {
        const inside = x >= 2 && x < 6 && y >= 2 && y < 6
        expect(r.rgba[(y * 8 + x) * 4 + 3]).toBe(inside ? 255 : 0)
      }
    }
  })

  it('lights a face by its own normal when the mesh gives none, and both sides alike', () => {
    const facing = raster(4, 4)
    rasterize(facing, mesh(square(0, 4, 0)), at, [0, 0, 1], [200, 100, 50])
    expect(Array.from(facing.rgba.slice(0, 3))).toEqual([200, 100, 50])
    const away = raster(4, 4)
    rasterize(away, mesh(square(0, 4, 0)), at, [0, 0, -1], [200, 100, 50])
    expect(Array.from(away.rgba.slice(0, 3))).toEqual([200, 100, 50])
  })

  it('lights by the vertex normals when the mesh has them', () => {
    const r = raster(4, 4)
    const sideways = [1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0]
    rasterize(r, mesh(square(0, 4, 0), sideways), at, [0, 0, 1], [200, 100, 50])
    expect(r.rgba[0]).toBe(60)
  })
})

describe('visibleRuns', () => {
  const r = raster(20, 20)
  rasterize(r, mesh(square(5, 15, 0)), at, [0, 0, 1], [255, 255, 255])

  it('draws an edge that lies in the surface, and one in front of it', () => {
    expect(visibleRuns(r, [6, 10, 0], [14, 10, 0], 0.01)).toEqual([0, 1])
    expect(visibleRuns(r, [6, 10, 3], [14, 10, 3], 0.01)).toEqual([0, 1])
  })

  it('hides an edge behind the surface, and shows the parts that pass beside it', () => {
    expect(visibleRuns(r, [8, 10, -1], [12, 10, -1], 0.01)).toEqual([])
    const runs = visibleRuns(r, [0, 10, -1], [20, 10, -1], 0.01)
    expect(runs.length).toBe(4)
    expect(runs[0]).toBe(0)
    expect(runs[1]! * 20).toBeGreaterThan(4)
    expect(runs[1]! * 20).toBeLessThan(8)
    expect(runs[2]! * 20).toBeGreaterThan(12)
    expect(runs[2]! * 20).toBeLessThan(16)
    expect(runs[3]).toBe(1)
  })
})
