// A minimal binary glTF reader: triangles and line primitives, with node transforms, in
// millimetres with z up, as the report's points are; glTF's own unit is the metre, and its y is up.
// Enough for the suite's shapes; anything it cannot read throws.

export interface Shape {
  // xyz per vertex.
  positions: Float32Array
  // Three vertex indices per triangle.
  triangles: Uint32Array
  // Two xyz ends per segment: the file's own line primitives, or the mesh's creases if none.
  edges: Float32Array
  // The smooth edges, from which a view finds the outline it sees: see topology.
  smooth: Float32Array
}

interface Accessor {
  bufferView?: number
  byteOffset?: number
  componentType: number
  count: number
  type: string
  normalized?: boolean
}

interface BufferView {
  byteOffset?: number
  byteLength: number
  byteStride?: number
}

interface Primitive {
  attributes: { POSITION?: number }
  indices?: number
  mode?: number
}

interface Node {
  mesh?: number
  children?: number[]
  matrix?: number[]
  translation?: number[]
  rotation?: number[]
  scale?: number[]
  extras?: { role?: string }
}

interface Gltf {
  accessors?: Accessor[]
  bufferViews?: BufferView[]
  meshes?: { primitives: Primitive[] }[]
  nodes?: Node[]
  scenes?: { nodes: number[] }[]
  scene?: number
  extensionsRequired?: string[]
}

const GLB_MAGIC = 0x46546c67
const CHUNK_JSON = 0x4e4f534a
const CHUNK_BIN = 0x004e4942
const COMPONENTS: Record<string, number> = { SCALAR: 1, VEC3: 3 }
// Each component type's size, reader, and the value a normalized one maps to 1. The signed and
// short kinds are KHR_mesh_quantization's, which the suite's files use for positions and normals.
const KINDS: Record<
  number,
  { size: number; max: number; get: (b: DataView, at: number) => number }
> = {
  5120: { size: 1, max: 127, get: (b, at) => b.getInt8(at) },
  5121: { size: 1, max: 255, get: (b, at) => b.getUint8(at) },
  5122: { size: 2, max: 32767, get: (b, at) => b.getInt16(at, true) },
  5123: { size: 2, max: 65535, get: (b, at) => b.getUint16(at, true) },
  5125: { size: 4, max: 0, get: (b, at) => b.getUint32(at, true) },
  5126: { size: 4, max: 0, get: (b, at) => b.getFloat32(at, true) },
}
// Extensions a file may require that this reader handles.
const UNDERSTOOD = new Set(['KHR_mesh_quantization'])
const MODE_LINES = 1
const MODE_LINE_STRIP = 3
const MODE_TRIANGLES = 4

type Mat = number[]

// glTF's (x, y, z) in metres is the suite's (x, -z, y) in millimetres. The suite's files turn z to y
// at their root node, so this undoes it.
const GLTF_TO_SUITE: Mat = [1000, 0, 0, 0, 0, 0, 1000, 0, 0, -1000, 0, 0, 0, 0, 0, 1]
// The suite's ghosts: what an operand leaves outside the result, open surfaces drawn behind it.
const GHOSTS = new Set(['ghost', 'ghost2'])

// Column-major, as glTF stores them.
function mul(a: Mat, b: Mat): Mat {
  const o = new Array<number>(16).fill(0)
  for (let c = 0; c < 4; c++)
    for (let r = 0; r < 4; r++)
      for (let k = 0; k < 4; k++) o[c * 4 + r]! += a[k * 4 + r]! * b[c * 4 + k]!
  return o
}

// A quarter turn's quaternion holds √½, so its matrix comes out a hair off 0 and ±1: snap those.
function exact(v: number): number {
  return Math.abs(v - Math.round(v)) < 1e-12 ? Math.round(v) : v
}

