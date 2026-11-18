// The shaded solid under the Scan view's edges: a depth-buffered rasterizer for an orthographic
// view, and the test that hides an edge behind it. Plain arrays, so the page ships no 3D library.
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

const AMBIENT = 0.3

// Fills `r` with `mesh` as `at` projects it to pixels and depth, lit from `light`. Both sides of
// a face are lit alike, so a mesh wound either way, or an open one, still reads as a surface.
export function rasterize(
  r: Raster,
  mesh: Mesh,
  at: (x: number, y: number, z: number) => [number, number, number],
  light: Vec3,
  face: [number, number, number],
) {
  r.depth.fill(-Infinity)
  r.rgba.fill(0)
  const { positions: P, normals: N, triangles: T } = mesh
  const count = P.length / 3
  const S = new Float32Array(count * 3)
  const lit = new Float32Array(count)
  const shade = (x: number, y: number, z: number) =>
    AMBIENT + (1 - AMBIENT) * Math.abs(x * light[0] + y * light[1] + z * light[2])
  for (let i = 0; i < count; i++) {
    S.set(at(P[i * 3]!, P[i * 3 + 1]!, P[i * 3 + 2]!), i * 3)
    const [nx, ny, nz] = [N[i * 3] ?? 0, N[i * 3 + 1] ?? 0, N[i * 3 + 2] ?? 0]
    lit[i] = nx || ny || nz ? shade(nx, ny, nz) : -1
  }
  for (let t = 0; t + 2 < T.length; t += 3) {
    const [a, b, c] = [T[t]!, T[t + 1]!, T[t + 2]!]
    const [ax, ay, az] = [S[a * 3]!, S[a * 3 + 1]!, S[a * 3 + 2]!]
    const [bx, by, bz] = [S[b * 3]!, S[b * 3 + 1]!, S[b * 3 + 2]!]
    const [cx, cy, cz] = [S[c * 3]!, S[c * 3 + 1]!, S[c * 3 + 2]!]
    const area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
    if (Math.abs(area) < 1e-9) continue
    let [la, lb, lc] = [lit[a]!, lit[b]!, lit[c]!]
    if (la < 0 || lb < 0 || lc < 0) {
      const u = [0, 1, 2].map(k => P[b * 3 + k]! - P[a * 3 + k]!)
      const v = [0, 1, 2].map(k => P[c * 3 + k]! - P[a * 3 + k]!)
      const n = [
        u[1]! * v[2]! - u[2]! * v[1]!,
        u[2]! * v[0]! - u[0]! * v[2]!,
        u[0]! * v[1]! - u[1]! * v[0]!,
      ]
      const len = Math.hypot(n[0]!, n[1]!, n[2]!) || 1
      la = lb = lc = shade(n[0]! / len, n[1]! / len, n[2]! / len)
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
        const l = wa * la + wb * lb + wc * lc
        r.rgba[i * 4] = face[0] * l
        r.rgba[i * 4 + 1] = face[1] * l
        r.rgba[i * 4 + 2] = face[2] * l
        r.rgba[i * 4 + 3] = 255
      }
    }
  }
}

// The parts of the segment from `a` to `b` (pixels and depth) that no surface hides, as pairs of
// parameters along it. An edge lies in the surface it bounds, so it counts as hidden only where
// every pixel around it is nearer than the edge by more than `bias`.
export function visibleRuns(r: Raster, a: number[], b: number[], bias: number): number[] {
  const hidden = (x: number, y: number, z: number) => {
    const [xi, yi] = [Math.floor(x), Math.floor(y)]
    for (let j = yi - 1; j <= yi + 1; j++) {
      for (let i = xi - 1; i <= xi + 1; i++) {
        if (i < 0 || j < 0 || i >= r.w || j >= r.h) return false
        if (r.depth[j * r.w + i]! <= z + bias) return false
      }
    }
    return true
  }
  const n = Math.max(1, Math.ceil(Math.hypot(b[0]! - a[0]!, b[1]! - a[1]!) / 2))
  const half = 0.5 / n
  const runs: number[] = []
  let open = false
  for (let i = 0; i <= n; i++) {
    const t = i / n
    const seen = !hidden(
      a[0]! + t * (b[0]! - a[0]!),
      a[1]! + t * (b[1]! - a[1]!),
      a[2]! + t * (b[2]! - a[2]!),
    )
    if (seen && !open) runs.push(Math.max(0, t - half))
    if (!seen && open) runs.push(t - half)
    open = seen
  }
  if (open) runs.push(1)
  return runs
}
