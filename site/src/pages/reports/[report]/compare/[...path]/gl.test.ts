import { describe, expect, it } from 'vitest'
import { prepare, sphereMesh, wideOf } from './gl'

describe('prepare', () => {
  const positions = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0])
  const triangles = new Uint32Array([0, 1, 2])
  const edges = new Float32Array(0)

  it('keeps a mesh indexed where every vertex has a normal', () => {
    const normals = new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1])
    expect(prepare({ positions, normals, triangles, edges }).idx).toBe(triangles)
  })

  it('gives each corner its triangle’s normal where the file gives none', () => {
    const p = prepare({ positions, normals: new Float32Array(9), triangles, edges })
    expect(p.idx).toBeNull()
    expect(Array.from(p.nrm)).toEqual([0, 0, 1, 0, 0, 1, 0, 0, 1])
  })
})

describe('wideOf', () => {
  it('gives a segment as a quad of six vertices, each with both ends and its place on it', () => {
    const w = wideOf(new Float32Array([0, 0, 0, 1, 2, 3]))
    expect(w.length).toBe(6 * 8)
    const places: number[][] = []
    for (let i = 0; i < w.length; i += 8) {
      expect(Array.from(w.slice(i, i + 6))).toEqual([0, 0, 0, 1, 2, 3])
      places.push([w[i + 6]!, w[i + 7]!])
    }
    expect(places).toEqual([
      [0, -1],
      [1, -1],
      [1, 1],
      [0, -1],
      [1, 1],
      [0, 1],
    ])
  })

  it('adds an arrow’s head of three vertices: its tip, and its base either side', () => {
    const w = wideOf(new Float32Array([0, 0, 0, 1, 2, 3]), true)
    expect(w.length).toBe(9 * 8)
    const head = [6, 7, 8].map(v => [w[v * 8 + 6]!, w[v * 8 + 7]!])
    expect(head).toEqual([
      [2, 0],
      [3, -1],
      [3, 1],
    ])
  })
})

describe('sphereMesh', () => {
  it('builds a ball round each centre, every vertex at its radius, faces turned outward', () => {
    const { pos, nrm } = sphereMesh(new Float32Array([1, 2, 3]), 0.5)
    expect(pos.length).toBe(nrm.length)
    for (let i = 0; i < pos.length; i += 3)
      expect(Math.hypot(pos[i]! - 1, pos[i + 1]! - 2, pos[i + 2]! - 3)).toBeCloseTo(0.5)
    for (let t = 0; t < pos.length; t += 9) {
      const p = [0, 1, 2].map(k => [pos[t + 3 * k]!, pos[t + 3 * k + 1]!, pos[t + 3 * k + 2]!])
      const u = [0, 1, 2].map(i => p[1]![i]! - p[0]![i]!)
      const v = [0, 1, 2].map(i => p[2]![i]! - p[0]![i]!)
      const face = [
        u[1]! * v[2]! - u[2]! * v[1]!,
        u[2]! * v[0]! - u[0]! * v[2]!,
        u[0]! * v[1]! - u[1]! * v[0]!,
      ]
      const n = [0, 1, 2].map(i => nrm[t + i]! + nrm[t + 3 + i]! + nrm[t + 6 + i]!)
      expect(face[0]! * n[0]! + face[1]! * n[1]! + face[2]! * n[2]!).toBeGreaterThan(0)
    }
  })
})
