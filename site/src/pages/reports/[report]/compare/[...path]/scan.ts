// The Scan view: the shape as a shaded solid with its edges, drawn by WebGL2 (gl.ts), and a
// kernel's misplaced points over it on a 2D canvas, which also takes the pointer. Orthographic,
// z up, turned by dragging.
import { Renderer, type Mesh, type OverlayGeometry, type Rgb } from './gl'
import type { Overlay, Side } from './glb'
import type { Vec3 } from './vec'

export interface Mark {
  p: Vec3
  // Where the kernel placed the point: inside the result, or not.
  held: boolean
}

// A solid drawn faint behind the subject, in one of two tones: its fill only where the subject
// is not, its edges only where the subject does not hide them.
export interface Ghost extends Mesh {
  tone: 0 | 1
}

export interface Scene extends Mesh {
  // The second subject, in its own tone: so two parts that clash, a part and its mirror image, or
  // the pieces of a split read apart where they meet.
  alt: Mesh
  ghosts: Ghost[]
  marks: Mark[]
  // What the case lays over its views: dots at its points, a box, a plane.
  overlay?: Overlay
  // The box every view is fitted to, when it should stay put as the scene changes.
  frame?: { lo: Vec3; hi: Vec3 }
}

export interface View {
  yaw: number
  pitch: number
  // How much nearer than the fitted view, and how far the centre is moved across the screen,
  // in model units along the screen's right and up.
  zoom: number
  pan: [number, number]
}

export const HOME: View = { yaw: -Math.PI / 4 - 0.35, pitch: 0.5, zoom: 1, pan: [0, 0] }

// The view a scene opens on: HOME, or for a case about a plane, HOME turned to look nearly along the
// plane, a little from one side: the pieces either side of it, and the cut between them, are seen
// side by side, where looking across the plane puts the near piece in front of the cut.
const ALONG_PLANE = 0.12

export function homeOf(o: Overlay | undefined): View {
  const look = o?.look
  const ll = look ? Math.hypot(...look) : 0
  if (look && ll) {
    return { ...HOME, yaw: Math.atan2(look[1], look[0]), pitch: Math.asin(look[2] / ll) }
  }
  const n = o?.plane?.normal
  const len = n ? Math.hypot(...n) : 0
  if (!n || !len) return { ...HOME }
  const z = n.map(x => x / len)
  const cp = Math.cos(HOME.pitch)
  const home: Vec3 = [cp * Math.cos(HOME.yaw), cp * Math.sin(HOME.yaw), Math.sin(HOME.pitch)]
  const across = home[0] * z[0]! + home[1] * z[1]! + home[2] * z[2]!
  const side = across < 0 ? -1 : 1
  const t = [0, 1, 2].map(k => home[k]! - across * z[k]! + ALONG_PLANE * side * z[k]!)
  const tl = Math.hypot(...t)
  if (tl < 1e-9) return { ...HOME }
  return { ...HOME, yaw: Math.atan2(t[1]!, t[0]!), pitch: Math.asin(t[2]! / tl) }
}

const ZOOM_MIN = 0.2
const ZOOM_MAX = 500

// `view` zoomed by `factor` about the screen point (x, y) in CSS pixels, which stays on the same
// point of the model.
export function zoomAt(view: View, c: Camera, x: number, y: number, factor: number): View {
  const zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, view.zoom * factor))
  const f = zoom / view.zoom
  const u = (x - c.w / 2) / c.scale
  const v = -(y - c.h / 2) / c.scale
  return { ...view, zoom, pan: [view.pan[0] + u * (1 - 1 / f), view.pan[1] + v * (1 - 1 / f)] }
}

// `view` turned by a drag of (dx, dy) CSS pixels at `k` radians a pixel: across, round the
// model's up; up and down, round the screen's level axis, with no stop at the top or bottom.
// Upside down, a drag across still moves the near side the pointer's way.
export function tumble(view: View, dx: number, dy: number, k: number): View {
  const flip = Math.cos(view.pitch) < 0 ? -1 : 1
  return { ...view, yaw: view.yaw - dx * k * flip, pitch: view.pitch + dy * k }
}

