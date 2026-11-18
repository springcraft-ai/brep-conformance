// The Scan view's shaded solid: a depth-buffered rasterizer for an orthographic view, lit per
// pixel. Plain arrays, so the page ships no 3D library.
import type { Vec3 } from './vec'

export interface Mesh {
  // xyz per vertex.
  positions: Float32Array
  // A unit normal per vertex, or zeros where the triangle's own normal is to be used.
  normals: Float32Array
  triangles: Uint32Array
}

export interface Raster {
  w: number
  h: number
  // Depth toward the viewer of the nearest surface in each pixel; -Infinity where there is none.
  depth: Float32Array
  rgba: Uint8ClampedArray
}

export function raster(w: number, h: number): Raster {
  return {
    w,
    h,
    depth: new Float32Array(w * h).fill(-Infinity),
    rgba: new Uint8ClampedArray(w * h * 4),
  }
}

// A colour for a unit normal in view space: x right, y up, z toward the viewer.
export type Shade = (x: number, y: number, z: number) => [number, number, number]

// Brushed metal in a studio: a sky above and a dark floor below, a key light from the upper left,
// a fill from the lower right, a white highlight, and a cool rim where the surface turns away. The
// lights are fixed to the view, so the shape keeps one look whichever way it is turned.
export function studio(base: [number, number, number]): Shade {
  const unit = (v: Vec3) => {
    const l = Math.hypot(...v)
    return v.map(c => c / l) as Vec3
  }
  const key = unit([-0.45, 0.75, 0.55])
  const fill = unit([0.7, -0.25, 0.45])
  const half = unit([key[0], key[1], key[2] + 1])
  const tone = (c: number) => 255 * Math.min(1, c) ** (1 / 1.15)
  return (x, y, z) => {
    const sky = y * 0.5 + 0.5
    const light =
      0.08 +
      0.22 * sky +
      0.8 * Math.max(0, x * key[0] + y * key[1] + z * key[2]) +
      0.22 * Math.max(0, x * fill[0] + y * fill[1] + z * fill[2])
    const shine = 0.55 * Math.max(0, x * half[0] + y * half[1] + z * half[2]) ** 48
    const rim = (1 - Math.max(0, z)) ** 3
    return [
      tone((base[0] / 255) * light + shine + 0.1 * rim),
      tone((base[1] / 255) * light + shine + 0.13 * rim),
      tone((base[2] / 255) * light + shine + 0.18 * rim),
    ]
  }
}

// Fills `r` with `mesh` as `at` projects it to pixels and depth. Each pixel takes `shade` of its
// normal, turned into the view by `basis` (right, up, toward) and interpolated across the
// triangle. A face seen from behind is shaded as its front, so a mesh wound either way, or an open
// one, still reads as a surface.
export function rasterize(
  r: Raster,
  mesh: Mesh,
  at: (x: number, y: number, z: number) => [number, number, number],
  basis: [Vec3, Vec3, Vec3],
  shade: Shade,
) {
  r.depth.fill(-Infinity)
  r.rgba.fill(0)
  const { positions: P, normals: N, triangles: T } = mesh
  const [right, up, toward] = basis
  const count = P.length / 3
  const S = new Float32Array(count * 3)
  // View-space normals; a vertex with none is marked by a zero vector.
  const V = new Float32Array(count * 3)
  const view = (x: number, y: number, z: number, out: Float32Array, at: number) => {
    out[at] = x * right[0] + y * right[1] + z * right[2]
    out[at + 1] = x * up[0] + y * up[1] + z * up[2]
    out[at + 2] = x * toward[0] + y * toward[1] + z * toward[2]
  }
  for (let i = 0; i < count; i++) {
    S.set(at(P[i * 3]!, P[i * 3 + 1]!, P[i * 3 + 2]!), i * 3)
    view(N[i * 3] ?? 0, N[i * 3 + 1] ?? 0, N[i * 3 + 2] ?? 0, V, i * 3)
  }
  const own = new Float32Array(9)
  for (let t = 0; t + 2 < T.length; t += 3) {
    const [a, b, c] = [T[t]!, T[t + 1]!, T[t + 2]!]
    const [ax, ay, az] = [S[a * 3]!, S[a * 3 + 1]!, S[a * 3 + 2]!]
    const [bx, by, bz] = [S[b * 3]!, S[b * 3 + 1]!, S[b * 3 + 2]!]
    const [cx, cy, cz] = [S[c * 3]!, S[c * 3 + 1]!, S[c * 3 + 2]!]
    const area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
    if (Math.abs(area) < 1e-9) continue
    const none = (v: number) => !V[v * 3] && !V[v * 3 + 1] && !V[v * 3 + 2]
    let src = V
    let [ia, ib, ic] = [a * 3, b * 3, c * 3]
    if (none(a) || none(b) || none(c)) {
      const u = [0, 1, 2].map(k => P[b * 3 + k]! - P[a * 3 + k]!)
      const v = [0, 1, 2].map(k => P[c * 3 + k]! - P[a * 3 + k]!)
      const n = [
        u[1]! * v[2]! - u[2]! * v[1]!,
        u[2]! * v[0]! - u[0]! * v[2]!,
        u[0]! * v[1]! - u[1]! * v[0]!,
      ]
      view(n[0]!, n[1]!, n[2]!, own, 0)
      own.copyWithin(3, 0, 3)
      own.copyWithin(6, 0, 3)
      src = own
      ;[ia, ib, ic] = [0, 3, 6]
    }
    const x0 = Math.max(0, Math.floor(Math.min(ax, bx, cx)))
    const x1 = Math.min(r.w - 1, Math.ceil(Math.max(ax, bx, cx)))
    const y0 = Math.max(0, Math.floor(Math.min(ay, by, cy)))
    const y1 = Math.min(r.h - 1, Math.ceil(Math.max(ay, by, cy)))
    for (let y = y0; y <= y1; y++) {
      const py = y + 0.5
      for (let x = x0; x <= x1; x++) {
        const px = x + 0.5
        const wa = ((bx - px) * (cy - py) - (by - py) * (cx - px)) / area
        const wb = ((cx - px) * (ay - py) - (cy - py) * (ax - px)) / area
        const wc = 1 - wa - wb
        if (wa < 0 || wb < 0 || wc < 0) continue
        const z = wa * az + wb * bz + wc * cz
        const i = y * r.w + x
        if (z <= r.depth[i]!) continue
        r.depth[i] = z
        let nx = wa * src[ia]! + wb * src[ib]! + wc * src[ic]!
        let ny = wa * src[ia + 1]! + wb * src[ib + 1]! + wc * src[ic + 1]!
        let nz = wa * src[ia + 2]! + wb * src[ib + 2]! + wc * src[ic + 2]!
        const len = Math.hypot(nx, ny, nz) || 1
        const flip = nz < 0 ? -len : len
        ;[nx, ny, nz] = [nx / flip, ny / flip, nz / flip]
        const [cr, cg, cb] = shade(nx, ny, nz)
        r.rgba[i * 4] = cr
        r.rgba[i * 4 + 1] = cg
        r.rgba[i * 4 + 2] = cb
        r.rgba[i * 4 + 3] = 255
      }
    }
  }
}
