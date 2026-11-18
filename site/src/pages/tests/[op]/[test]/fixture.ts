// Binary glTF files for the tests: a box, with or without its edges as a line primitive, in
// millimetres under a root node that scales them to glTF's metres and turns z to y, as the suite
// writes them.

export const Z_UP_TO_Y_UP = [-Math.SQRT1_2, 0, 0, Math.SQRT1_2]

export function glbOf(
  positions: number[],
  triangles: number[],
  lines: number[] = [],
  node: Record<string, unknown> = {},
  others: Record<string, unknown>[] = [],
): ArrayBuffer {
  const pos = new Float32Array(positions)
  const tri = new Uint16Array(triangles)
  const lin = new Uint16Array(lines)
  const pad = (n: number) => (n + 3) & ~3
  const offTri = pad(pos.byteLength)
  const offLin = offTri + pad(tri.byteLength)
  const binLen = offLin + pad(lin.byteLength)
  const primitives: Record<string, unknown>[] = [{ attributes: { POSITION: 0 }, indices: 1 }]
  if (lines.length) primitives.push({ attributes: { POSITION: 0 }, indices: 2, mode: 1 })
  const json = {
    asset: { version: '2.0' },
    scene: 0,
    scenes: [{ nodes: [1] }],
    nodes: [
      { mesh: 0, ...node },
      {
        children: [0, ...others.map((_, i) => i + 2)],
        rotation: Z_UP_TO_Y_UP,
        scale: [1e-3, 1e-3, 1e-3],
      },
      ...others,
    ],
    meshes: [{ primitives }],
    buffers: [{ byteLength: binLen }],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: pos.byteLength },
      { buffer: 0, byteOffset: offTri, byteLength: tri.byteLength },
      { buffer: 0, byteOffset: offLin, byteLength: lin.byteLength },
    ],
    accessors: [
      { bufferView: 0, componentType: 5126, count: pos.length / 3, type: 'VEC3' },
      { bufferView: 1, componentType: 5123, count: tri.length, type: 'SCALAR' },
      { bufferView: 2, componentType: 5123, count: lin.length, type: 'SCALAR' },
    ],
  }
  const text = new TextEncoder().encode(JSON.stringify(json))
  const jsonLen = pad(text.length)
  const out = new ArrayBuffer(12 + 8 + jsonLen + 8 + binLen)
  const dv = new DataView(out)
  const bytes = new Uint8Array(out)
  dv.setUint32(0, 0x46546c67, true)
  dv.setUint32(4, 2, true)
  dv.setUint32(8, out.byteLength, true)
  dv.setUint32(12, jsonLen, true)
  dv.setUint32(16, 0x4e4f534a, true)
  bytes.fill(0x20, 20, 20 + jsonLen)
  bytes.set(text, 20)
  const b = 20 + jsonLen
  dv.setUint32(b, binLen, true)
  dv.setUint32(b + 4, 0x004e4942, true)
  bytes.set(new Uint8Array(pos.buffer), b + 8)
  bytes.set(new Uint8Array(tri.buffer), b + 8 + offTri)
  bytes.set(new Uint8Array(lin.buffer), b + 8 + offLin)
  return out
}

// A box from the origin to `size`, wound outward, its faces sharing corners.
export function box(size: [number, number, number]): { positions: number[]; triangles: number[] } {
  const positions: number[] = []
  for (let i = 0; i < 8; i++)
    positions.push(i & 1 ? size[0] : 0, i & 2 ? size[1] : 0, i & 4 ? size[2] : 0)
  // prettier-ignore
  const triangles = [
    0, 2, 1, 1, 2, 3,
    4, 5, 6, 5, 7, 6,
    0, 1, 4, 1, 5, 4,
    2, 6, 3, 3, 6, 7,
    0, 4, 2, 2, 4, 6,
    1, 3, 5, 3, 7, 5,
  ]
  return { positions, triangles }
}