// `view` moved so the model follows a drag of (dx, dy) CSS pixels.
export function panBy(view: View, c: Camera, dx: number, dy: number): View {
  return { ...view, pan: [view.pan[0] - dx / c.scale, view.pan[1] + dy / c.scale] }
}

export interface Camera {
  center: Vec3
  scale: number
  right: Vec3
  up: Vec3
  toward: Vec3
  w: number
  h: number
  // Half the frame's diagonal: no point of the frame lies farther from its centre.
  radius: number
}

export function camera(view: View, frame: { lo: Vec3; hi: Vec3 }, w: number, h: number): Camera {
  const { yaw, pitch } = view
  const cp = Math.cos(pitch)
  const sp = Math.sin(pitch)
  const right: Vec3 = [-Math.sin(yaw), Math.cos(yaw), 0]
  const up: Vec3 = [-sp * Math.cos(yaw), -sp * Math.sin(yaw), cp]
  const [px, py] = view.pan
  // Moved across the screen only, so depth toward the viewer is the frame's at any pan.
  const center = [0, 1, 2].map(
    k => (frame.lo[k]! + frame.hi[k]!) / 2 + px * right[k]! + py * up[k]!,
  ) as Vec3
  const radius = Math.max(Math.hypot(...[0, 1, 2].map(k => frame.hi[k]! - frame.lo[k]!)) / 2, 1e-9)
  return {
    center,
    scale: (0.44 * Math.min(w, h) * view.zoom) / radius,
    right,
    up,
    toward: [cp * Math.cos(yaw), cp * Math.sin(yaw), sp],
    w,
    h,
    radius,
  }
}

// Screen x, y in CSS pixels, and depth toward the viewer in model units.
export function project(c: Camera, x: number, y: number, z: number): [number, number, number] {
  const d = [x - c.center[0], y - c.center[1], z - c.center[2]]
  const dot = (v: Vec3) => d[0]! * v[0] + d[1]! * v[1] + d[2]! * v[2]
  return [c.w / 2 + dot(c.right) * c.scale, c.h / 2 - dot(c.up) * c.scale, dot(c.toward)]
}

interface Colors {
  edge: Rgb
  face: Rgb
  faceAlt: Rgb
  ghost: [Rgb, Rgb]
  fail: string
  // The overlay's dots; its box, plane and lines; and its points outside or with no answer.
  dot: Rgb
  guide: Rgb
  quiet: Rgb
  // A curvature tick where the surface bends inward.
  concave: Rgb
  focus: string
  annot: string
}

// Any CSS colour as its bytes, by painting it.
function rgbOf(color: string): Rgb {
  const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true })!
  ctx.fillStyle = color
  ctx.fillRect(0, 0, 1, 1)
  const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data
  return [r!, g!, b!]
}

function colorsOf(el: Element): Colors {
  const s = getComputedStyle(el)
  const v = (name: string, fallback: string) => s.getPropertyValue(name).trim() || fallback
  return {
    edge: rgbOf(v('--scan-edge', '#000000')),
    face: rgbOf(v('--scan-face', '#8a8c8e')),
    faceAlt: rgbOf(v('--scan-face-alt', '#7f97a8')),
    ghost: [rgbOf(v('--scan-ghost', '#4fb3ff')), rgbOf(v('--scan-ghost2', '#f5a200'))],
    fail: v('--scan-fail', '#ff5a33'),
    dot: rgbOf(v('--scan-dot', '#f5a200')),
    guide: rgbOf(v('--scan-guide', '#a2a4a6')),
    quiet: rgbOf(v('--scan-quiet', '#a2a4a6')),
    concave: rgbOf(v('--scan-concave', '#b98cff')),
    focus: v('--scan-focus', '#f5a200'),
    annot: v('--scan-annot', '#6a6c6e'),
  }
}

