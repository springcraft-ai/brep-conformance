import { describe, expect, it } from 'vitest'
import { raster, rasterize, studio, type Mesh, type Shade } from './solid'
import type { Vec3 } from './vec'

// A unit pixel grid looking down -z: x and y are pixels, z is depth toward the viewer.
const at = (x: number, y: number, z: number): [number, number, number] => [x, y, z]
const FRONT: [Vec3, Vec3, Vec3] = [
  [1, 0, 0],
  [0, 1, 0],
  [0, 0, 1],
]
// Writes the view-space normal as bytes, so a test reads back what the rasterizer shaded with.
const normal: Shade = (x, y, z) => [(x + 1) * 100, (y + 1) * 100, (z + 1) * 100]
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
      rasterize(r, mesh([...square(0, 8, zs[0]!), ...square(2, 6, zs[1]!)]), at, FRONT, normal)
      expect(r.depth[4 * 8 + 4]).toBe(5)
      expect(r.depth[0]).toBe(zs[0])
    }
  })

  it('fills a face with no cracks along its diagonal and leaves the rest clear', () => {
    const r = raster(8, 8)
    rasterize(r, mesh(square(2, 6, 0)), at, FRONT, normal)
    for (let y = 0; y < 8; y++) {
      for (let x = 0; x < 8; x++) {
        const inside = x >= 2 && x < 6 && y >= 2 && y < 6
        expect(r.rgba[(y * 8 + x) * 4 + 3]).toBe(inside ? 255 : 0)
      }
    }
  })

  it('shades a face by its own normal when the mesh gives none, from the front either way', () => {
    const r = raster(4, 4)
    rasterize(r, mesh(square(0, 4, 0)), at, FRONT, normal)
    expect(Array.from(r.rgba.slice(0, 3))).toEqual([100, 100, 200])
    const away: Mesh = { ...mesh(square(0, 4, 0)), triangles: Uint32Array.of(0, 2, 1, 0, 3, 2) }
    rasterize(r, away, at, FRONT, normal)
    expect(Array.from(r.rgba.slice(0, 3))).toEqual([100, 100, 200])
  })

  it('shades by the vertex normals, turned into the view', () => {
    const r = raster(4, 4)
    const tilted = [1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1]
    rasterize(r, mesh(square(0, 4, 0), tilted), at, FRONT, normal)
    const s = Math.round((Math.SQRT1_2 + 1) * 100)
    expect(Array.from(r.rgba.slice(0, 3))).toEqual([s, 100, s])
    // Turned a quarter about y, the world's x is toward the viewer: the same normal faces them.
    const turned: [Vec3, Vec3, Vec3] = [
      [0, 0, -1],
      [0, 1, 0],
      [1, 0, 0],
    ]
    const sideways = [1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0]
    rasterize(r, mesh(square(0, 4, 0), sideways), at, turned, normal)
    expect(Array.from(r.rgba.slice(0, 3))).toEqual([100, 100, 200])
  })
})

describe('studio', () => {
  const metal = studio([184, 194, 204])
  const brightness = (c: number[]) => c[0]! + c[1]! + c[2]!

  it('lights a face toward the upper-left key more than one toward the lower right', () => {
    const k = Math.SQRT1_2
    expect(brightness(metal(-k, k, 0.2))).toBeGreaterThan(brightness(metal(k, -k, 0.2)))
  })

  it('keeps every channel a byte', () => {
    const normals: Vec3[] = [
      [0, 0, 1],
      [0, 1, 0],
      [0, -1, 0],
      [-0.45, 0.75, 0.55],
    ]
    for (const n of normals)
      for (const c of metal(...n)) {
        expect(c).toBeGreaterThanOrEqual(0)
        expect(c).toBeLessThanOrEqual(255)
      }
  })
})
