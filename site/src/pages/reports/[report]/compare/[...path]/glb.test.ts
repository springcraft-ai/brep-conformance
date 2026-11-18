import { describe, expect, it } from 'vitest'
import { box, glbOf, Z_UP_TO_Y_UP } from './fixture'
import { overlayOf, parseGlb } from './glb'

// A triangle and one edge as the suite writes them: positions as SHORT padded to 8 bytes, normals
// as normalized BYTE, and the node's translation and scale taking them back.
function quantized(required: string[]): ArrayBuffer {
  const bin = new ArrayBuffer(48)
  const dv = new DataView(bin)
  const points = [
    [0, 0, 0],
    [1, 0, 0],
    [0, -2, 0],
  ]
  points.forEach((p, i) => p.forEach((x, k) => dv.setInt16(i * 8 + k * 2, x, true)))
  for (let i = 0; i < 3; i++) dv.setInt8(24 + i * 4 + 2, 127)
  ;[0, 1, 2, 0, 1, 0].forEach((x, i) => dv.setUint16(36 + i * 2, x, true))
  const json = {
    asset: { version: '2.0' },
    extensionsUsed: required,
    extensionsRequired: required,
    scene: 0,
    scenes: [{ nodes: [1] }],
    nodes: [
      { mesh: 0, translation: [1, 2, 3], scale: [0.5, 0.5, 0.5] },
      { children: [0], rotation: Z_UP_TO_Y_UP, scale: [1e-3, 1e-3, 1e-3] },
    ],
    meshes: [
      {
        primitives: [
          { attributes: { POSITION: 0, NORMAL: 1 }, indices: 2, mode: 4 },
          { attributes: { POSITION: 0 }, indices: 3, mode: 1 },
        ],
      },
    ],
    buffers: [{ byteLength: 48 }],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: 24, byteStride: 8 },
      { buffer: 0, byteOffset: 24, byteLength: 12, byteStride: 4 },
      { buffer: 0, byteOffset: 36, byteLength: 12 },
    ],
    accessors: [
      { bufferView: 0, componentType: 5122, count: 3, type: 'VEC3' },
      { bufferView: 1, componentType: 5120, normalized: true, count: 3, type: 'VEC3' },
      { bufferView: 2, componentType: 5123, count: 3, type: 'SCALAR' },
      { bufferView: 2, byteOffset: 6, componentType: 5123, count: 2, type: 'SCALAR' },
    ],
  }
  const text = new TextEncoder().encode(JSON.stringify(json))
  const jsonLen = (text.length + 3) & ~3
  const out = new ArrayBuffer(12 + 8 + jsonLen + 8 + bin.byteLength)
  const o = new DataView(out)
  const bytes = new Uint8Array(out)
  o.setUint32(0, 0x46546c67, true)
  o.setUint32(4, 2, true)
  o.setUint32(8, out.byteLength, true)
  o.setUint32(12, jsonLen, true)
  o.setUint32(16, 0x4e4f534a, true)
  bytes.fill(0x20, 20, 20 + jsonLen)
  bytes.set(text, 20)
  o.setUint32(20 + jsonLen, bin.byteLength, true)
  o.setUint32(24 + jsonLen, 0x004e4942, true)
  bytes.set(new Uint8Array(bin), 28 + jsonLen)
  return out
}