function grow(b: { lo: Vec3; hi: Vec3 }, xs: ArrayLike<number>) {
  for (let i = 0; i + 2 < xs.length; i += 3)
    for (let k = 0; k < 3; k++) {
      b.lo[k] = Math.min(b.lo[k]!, xs[i + k]!)
      b.hi[k] = Math.max(b.hi[k]!, xs[i + k]!)
    }
}

// Every place the overlay marks: its points and the answers drawn at them, the box's corners, a
// point of the plane, and the drawing's lines.
export function overlayAt(o: Overlay): Vec3[] {
  const at: Vec3[] = [...o.points, ...(o.box ?? [])]
  if (o.plane) at.push(o.plane.origin)
  for (const l of o.lines) at.push(...l.pts)
  for (const c of o.closest) at.push(c.at, ...(c.foot ? [c.foot] : []))
  for (const s of o.sides) at.push(s.at)
  for (const n of o.normals) at.push(n.at)
  for (const c of o.curvature) at.push(c.foot)
  return at
}

// The frame every view of a scene is fitted to: the shape, its ghosts, and the marks with it.
export function frameOf(scene: Scene): { lo: Vec3; hi: Vec3 } {
  if (scene.frame) return scene.frame
  const b = {
    lo: [Infinity, Infinity, Infinity] as Vec3,
    hi: [-Infinity, -Infinity, -Infinity] as Vec3,
  }
  for (const s of [scene, scene.alt, ...scene.ghosts]) {
    grow(b, s.positions)
    grow(b, s.edges)
  }
  for (const m of scene.marks) grow(b, m.p)
  if (!Number.isFinite(b.lo[0])) return { lo: [-1, -1, -1], hi: [1, 1, 1] }
  for (let k = 0; k < 3; k++) {
    if (b.hi[k]! - b.lo[k]! < 1e-6) {
      b.lo[k]! -= 1
      b.hi[k]! += 1
    }
  }
  return b
}

// The marks, the focus reticle and the axes over the GPU's drawing.
function overlay(
  ctx: CanvasRenderingContext2D,
  scene: Scene,
  c: Camera,
  col: Colors,
  focus: number,
) {
  ctx.clearRect(0, 0, c.w, c.h)

  ctx.lineWidth = 1.5
  scene.marks.forEach((m, i) => {
    const [x, y] = project(c, ...m.p)
    ctx.strokeStyle = col.fail
    ctx.fillStyle = col.fail
    if (m.held) ctx.fillRect(x - 3.5, y - 3.5, 7, 7)
    else ctx.strokeRect(x - 3.25, y - 3.25, 6.5, 6.5)
    if (i === focus) reticle(ctx, x, y, col.focus)
  })
  triad(ctx, c, col.annot)
}

// How far past the solid's box a plane is drawn, as a share of the box's size each way.
const PLANE_MARGIN = 0.08
// A hidden stretch's dash, and the gap after it; a normal's arrow: each as a share of the solid's
// size, so they read the same on a small part and a large one.
const DASH = 0.012
const ARROW = 0.08
// A point's ball, its radius as a share of the solid's size.
const BALL = 0.011
// How far a classify probe's inside and outside points are drawn from its point on the surface, as a
// share of the solid's size; and how near, in millimetres, they lie in fact.
const PROBE = 0.035
const PROBE_NEAR = 0.05
// Half a curvature tick's length, as a share of the solid's size.
const TICK = 0.05

// `ends` as dashes along the polyline `pts`, each `dash` long with as long a gap after it.
function dashed(pts: readonly Vec3[], dash: number, ends: number[]) {
  let on = true
  let left = dash
  for (let i = 0; i + 1 < pts.length; i++) {
    const [a, b] = [pts[i]!, pts[i + 1]!]
    const len = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2])
    let t = 0
    while (t < len) {
      const step = Math.min(left, len - t)
      if (on) {
        const at = (s: number) => [0, 1, 2].map(k => a[k]! + ((b[k]! - a[k]!) * s) / len)
        ends.push(...at(t), ...at(t + step))
      }
      t += step
      left -= step
      if (left <= 0) {
        on = !on
        left = dash
      }
    }
  }
}