function local(n: Node): Mat {
  if (n.matrix) return n.matrix
  const [tx, ty, tz] = n.translation ?? [0, 0, 0]
  const [x, y, z, w] = n.rotation ?? [0, 0, 0, 1]
  const [sx, sy, sz] = n.scale ?? [1, 1, 1]
  return [
    exact(1 - 2 * (y! * y! + z! * z!)) * sx!,
    exact(2 * (x! * y! + z! * w!)) * sx!,
    exact(2 * (x! * z! - y! * w!)) * sx!,
    0,
    exact(2 * (x! * y! - z! * w!)) * sy!,
    exact(1 - 2 * (x! * x! + z! * z!)) * sy!,
    exact(2 * (y! * z! + x! * w!)) * sy!,
    0,
    exact(2 * (x! * z! + y! * w!)) * sz!,
    exact(2 * (y! * z! - x! * w!)) * sz!,
    exact(1 - 2 * (x! * x! + y! * y!)) * sz!,
    0,
    tx!,
    ty!,
    tz!,
    1,
  ]
}

function read(gltf: Gltf, bin: DataView, index: number, want: string): number[] {
  const a = gltf.accessors?.[index]
  if (!a) throw new Error(`glb: no accessor ${index}`)
  if (a.type !== want) throw new Error(`glb: accessor ${index} is ${a.type}, not ${want}`)
  const n = COMPONENTS[a.type]!
  const out = new Array<number>(a.count * n).fill(0)
  if (a.bufferView === undefined) return out
  const view = gltf.bufferViews?.[a.bufferView]
  if (!view) throw new Error(`glb: no buffer view ${a.bufferView}`)
  const kind = KINDS[a.componentType]
  if (!kind || (a.normalized && !kind.max))
    throw new Error(`glb: component type ${a.componentType} unsupported`)
  const stride = view.byteStride ?? kind.size * n
  const base = (view.byteOffset ?? 0) + (a.byteOffset ?? 0)
  for (let i = 0; i < a.count; i++) {
    for (let k = 0; k < n; k++) {
      const x = kind.get(bin, base + i * stride + k * kind.size)
      out[i * n + k] = a.normalized ? Math.max(x / kind.max, -1) : x
    }
  }
  return out
}

export function parseGlb(buf: ArrayBuffer): Shape {
  const dv = new DataView(buf)
  if (buf.byteLength < 20 || dv.getUint32(0, true) !== GLB_MAGIC) throw new Error('glb: not a GLB')
  let gltf: Gltf | null = null
  let bin = new DataView(new ArrayBuffer(0))
  for (let at = 12; at + 8 <= buf.byteLength;) {
    const len = dv.getUint32(at, true)
    const type = dv.getUint32(at + 4, true)
    if (type === CHUNK_JSON)
      gltf = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, at + 8, len))) as Gltf
    else if (type === CHUNK_BIN) bin = new DataView(buf, at + 8, len)
    at += 8 + len
  }
  if (!gltf) throw new Error('glb: no JSON chunk')
  const unknown = (gltf.extensionsRequired ?? []).filter(e => !UNDERSTOOD.has(e))
  if (unknown.length) throw new Error(`glb: needs ${unknown.join(', ')}`)

  const positions: number[] = []
  const triangles: number[] = []
  const lines: number[] = []
  const g = gltf
  const place = (m: Mat, p: number[], i: number) => {
    const [x, y, z] = [p[i]!, p[i + 1]!, p[i + 2]!]
    return [0, 1, 2].map(r => m[r]! * x + m[4 + r]! * y + m[8 + r]! * z + m[12 + r]!)
  }
  const visit = (index: number, parent: Mat, depth: number) => {
    const node = g.nodes?.[index]
    if (!node || depth > 64 || GHOSTS.has(node.extras?.role ?? '')) return
    const m = mul(parent, local(node))
    for (const prim of node.mesh !== undefined ? (g.meshes?.[node.mesh]?.primitives ?? []) : []) {
      if (prim.attributes.POSITION === undefined) continue
      const mode = prim.mode ?? MODE_TRIANGLES
      const p = read(g, bin, prim.attributes.POSITION, 'VEC3')
      const count = p.length / 3
      const idx =
        prim.indices !== undefined
          ? read(g, bin, prim.indices, 'SCALAR')
          : Array.from({ length: count }, (_, i) => i)
      if (mode === MODE_TRIANGLES) {
        const first = positions.length / 3
        for (let i = 0; i < count; i++) positions.push(...place(m, p, i * 3))
        for (const i of idx) triangles.push(first + i)
      } else if (mode === MODE_LINES || mode === MODE_LINE_STRIP) {
        const step = mode === MODE_LINES ? 2 : 1
        for (let k = 0; k + 1 < idx.length; k += step)
          lines.push(...place(m, p, idx[k]! * 3), ...place(m, p, idx[k + 1]! * 3))
      }
    }
    for (const c of node.children ?? []) visit(c, m, depth + 1)
  }
  const roots = g.scenes?.[g.scene ?? 0]?.nodes ?? (g.nodes ?? []).map((_, i) => i)
  for (const r of roots) visit(r, GLTF_TO_SUITE, 0)

  const P = Float32Array.from(positions)
  const T = Uint32Array.from(triangles)
  const t = topology(P, T)
  const edges = lines.length ? Float32Array.from(lines) : t.creases
  return { positions: P, triangles: T, edges, smooth: t.smooth }
}

