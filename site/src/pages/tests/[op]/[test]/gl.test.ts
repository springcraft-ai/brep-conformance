import { describe, expect, it } from 'vitest'
import { prepare, quads } from './gl'

describe('quads', () => {
  it('makes each segment two triangles, every corner holding both ends, its end and its side', () => {
    const q = quads(new Float32Array([0, 0, 0, 1, 2, 3]))
    expect(q.length).toBe(6 * 8)
    for (let c = 0; c < 6; c++)
      expect(Array.from(q.subarray(c * 8, c * 8 + 6))).toEqual([0, 0, 0, 1, 2, 3])
    const tw = [0, 1, 2, 3, 4, 5].map(c => [q[c * 8 + 6], q[c * 8 + 7]])
    expect(tw).toEqual([
      [0, 1],
      [0, -1],
      [1, 1],
      [1, 1],
      [0, -1],
      [1, -1],
    ])
  })
})

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