// The case's overlay as the GPU draws it: a plane as the polygon it cuts from the box `within`
// (the solid's, a little grown), which reaches past the solid on every side as an endless plane
// does without filling the view; a box as its twelve edges; the points as they are.
export function overlayGeometry(
  o: Overlay | undefined,
  within: { lo: Vec3; hi: Vec3 },
): OverlayGeometry {
  const none = new Float32Array(0)
  if (!o) {
    return {
      plane: none,
      planeEdges: none,
      box: none,
      points: none,
      lines: none,
      hiddenLines: none,
      sticks: none,
      seen: none,
      convex: none,
      concave: none,
      flat: none,
      arrows: none,
      dashes: none,
      inside: none,
      quiet: none,
      ball: 0,
    }
  }
  let plane = none
  let planeEdges = none
  if (o.plane) {
    const { origin, normal: n } = o.plane
    const grow = [0, 1, 2].map(k => PLANE_MARGIN * (within.hi[k]! - within.lo[k]!))
    const lo = within.lo.map((x, k) => x - grow[k]!)
    const hi = within.hi.map((x, k) => x + grow[k]!)
    const corner = (i: number) => [
      i & 1 ? hi[0]! : lo[0]!,
      i & 2 ? hi[1]! : lo[1]!,
      i & 4 ? hi[2]! : lo[2]!,
    ]
    const side = (p: number[]) => [0, 1, 2].reduce((s, k) => s + n[k]! * (p[k]! - origin[k]!), 0)
    // Where the plane crosses each of the box's edges.
    const cut: number[][] = []
    for (let i = 0; i < 8; i++)
      for (const bit of [1, 2, 4]) {
        if (i & bit) continue
        const [a, b] = [corner(i), corner(i | bit)]
        const [da, db] = [side(a), side(b)]
        if (da < 0 === db < 0 || da === db) continue
        const t = da / (da - db)
        cut.push([0, 1, 2].map(k => a[k]! + t * (b[k]! - a[k]!)))
      }
    if (cut.length >= 3) {
      // In order round their centre, by their angle in the plane.
      const c = [0, 1, 2].map(k => cut.reduce((s, p) => s + p[k]!, 0) / cut.length)
      const len = Math.hypot(...n) || 1
      const z = n.map(x => x / len)
      const seed = Math.abs(z[0]!) < 0.9 ? [1, 0, 0] : [0, 1, 0]
      const dz = seed.reduce((s, x, k) => s + x * z[k]!, 0)
      const u = seed.map((x, k) => x - dz * z[k]!)
      const v = [
        z[1]! * u[2]! - z[2]! * u[1]!,
        z[2]! * u[0]! - z[0]! * u[2]!,
        z[0]! * u[1]! - z[1]! * u[0]!,
      ]
      const angle = (p: number[]) => {
        const d = p.map((x, k) => x - c[k]!)
        return Math.atan2(
          d.reduce((s, x, k) => s + x * v[k]!, 0),
          d.reduce((s, x, k) => s + x * u[k]!, 0),
        )
      }
      cut.sort((p, q) => angle(p) - angle(q))
      const ring = cut.map((p, i) => [p, cut[(i + 1) % cut.length]!] as const)
      plane = Float32Array.from(ring.flatMap(([p, q]) => [...c, ...p, ...q]))
      planeEdges = Float32Array.from(ring.flatMap(([p, q]) => [...p, ...q]))
    }
  }
  let box = none
  if (o.box) {
    const [lo, hi] = o.box
    const at = (i: number) => [i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]]
    const ends: number[] = []
    for (let i = 0; i < 8; i++)
      for (const bit of [1, 2, 4]) if (!(i & bit)) ends.push(...at(i), ...at(i | bit))
    box = Float32Array.from(ends)
  }
  const size = Math.hypot(...[0, 1, 2].map(k => within.hi[k]! - within.lo[k]!))
  // Amber: the points asked about, a point on the boundary. Blue: a nearest point, a point inside.
  // Grey: a point outside, a point with no normal.
  const points: number[] = o.points.flat()
  const inside: number[] = []
  const quiet: number[] = []
  const lines: number[] = []
  const arrows: number[] = []
  const sticks: number[] = []
  // A drawing's seen stretches: it names its hidden ones, so these get no dashed copy.
  const seen: number[] = []
  const dashes: number[] = []
  for (const l of o.lines) {
    if (l.seen) for (let i = 0; i + 1 < l.pts.length; i++) seen.push(...l.pts[i]!, ...l.pts[i + 1]!)
    else dashed(l.pts, DASH * size, dashes)
  }
  for (const c of o.closest) {
    points.push(...c.at)
    if (c.foot) {
      inside.push(...c.foot)
      lines.push(...c.at, ...c.foot)
    }
  }
  // A classify probe is a point on the surface with points a hair inside and outside it, too close
  // to tell apart; they are spread along the line through the three, and joined by a stick.
  const near = (p: Vec3, q: Vec3) => Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]) < PROBE_NEAR
  const used = new Set<number>()
  o.sides.forEach((s, i) => {
    if (s.side !== 'on') return
    const pick = (side: Side) =>
      o.sides.findIndex((t, j) => t.side === side && !used.has(j) && near(t.at, s.at))
    const [ii, oi] = [pick('in'), pick('out')]
    const [inner, outer] = [o.sides[ii]?.at, o.sides[oi]?.at]
    const ends =
      inner && outer ? [inner, outer] : inner ? [inner, s.at] : outer ? [s.at, outer] : null
    if (!ends) return
    const d = [0, 1, 2].map(k => ends[1]![k]! - ends[0]![k]!)
    const len = Math.hypot(...d)
    if (!len) return
    const spread = (sign: number) =>
      [0, 1, 2].map(k => s.at[k]! + (sign * PROBE * size * d[k]!) / len)
    used.add(i)
    points.push(...s.at)
    if (inner) {
      used.add(ii)
      inside.push(...spread(-1))
    }
    if (outer) {
      used.add(oi)
      quiet.push(...spread(1))
    }
    sticks.push(...(inner ? spread(-1) : s.at), ...(outer ? spread(1) : s.at))
  })
  o.sides.forEach((s, i) => {
    if (!used.has(i)) (s.side === 'in' ? inside : s.side === 'on' ? points : quiet).push(...s.at)
  })
  for (const { at, n } of o.normals) {
    const len = n ? Math.hypot(...n) : 0
    if (!n || !len) {
      quiet.push(...at)
      continue
    }
    arrows.push(...at, ...[0, 1, 2].map(k => at[k]! + (n[k]! / len) * ARROW * size))
  }
  // At each foot, a tick along each principal direction, sorted by how the surface bends that way:
  // outward, inward, or not at all; and a stick to the foot from the point asked about.
  const convex: number[] = []
  const concave: number[] = []
  const flat: number[] = []
  o.curvature.forEach((c, i) => {
    const { foot: f, n, d1 } = c
    const d2: Vec3 = [
      n[1] * d1[2] - n[2] * d1[1],
      n[2] * d1[0] - n[0] * d1[2],
      n[0] * d1[1] - n[1] * d1[0],
    ]
    for (const [d, k] of [
      [d1, c.k[0]],
      [d2, c.k[1]],
    ] as const) {
      const half = d.map(x => x * TICK * size)
      const into = Math.abs(k) < 1e-9 ? flat : k > 0 ? convex : concave
      into.push(...[0, 1, 2].map(j => f[j]! - half[j]!), ...[0, 1, 2].map(j => f[j]! + half[j]!))
    }
    const q = o.points[i]
    if (q) sticks.push(...q, ...f)
  })
  // Pairs of points again as dashes, for where the solid hides them.
  const dashesOf = (ends: number[]) => {
    const out: number[] = []
    for (let i = 0; i + 5 < ends.length; i += 6) {
      const pair: Vec3[] = [
        [ends[i]!, ends[i + 1]!, ends[i + 2]!],
        [ends[i + 3]!, ends[i + 4]!, ends[i + 5]!],
      ]
      dashed(pair, DASH * size, out)
    }
    return Float32Array.from(out)
  }
  return {
    plane,
    planeEdges,
    box,
    points: Float32Array.from(points),
    lines: Float32Array.from(lines),
    hiddenLines: dashesOf(lines),
    sticks: Float32Array.from(sticks),
    seen: Float32Array.from(seen),
    convex: Float32Array.from(convex),
    concave: Float32Array.from(concave),
    flat: Float32Array.from(flat),
    arrows: Float32Array.from(arrows),
    dashes: Float32Array.from(dashes),
    inside: Float32Array.from(inside),
    quiet: Float32Array.from(quiet),
    ball: BALL * size,
  }
}

