// The Scan view: the shape shaded by mesh.ts on a WebGL2 canvas, and over it, as HTML, a kernel's
// misplaced points, the focused point's reticle, and the axes. Orthographic, z up, turned by
// dragging. The page ships no 3D library.
import { createMeshLayer, type Mesh } from './mesh'
import { bounds, type Vec3 } from './vec'

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

// The canvas's CSS colour is the metal's own, --scan-face, read as bytes.
function faceColor(el: Element): [number, number, number] {
  const m = getComputedStyle(el).color.match(/[\d.]+/g)
  return m && m.length >= 3 ? [Number(m[0]), Number(m[1]), Number(m[2])] : [184, 194, 204]
}

const SVG = 'http://www.w3.org/2000/svg'
const AXES: [string, Vec3][] = [
  ['x', [1, 0, 0]],
  ['y', [0, 1, 0]],
  ['z', [0, 0, 1]],
]

function triad(): { el: SVGSVGElement; place(c: Camera): void } {
  const el = document.createElementNS(SVG, 'svg')
  el.setAttribute('class', 'bc-scan__triad')
  el.setAttribute('viewBox', '-30 -30 60 60')
  el.setAttribute('aria-hidden', 'true')
  const parts = AXES.map(([name]) => {
    const line = document.createElementNS(SVG, 'line')
    const text = document.createElementNS(SVG, 'text')
    text.textContent = name
    el.append(line, text)
    return { line, text }
  })
  return {
    el,
    place(c) {
      AXES.forEach(([, v], i) => {
        const dx = v[0] * c.right[0] + v[1] * c.right[1] + v[2] * c.right[2]
        const dy = v[0] * c.up[0] + v[1] * c.up[1] + v[2] * c.up[2]
        const { line, text } = parts[i]!
        line.setAttribute('x2', String(dx * 16))
        line.setAttribute('y2', String(-dy * 16))
        text.setAttribute('x', String(dx * 24))
        text.setAttribute('y', String(-dy * 24))
      })
    },
  }
}

function markButtons(marks: Mark[], onPick: (i: number) => void): HTMLButtonElement[] {
  return marks.map((m, i) => {
    const b = document.createElement('button')
    b.type = 'button'
    b.className = 'bc-scan__mark sc-focus'
    if (m.held) b.dataset.held = ''
    b.setAttribute('aria-pressed', 'false')
    b.setAttribute(
      'aria-label',
      `point ${i + 1}: ${m.held ? 'placed inside, belongs outside' : 'placed outside, belongs inside'}`,
    )
    b.addEventListener('click', () => onPick(i))
    return b
  })
}

export interface ScanHandle {
  // False where the browser has no WebGL2: the points and axes are drawn, the shape is not.
  readonly gl: boolean
  set(scene: Scene): void
  focus(i: number | null): void
  reset(): void
  destroy(): void
}

// Draws `scene`'s shape into `canvas` and its points and axes into `overlay`, and keeps them drawn:
// on resize, on drag, and on each change.
export function mountScan(
  canvas: HTMLCanvasElement,
  overlay: HTMLElement,
  initial: Scene,
  onPick: (i: number | null) => void,
): ScanHandle {
  let scene = initial
  let frame = frameOf(scene)
  let view = { ...HOME }
  let focused = -1
  let queued = 0

  const render = () => {
    queued = 0
    const w = canvas.clientWidth
    const h = canvas.clientHeight
    if (!w || !h) return
    const c = camera(view, frame, w, h)
    layer?.draw(c, window.devicePixelRatio || 1)
    scene.marks.forEach((m, i) => {
      const [x, y] = project(c, ...m.p)
      buttons[i]!.style.translate = `${x}px ${y}px`
    })
    axes.place(c)
  }
  const redraw = () => {
    if (!queued) queued = requestAnimationFrame(render)
  }

  const layer = createMeshLayer(canvas, faceColor(canvas), redraw)
  layer?.set(scene)
  const marks = document.createElement('div')
  marks.className = 'bc-scan__marks'
  const axes = triad()
  overlay.replaceChildren(marks, axes.el)
  let buttons = markButtons(scene.marks, i => onPick(i))
  marks.replaceChildren(...buttons)

  const showFocus = () =>
    buttons.forEach((b, i) => {
      b.classList.toggle('is-focus', i === focused)
      b.setAttribute('aria-pressed', String(i === focused))
    })

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
  // A tap on the shape, not a drag, lets go of the focused point; a point is picked by its button.
  const up = () => {
    if (drag && !drag.moved) onPick(null)
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
    gl: !!layer,
    set(next) {
      if (next.positions !== scene.positions || next.triangles !== scene.triangles) layer?.set(next)
      if (next.marks !== scene.marks) {
        buttons = markButtons(next.marks, i => onPick(i))
        marks.replaceChildren(...buttons)
      }
      scene = next
      frame = frameOf(scene)
      focused = -1
      showFocus()
      redraw()
    },
    focus(i) {
      focused = i ?? -1
      showFocus()
    },
    reset() {
      view = { ...HOME }
      redraw()
    },
    destroy() {
      layer?.destroy()
      ro.disconnect()
      cancelAnimationFrame(queued)
      overlay.replaceChildren()
      canvas.removeEventListener('pointerdown', down)
      canvas.removeEventListener('pointermove', move)
      canvas.removeEventListener('pointerup', up)
      canvas.removeEventListener('keydown', key)
    },
  }
}
