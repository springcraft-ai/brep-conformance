import { describe, expect, it } from 'vitest'
import { prepare } from './gl'

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