// The lock-on reticle: four corner brackets round the focused point.
function reticle(ctx: CanvasRenderingContext2D, x: number, y: number, color: string) {
  const r = 11
  const l = 5
  ctx.strokeStyle = color
  ctx.lineWidth = 1
  ctx.beginPath()
  for (const [sx, sy] of [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ] as const) {
    ctx.moveTo(x + sx * r, y + sy * (r - l))
    ctx.lineTo(x + sx * r, y + sy * r)
    ctx.lineTo(x + sx * (r - l), y + sy * r)
  }
  ctx.stroke()
}

function triad(ctx: CanvasRenderingContext2D, c: Camera, color: string) {
  const ox = c.w - 34
  const oy = c.h - 30
  ctx.strokeStyle = color
  ctx.fillStyle = color
  ctx.lineWidth = 1
  ctx.font = '9px ui-monospace, monospace'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  const axes: [string, Vec3][] = [
    ['x', [1, 0, 0]],
    ['y', [0, 1, 0]],
    ['z', [0, 0, 1]],
  ]
  for (const [name, v] of axes) {
    const dx = v[0] * c.right[0] + v[1] * c.right[1] + v[2] * c.right[2]
    const dy = v[0] * c.up[0] + v[1] * c.up[1] + v[2] * c.up[2]
    ctx.beginPath()
    ctx.moveTo(ox, oy)
    ctx.lineTo(ox + dx * 16, oy - dy * 16)
    ctx.stroke()
    ctx.fillText(name, ox + dx * 24, oy - dy * 24)
  }
}

