// The Scan view: the shape as a shaded solid, and a kernel's misplaced points over it.
// Orthographic, z up, turned by dragging. Canvas 2D over solid.ts's rasterizer: the suite's meshes
// redraw well inside a frame, and the page ships no 3D library.
import { bounds, type Vec3 } from './vec'
import { raster, rasterize, studio, type Mesh, type Raster, type Shade } from './solid'

export interface Mark {
  p: Vec3
  // Where the kernel placed the point: inside the result, or not.
  held: boolean
}

export interface Scene extends Mesh {
  edges: Float32Array
  // Smooth edges with their faces' normals, as glb.ts's topology gives them.
  smooth: Float32Array
  marks: Mark[]
}

export interface View {
  yaw: number
  pitch: number
}

export const HOME: View = { yaw: -Math.PI / 4 - 0.35, pitch: 0.5 }

export interface Camera {
  center: Vec3
  scale: number
  right: Vec3
  up: Vec3
  toward: Vec3
  w: number
  h: number
}

export function camera(view: View, frame: { lo: Vec3; hi: Vec3 }, w: number, h: number): Camera {
  const { yaw, pitch } = view
  const cp = Math.cos(pitch)
  const sp = Math.sin(pitch)
  const center = [0, 1, 2].map(k => (frame.lo[k]! + frame.hi[k]!) / 2) as Vec3
  const radius = Math.max(Math.hypot(...[0, 1, 2].map(k => frame.hi[k]! - frame.lo[k]!)) / 2, 1e-9)
  return {
    center,
    scale: (0.44 * Math.min(w, h)) / radius,
    right: [-Math.sin(yaw), Math.cos(yaw), 0],
    up: [-sp * Math.cos(yaw), -sp * Math.sin(yaw), cp],
    toward: [cp * Math.cos(yaw), cp * Math.sin(yaw), sp],
    w,
    h,
  }
}

// Screen x, y in CSS pixels, and depth toward the viewer in model units.
export function project(c: Camera, x: number, y: number, z: number): [number, number, number] {
  const d = [x - c.center[0], y - c.center[1], z - c.center[2]]
  const dot = (v: Vec3) => d[0]! * v[0] + d[1]! * v[1] + d[2]! * v[2]
  return [c.w / 2 + dot(c.right) * c.scale, c.h / 2 - dot(c.up) * c.scale, dot(c.toward)]
}

export interface Colors {
  face: Shade
  fail: string
  focus: string
  annot: string
}

// Any CSS colour as the bytes the rasterizer writes, by painting it.
function rgbOf(color: string): [number, number, number] {
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
    face: studio(rgbOf(v('--scan-face', '#b8c2cc'))),
    fail: v('--scan-fail', '#ff5a33'),
    focus: v('--scan-focus', '#f5a200'),
    annot: v('--scan-annot', '#6a6c6e'),
  }
}

// The frame every view of a scene is fitted to: the shape, and the marks with it.
export function frameOf(scene: Scene): { lo: Vec3; hi: Vec3 } {
  const ends: number[] = []
  for (let i = 0; i + 11 < scene.smooth.length; i += 12)
    for (let k = 0; k < 6; k++) ends.push(scene.smooth[i + k]!)
  const all = [...scene.positions, ...scene.edges, ...ends, ...scene.marks.flatMap(m => m.p)]
  const b = bounds(all) ?? { lo: [-1, -1, -1], hi: [1, 1, 1] }
  for (let k = 0; k < 3; k++) {
    if (b.hi[k]! - b.lo[k]! < 1e-6) {
      b.lo[k]! -= 1
      b.hi[k]! += 1
    }
  }
  return b
}

function draw(
  ctx: CanvasRenderingContext2D,
  scene: Scene,
  c: Camera,
  col: Colors,
  focus: number,
  r: Raster,
) {
  ctx.clearRect(0, 0, c.w, c.h)
  if (!scene.triangles.length && !scene.edges.length && !scene.marks.length) return
  // The raster is in device pixels, the camera in CSS pixels.
  const k = r.w / c.w
  rasterize(
    r,
    scene,
    (x, y, z) => {
      const p = project(c, x, y, z)
      return [p[0] * k, p[1] * k, p[2]]
    },
    [c.right, c.up, c.toward],
    col.face,
  )
  ctx.putImageData(new ImageData(r.rgba as Uint8ClampedArray<ArrayBuffer>, r.w, r.h), 0, 0)

  ctx.globalAlpha = 1
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

// Draws `scene` into `canvas` and keeps it drawn: on resize, on drag, and on each change.
export function mountScan(
  canvas: HTMLCanvasElement,
  initial: Scene,
  onPick: (i: number | null) => void,
): ScanHandle {
  const ctx = canvas.getContext('2d')!
  let scene = initial
  let frame = frameOf(scene)
  let view = { ...HOME }
  let focused = -1
  let w = 0
  let h = 0
  let queued = 0
  let buffer = raster(0, 0)
  const colors = colorsOf(canvas)

  const render = () => {
    queued = 0
    const dpr = window.devicePixelRatio || 1
    w = canvas.clientWidth
    h = canvas.clientHeight
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr)
      canvas.height = Math.round(h * dpr)
    }
    if (!w || !h) return
    if (buffer.w !== canvas.width || buffer.h !== canvas.height)
      buffer = raster(canvas.width, canvas.height)
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    draw(ctx, scene, camera(view, frame, w, h), colors, focused, buffer)
  }
  const redraw = () => {
    if (!queued) queued = requestAnimationFrame(render)
  }

  let drag: { x: number; y: number; view: View; moved: boolean } | null = null
  const down = (e: PointerEvent) => {
    canvas.setPointerCapture(e.pointerId)
    drag = { x: e.clientX, y: e.clientY, view, moved: false }
  }
  const move = (e: PointerEvent) => {
    if (!drag) return
    const dx = e.clientX - drag.x
    const dy = e.clientY - drag.y
    if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true
    view = {
      yaw: drag.view.yaw - dx * 0.01,
      pitch: Math.max(-1.5, Math.min(1.5, drag.view.pitch + dy * 0.01)),
    }
    redraw()
  }
  const up = (e: PointerEvent) => {
    if (drag && !drag.moved) {
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
    view = { yaw: view.yaw + t[0], pitch: Math.max(-1.5, Math.min(1.5, view.pitch + t[1])) }
    redraw()
  }
  canvas.addEventListener('pointerdown', down)
  canvas.addEventListener('pointermove', move)
  canvas.addEventListener('pointerup', up)
  canvas.addEventListener('keydown', key)
  const ro = new ResizeObserver(redraw)
  ro.observe(canvas)
  render()

  return {
    set(next) {
      scene = next
      frame = frameOf(scene)
      focused = -1
      redraw()
    },
    focus(i) {
      focused = i ?? -1
      redraw()
    },
    reset() {
      view = { ...HOME }
      redraw()
    },
    destroy() {
      ro.disconnect()
      cancelAnimationFrame(queued)
      canvas.removeEventListener('pointerdown', down)
      canvas.removeEventListener('pointermove', move)
      canvas.removeEventListener('pointerup', up)
      canvas.removeEventListener('keydown', key)
    },
  }
}