export interface Topology {
  // Two xyz ends per segment where the mesh folds by more than the crease angle, or ends.
  creases: Float32Array
  // Per edge between two faces that meet smoothly: its two ends, then both faces' normals.
  smooth: Float32Array
}

// Where the mesh folds by more than `degrees`, or ends: the edges a CAD view draws. The rest are
// kept with their faces' normals, so a view can draw the outline of a curved surface.
export function topology(positions: Float32Array, triangles: Uint32Array, degrees = 20): Topology {
  // Weld by position, since a mesh splits vertices along every seam and crease.
  const weld = new Map<string, number>()
  const id = new Uint32Array(positions.length / 3)
  for (let i = 0; i < id.length; i++) {
    const key = [0, 1, 2].map(k => Math.round(positions[i * 3 + k]! * 1e5)).join(',')
    let w = weld.get(key)
    if (w === undefined) weld.set(key, (w = weld.size))
    id[i] = w
  }
  const normals: (number[] | null)[] = []
  for (let t = 0; t < triangles.length; t += 3) {
    const [a, b, c] = [0, 1, 2].map(k => triangles[t + k]! * 3)
    const u = [0, 1, 2].map(k => positions[b! + k]! - positions[a! + k]!)
    const v = [0, 1, 2].map(k => positions[c! + k]! - positions[a! + k]!)
    const n = [
      u[1]! * v[2]! - u[2]! * v[1]!,
      u[2]! * v[0]! - u[0]! * v[2]!,
      u[0]! * v[1]! - u[1]! * v[0]!,
    ]
    const len = Math.hypot(n[0]!, n[1]!, n[2]!)
    // A sliver with no area, such as a row at a sphere's pole, has no side and bounds nothing.
    normals.push(len > 1e-12 ? n.map(x => x / len) : null)
  }
  const byEdge = new Map<string, { a: number; b: number; faces: number[] }>()
  for (let t = 0; t < triangles.length; t += 3) {
    if (!normals[t / 3]) continue
    for (let k = 0; k < 3; k++) {
      const a = triangles[t + k]!
      const b = triangles[t + ((k + 1) % 3)]!
      const [wa, wb] = [id[a]!, id[b]!]
      if (wa === wb) continue
      const key = wa < wb ? `${wa},${wb}` : `${wb},${wa}`
      const e = byEdge.get(key) ?? { a, b, faces: [] }
      e.faces.push(t / 3)
      byEdge.set(key, e)
    }
  }
  const cos = Math.cos((degrees * Math.PI) / 180)
  const creases: number[] = []
  const smooth: number[] = []
  const at = (v: number) => [positions[v * 3]!, positions[v * 3 + 1]!, positions[v * 3 + 2]!]
  for (const e of byEdge.values()) {
    const nf = normals[e.faces[0]!]!
    const ng = e.faces.length === 2 ? normals[e.faces[1]!]! : null
    if (ng && nf.reduce((s, x, k) => s + x * ng[k]!, 0) >= cos)
      smooth.push(...at(e.a), ...at(e.b), ...nf, ...ng)
    else creases.push(...at(e.a), ...at(e.b))
  }
  return { creases: Float32Array.from(creases), smooth: Float32Array.from(smooth) }
}