export interface ScanHandle {
  set(scene: Scene): void
  focus(i: number | null): void
  reset(): void
  destroy(): void
}

// Draws `scene` into `surface` (WebGL2) with its marks on `canvas`, and keeps it drawn: on
// resize, on drag, and on each change. Throws where the browser has no WebGL2.
export function mountScan(
  canvas: HTMLCanvasElement,
  surface: HTMLCanvasElement,
  initial: Scene,
  onPick: (i: number | null) => void,
): ScanHandle {
  const ctx = canvas.getContext('2d')!
  const renderer = new Renderer(surface)
  let scene = initial
  let frame = frameOf(scene)
  let view = homeOf(scene.overlay)
  let focused = -1
  let w = 0
  let h = 0
  let queued = 0
  const colors = colorsOf(canvas)
  renderer.set(scene, scene.alt, scene.ghosts)
  renderer.setOverlay(overlayGeometry(scene.overlay, frame))

  const render = () => {
    queued = 0
    const dpr = window.devicePixelRatio || 1
    w = canvas.clientWidth
    h = canvas.clientHeight
    if (!w || !h) return
    for (const c of [canvas, surface]) {
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
        c.width = Math.round(w * dpr)
        c.height = Math.round(h * dpr)
      }
    }
    const c = camera(view, frame, w, h)
    renderer.draw(
      {
        center: c.center,
        right: c.right,
        up: c.up,
        toward: c.toward,
        scale: c.scale * dpr,
        w: surface.width,
        h: surface.height,
        reach: c.radius * 1.01,
        dpr,
      },
      {
        face: colors.face,
        faceAlt: colors.faceAlt,
        edge: colors.edge,
        ghost: colors.ghost,
        guide: colors.guide,
        dot: colors.dot,
        quiet: colors.quiet,
        concave: colors.concave,
      },
    )
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    overlay(ctx, scene, c, colors, focused)
  }
  const redraw = () => {
    if (!queued) queued = requestAnimationFrame(render)
  }

  // A left drag tumbles; a right drag, or a left one with ⇧, ⌃ or ⌘ or with Space held, pans; a
  // middle drag zooms about the view's centre, as do a pinch and a scroll with ⌘ or Ctrl (see
  // `wheel`). On touch, one finger tumbles and two pinch to zoom and move together to pan.
  type Mode = 'tumble' | 'pan' | 'zoom'
  const pointers = new Map<number, { x: number; y: number }>()
  let drag: { mode: Mode; x: number; y: number; moved: boolean } | null = null
  let pinch: { mid: [number, number]; dist: number } | null = null
  let spaceHeld = false
  const cam = () => camera(view, frame, w, h)
  const pinchOf = (): { mid: [number, number]; dist: number } => {
    const [a, b] = [...pointers.values()] as [{ x: number; y: number }, { x: number; y: number }]
    return { mid: [(a.x + b.x) / 2, (a.y + b.y) / 2], dist: Math.hypot(a.x - b.x, a.y - b.y) }
  }
  const down = (e: PointerEvent) => {
    canvas.setPointerCapture(e.pointerId)
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })
    if (pointers.size === 2) {
      drag = null
      pinch = pinchOf()
      return
    }
    const pans =
      e.button === 2 || (e.button === 0 && (spaceHeld || e.shiftKey || e.ctrlKey || e.metaKey))
    const mode: Mode = e.button === 1 ? 'zoom' : pans ? 'pan' : 'tumble'
    drag = { mode, x: e.clientX, y: e.clientY, moved: false }
  }
  const move = (e: PointerEvent) => {
    if (!pointers.has(e.pointerId)) return
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })
    if (pinch && pointers.size === 2) {
      const now = pinchOf()
      view = panBy(view, cam(), now.mid[0] - pinch.mid[0], now.mid[1] - pinch.mid[1])
      if (pinch.dist > 0) view = zoomAt(view, cam(), w / 2, h / 2, now.dist / pinch.dist)
      pinch = now
      return redraw()
    }
    if (!drag) return
    const dx = e.clientX - drag.x
    const dy = e.clientY - drag.y
    drag.x = e.clientX
    drag.y = e.clientY
    if (Math.abs(dx) + Math.abs(dy) > 0) drag.moved = true
    if (drag.mode === 'pan') view = panBy(view, cam(), dx, dy)
    else if (drag.mode === 'zoom') view = zoomAt(view, cam(), w / 2, h / 2, Math.exp(-dy * 0.01))
    else view = tumble(view, dx, dy, (2 * Math.PI) / (h || 1))
    redraw()
  }
  const up = (e: PointerEvent) => {
    pointers.delete(e.pointerId)
    if (pointers.size < 2) pinch = null
    if (drag && !drag.moved && drag.mode === 'tumble') {
      const r = canvas.getBoundingClientRect()
      const c = camera(view, frame, w, h)
      let best = -1
      let bestD = 12
      scene.marks.forEach((m, i) => {
        const [x, y] = project(c, ...m.p)
        const d = Math.hypot(x - (e.clientX - r.left), y - (e.clientY - r.top))
        if (d < bestD) [best, bestD] = [i, d]
      })
      onPick(best < 0 ? null : best)
    }
    drag = null
  }
  const key = (e: KeyboardEvent) => {
    const step = 0.12
    const turns: Record<string, [number, number]> = {
      ArrowLeft: [step, 0],
      ArrowRight: [-step, 0],
      ArrowUp: [0, step],
      ArrowDown: [0, -step],
    }
    const t = turns[e.key]
    if (!t) return
    e.preventDefault()
    view = { ...view, yaw: view.yaw + t[0], pitch: view.pitch + t[1] }
    redraw()
  }
  // The view sits in a page, so a plain scroll scrolls the page. ⌘ or Ctrl with it zooms, and so
  // does a trackpad's pinch, which Chrome and Firefox send as a wheel with Ctrl held.
  const wheel = (e: WheelEvent) => {
    if (!e.ctrlKey && !e.metaKey) return
    e.preventDefault()
    const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY
    view = zoomAt(view, cam(), w / 2, h / 2, Math.exp(-dy * 0.01))
    redraw()
  }
  // Safari sends a trackpad's pinch as gesture events, each with the scale since it began.
  let gestureScale = 1
  const gesture = (e: Event) => {
    e.preventDefault()
    const scale = (e as Event & { scale?: number }).scale ?? 1
    if (e.type === 'gesturestart') gestureScale = 1
    else if (scale > 0) {
      view = zoomAt(view, cam(), w / 2, h / 2, scale / gestureScale)
      gestureScale = scale
      redraw()
    }
  }
  // Space pans only while the view has the focus, so elsewhere it still scrolls the page.
  const space = (e: KeyboardEvent) => {
    if (e.code !== 'Space') return
    if (e.type === 'keydown' && e.target !== canvas) return
    if (e.type === 'keydown') e.preventDefault()
    spaceHeld = e.type === 'keydown'
  }
  const blur = () => (spaceHeld = false)
  const menu = (e: Event) => e.preventDefault()
  canvas.addEventListener('pointerdown', down)
  canvas.addEventListener('pointermove', move)
  canvas.addEventListener('pointerup', up)
  canvas.addEventListener('pointercancel', up)
  canvas.addEventListener('keydown', key)
  canvas.addEventListener('wheel', wheel, { passive: false })
  canvas.addEventListener('contextmenu', menu)
  for (const g of ['gesturestart', 'gesturechange']) canvas.addEventListener(g, gesture)
  window.addEventListener('keydown', space)
  window.addEventListener('keyup', space)
  window.addEventListener('blur', blur)
  const ro = new ResizeObserver(redraw)
  ro.observe(canvas)
  render()

  return {
    set(next) {
      // A case with another home, such as another plane or a drawing seen from elsewhere, opens on
      // it; one with the same home keeps the reader's view.
      const home = homeOf(next.overlay)
      if (JSON.stringify(home) !== JSON.stringify(homeOf(scene.overlay))) view = home
      scene = next
      frame = frameOf(scene)
      focused = -1
      renderer.set(scene, scene.alt, scene.ghosts)
      renderer.setOverlay(overlayGeometry(scene.overlay, frame))
      redraw()
    },
    focus(i) {
      focused = i ?? -1
      redraw()
    },
    reset() {
      view = homeOf(scene.overlay)
      redraw()
    },
    destroy() {
      ro.disconnect()
      cancelAnimationFrame(queued)
      canvas.removeEventListener('pointerdown', down)
      canvas.removeEventListener('pointermove', move)
      canvas.removeEventListener('pointerup', up)
      canvas.removeEventListener('pointercancel', up)
      canvas.removeEventListener('keydown', key)
      canvas.removeEventListener('wheel', wheel)
      canvas.removeEventListener('contextmenu', menu)
      for (const g of ['gesturestart', 'gesturechange']) canvas.removeEventListener(g, gesture)
      window.removeEventListener('keydown', space)
      window.removeEventListener('keyup', space)
      window.removeEventListener('blur', blur)
      renderer.destroy()
    },
  }
}
