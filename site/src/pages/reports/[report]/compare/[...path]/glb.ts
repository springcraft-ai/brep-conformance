// A minimal binary glTF reader: triangles and line primitives, with node transforms, in
// millimetres with z up, as the report's points are; glTF's own unit is the metre, and its y is up.
// Enough for the suite's shapes; anything it cannot read throws.

export interface Shape {
  // xyz per vertex.
  positions: Float32Array
  // A unit normal per vertex, or zeros where the file gives none.
  normals: Float32Array
  // Three vertex indices per triangle.
  triangles: Uint32Array
  // Two xyz ends per segment: the file's own line primitives, none where it has none.
  edges: Float32Array
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
  attributes: { POSITION?: number; NORMAL?: number }
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
  scenes?: { nodes: number[]; name?: string; extras?: { note?: string } }[]
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

type Xyz = [number, number, number]

// What the suite lays over every view of a case, in its own millimetres, z up: the points the case
// asks about or the edges and faces its steps name, a box, or a plane.
export interface Overlay {
  points: Xyz[]
  box: [Xyz, Xyz] | null
  plane: { origin: Xyz; normal: Xyz } | null
}

const xyz = (v: unknown): v is Xyz =>
  Array.isArray(v) && v.length === 3 && v.every(x => typeof x === 'number' && Number.isFinite(x))

// The overlay the file's extras carry; whatever is missing or malformed is left out.
export function parseOverlay(buf: ArrayBuffer): Overlay {
  const extras = ((chunks(buf).gltf as { extras?: Record<string, unknown> }).extras ?? {}) as {
    points?: unknown
    box?: unknown
    plane?: { origin?: unknown; normal?: unknown }
  }
  const points = Array.isArray(extras.points) ? extras.points.filter(xyz) : []
  const b = extras.box
  const box =
    Array.isArray(b) && b.length === 2 && xyz(b[0]) && xyz(b[1])
      ? ([b[0], b[1]] as [Xyz, Xyz])
      : null
  const p = extras.plane
  const plane = p && xyz(p.origin) && xyz(p.normal) ? { origin: p.origin, normal: p.normal } : null
  return { points, box, plane }
}

// One view of a case as its file draws it: a scene, and its solids merged by role.
export interface GlbView {
  name: string
  note?: string
  // Each role's solids as one shape: `main`, `alt`, and the suite's ghosts.
  solids: { role: string; shape: Shape }[]
}

// The file's first scene, ghosts aside, as one shape.
export function parseGlb(buf: ArrayBuffer): Shape {
  const { gltf: g, bin } = chunks(buf)
  const roots = g.scenes?.[g.scene ?? 0]?.nodes ?? (g.nodes ?? []).map((_, i) => i)
  return gather(
    g,
    bin,
    roots.map(r => [r, GLTF_TO_SUITE]),
    r => GHOSTS.has(r),
  )
}

// Every scene of the file, each solid under the first node that names a role, merged by role.
export function parseViews(buf: ArrayBuffer): GlbView[] {
  const { gltf: g, bin } = chunks(buf)
  return (g.scenes ?? []).map((scene, n) => {
    const byRole = new Map<string, [number, Mat][]>()
    const add = (role: string, index: number, parent: Mat) => {
      const list = byRole.get(role) ?? []
      list.push([index, parent])
      byRole.set(role, list)
    }
    const walk = (index: number, parent: Mat, depth: number) => {
      const node = g.nodes?.[index]
      if (!node || depth > 64) return
      const role = node.extras?.role
      if (role || node.mesh !== undefined) return add(role ?? 'main', index, parent)
      const m = mul(parent, local(node))
      for (const c of node.children ?? []) walk(c, m, depth + 1)
    }
    for (const r of scene.nodes) walk(r, GLTF_TO_SUITE, 0)
    return {
      name: scene.name ?? `view ${n + 1}`,
      note: scene.extras?.note,
      solids: [...byRole].map(([role, starts]) => ({
        role,
        shape: gather(g, bin, starts, () => false),
      })),
    }
  })
}

function chunks(buf: ArrayBuffer): { gltf: Gltf; bin: DataView } {
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
  return { gltf, bin }
}

// The geometry below each of `starts`, a node and the transform above it, as one shape; a node
// whose role `skip` names is left out with everything below it.
function gather(
  g: Gltf,
  bin: DataView,
  starts: [number, Mat][],
  skip: (role: string) => boolean,
): Shape {
  const positions: number[] = []
  const normals: number[] = []
  const triangles: number[] = []
  const lines: number[] = []
  const place = (m: Mat, p: number[], i: number) => {
    const [x, y, z] = [p[i]!, p[i + 1]!, p[i + 2]!]
    return [0, 1, 2].map(r => m[r]! * x + m[4 + r]! * y + m[8 + r]! * z + m[12 + r]!)
  }
  // By the cofactors of the transform, which keep a normal square to its face under any scale.
  const turn = (m: Mat, n: number[], i: number) => {
    const col = (c: number) => [m[c * 4]!, m[c * 4 + 1]!, m[c * 4 + 2]!]
    const cross = (u: number[], v: number[]) => [
      u[1]! * v[2]! - u[2]! * v[1]!,
      u[2]! * v[0]! - u[0]! * v[2]!,
      u[0]! * v[1]! - u[1]! * v[0]!,
    ]
    const rows = [cross(col(1), col(2)), cross(col(2), col(0)), cross(col(0), col(1))]
    const o = [0, 1, 2].map(r => rows.reduce((s, row, k) => s + row[r]! * n[i + k]!, 0))
    const len = Math.hypot(o[0]!, o[1]!, o[2]!)
    return len > 1e-12 ? o.map(x => x / len) : [0, 0, 0]
  }
  const visit = (index: number, parent: Mat, depth: number) => {
    const node = g.nodes?.[index]
    if (!node || depth > 64 || skip(node.extras?.role ?? '')) return
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
        const n =
          prim.attributes.NORMAL !== undefined ? read(g, bin, prim.attributes.NORMAL, 'VEC3') : []
        for (let i = 0; i < count; i++) {
          positions.push(...place(m, p, i * 3))
          normals.push(...(n.length === p.length ? turn(m, n, i * 3) : [0, 0, 0]))
        }
        for (const i of idx) triangles.push(first + i)
      } else if (mode === MODE_LINES || mode === MODE_LINE_STRIP) {
        const step = mode === MODE_LINES ? 2 : 1
        for (let k = 0; k + 1 < idx.length; k += step)
          lines.push(...place(m, p, idx[k]! * 3), ...place(m, p, idx[k + 1]! * 3))
      }
    }
    for (const c of node.children ?? []) visit(c, m, depth + 1)
  }
  for (const [r, m] of starts) visit(r, m, 0)

  const P = Float32Array.from(positions)
  const T = Uint32Array.from(triangles)
  // The kernel's own edges only: a solid it gave none, such as a sphere, is drawn with none.
  return {
    positions: P,
    normals: Float32Array.from(normals),
    triangles: T,
    edges: Float32Array.from(lines),
  }
}
