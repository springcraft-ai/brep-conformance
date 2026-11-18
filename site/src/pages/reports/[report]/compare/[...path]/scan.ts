// The Scan view: the shape as a shaded solid with its edges, drawn by WebGL2 (gl.ts), and a
// kernel's misplaced points over it on a 2D canvas, which also takes the pointer. Orthographic,
// z up, turned by dragging.
import { Renderer, type Mesh, type OverlayGeometry, type Rgb } from './gl'
import type { Overlay } from './glb'
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
  // The overlay's dots, and its box and plane.
  dot: Rgb
  guide: Rgb
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

// How far an overlay's plane reaches each way from its origin, as a share of the frame's radius:
// across the solid it cuts, without filling the view.
const PLANE_SIZE = 0.6

const radiusOf = (f: { lo: Vec3; hi: Vec3 }) =>
  Math.hypot(...[0, 1, 2].map(k => f.hi[k]! - f.lo[k]!)) / 2

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

// The case's overlay as the GPU draws it: a plane as a square `size` across each way from its
// origin, a box as its twelve edges, the points as they are.
export function overlayGeometry(o: Overlay | undefined, size: number): OverlayGeometry {
  const none = new Float32Array(0)
  if (!o) return { plane: none, planeEdges: none, box: none, points: none }
  let plane = none
  let planeEdges = none
  if (o.plane) {
    const n = o.plane.normal
    const len = Math.hypot(...n) || 1
    const z = n.map(x => x / len) as Vec3
    const seed: Vec3 = Math.abs(z[0]) < 0.9 ? [1, 0, 0] : [0, 1, 0]
    const d = seed[0] * z[0] + seed[1] * z[1] + seed[2] * z[2]
    const u0 = seed.map((s, k) => s - d * z[k]!)
    const ul = Math.hypot(...u0)
    const u = u0.map(x => x / ul) as Vec3
    const v: Vec3 = [
      z[1] * u[2] - z[2] * u[1],
      z[2] * u[0] - z[0] * u[2],
      z[0] * u[1] - z[1] * u[0],
    ]
    const corner = (a: number, b: number) =>
      [0, 1, 2].map(k => o.plane!.origin[k]! + size * (a * u[k]! + b * v[k]!))
    const [p, q, r, s] = [corner(-1, -1), corner(1, -1), corner(1, 1), corner(-1, 1)]
    plane = Float32Array.from([p, q, r, p, r, s].flat())
    planeEdges = Float32Array.from([p, q, q, r, r, s, s, p].flat())
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
  return { plane, planeEdges, box, points: Float32Array.from(o.points.flat()) }
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
  renderer.setOverlay(overlayGeometry(scene.overlay, PLANE_SIZE * radiusOf(frame)))

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
      // A case about another plane opens on its own view; the same plane keeps the reader's.
      if (JSON.stringify(next.overlay?.plane) !== JSON.stringify(scene.overlay?.plane))
        view = homeOf(next.overlay)
      scene = next
      frame = frameOf(scene)
      focused = -1
      renderer.set(scene, scene.alt, scene.ghosts)
      renderer.setOverlay(overlayGeometry(scene.overlay, PLANE_SIZE * radiusOf(frame)))
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