describe('parseGlb', () => {
  const { positions, triangles } = box([2, 3, 4])

  it('reads triangles, and draws no edge the file does not give', () => {
    // A sphere's file has no edges; folds found in its mesh would draw lines that are not there.
    const s = parseGlb(glbOf(positions, triangles))
    expect(s.positions.length).toBe(24)
    expect(s.triangles.length).toBe(36)
    expect(s.edges.length).toBe(0)
  })

  it('reads no normals from a file without them, and turns the ones a file has', () => {
    expect(Array.from(parseGlb(glbOf(positions, triangles)).normals)).toEqual(new Array(24).fill(0))
    const n = parseGlb(quantized(['KHR_mesh_quantization'])).normals
    expect(Array.from(n)).toEqual([0, 0, 1, 0, 0, 1, 0, 0, 1])
  })

  it('keeps the file’s own edges when it has them', () => {
    const s = parseGlb(glbOf(positions, triangles, [0, 1, 1, 3]))
    expect(Array.from(s.edges)).toEqual([0, 0, 0, 2, 0, 0, 2, 0, 0, 2, 3, 0])
  })

  it('applies node transforms', () => {
    const node = { translation: [10, 0, 0], scale: [2, 1, 1] }
    const s = parseGlb(glbOf(positions, triangles, [], node))
    expect(s.positions[0]).toBe(10)
    expect(s.positions[3]).toBe(14)
  })

  it('reads KHR_mesh_quantization: SHORT positions under a dequantizing node', () => {
    const s = parseGlb(quantized(['KHR_mesh_quantization']))
    expect(Array.from(s.positions)).toEqual([1, 2, 3, 1.5, 2, 3, 1, 1, 3])
    expect(Array.from(s.triangles)).toEqual([0, 1, 2])
    expect(Array.from(s.edges)).toEqual([1, 2, 3, 1.5, 2, 3])
  })

  it('reads glTF’s metres as millimetres', () => {
    const metres = parseGlb(glbOf(positions, triangles, [], { scale: [1e3, 1e3, 1e3] }))
    expect(metres.positions[3]).toBe(2000)
  })

  it('leaves out the ghosts, which are open surfaces behind the shape', () => {
    const ghost = { mesh: 0, translation: [100, 0, 0], extras: { role: 'ghost' } }
    const s = parseGlb(glbOf(positions, triangles, [0, 1], { extras: { role: 'main' } }, [ghost]))
    expect(s.triangles.length).toBe(36)
    expect(Math.max(...s.positions)).toBe(4)
    expect(s.edges.length).toBe(6)
  })

  it('refuses a file that requires an extension it does not know', () => {
    expect(() => parseGlb(quantized(['KHR_mesh_quantization', 'EXT_meshopt_compression']))).toThrow(
      /needs EXT_meshopt_compression/,
    )
  })

  it('refuses what is not a GLB', () => {
    expect(() => parseGlb(new ArrayBuffer(32))).toThrow(/not a GLB/)
  })
})

describe('overlayOf', () => {
  it('reads the answers a query is checked against, and the direction a drawing looks from', () => {
    const o = overlayOf({
      lines: [
        {
          seen: false,
          pts: [
            [0, 0, 0],
            [1, 0, 0],
          ],
        },
      ],
      look: [0, 0, 1],
      closest: [{ at: [1, 2, 3], foot: null, distance: 2 }],
      sides: [{ at: [0, 0, 0], side: 'on' }],
      normals: [{ at: [0, 0, 0], n: [0, 1, 0] }],
    })
    expect(o.lines).toEqual([
      {
        seen: false,
        pts: [
          [0, 0, 0],
          [1, 0, 0],
        ],
      },
    ])
    expect(o.look).toEqual([0, 0, 1])
    expect(o.closest).toEqual([{ at: [1, 2, 3], foot: null, distance: 2 }])
    expect(o.sides).toEqual([{ at: [0, 0, 0], side: 'on' }])
    expect(o.normals).toEqual([{ at: [0, 0, 0], n: [0, 1, 0] }])
  })

  it('leaves out what is malformed, and gives empty lists where the file has nothing', () => {
    const o = overlayOf({
      lines: [
        { seen: 'yes', pts: [[0, 0, 0]] },
        {
          seen: true,
          pts: [
            [0, 0],
            [1, 1, 1],
          ],
        },
      ],
      look: [0, 0],
      sides: [{ at: [0, 0, 0], side: 'beside' }],
      normals: [{ at: [0, 0, 0], n: 'up' }],
    })
    expect(o.lines).toEqual([])
    expect(o.look).toBeNull()
    expect(o.sides).toEqual([])
    expect(o.normals).toEqual([])
    expect(overlayOf(undefined)).toEqual({
      points: [],
      box: null,
      plane: null,
      lines: [],
      look: null,
      closest: [],
      sides: [],
      normals: [],
    })
  })
})
