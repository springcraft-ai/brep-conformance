// Everything the Scan view needs in the browser, loaded as one chunk after the page hydrates. It
// imports nothing the page itself imports: a module both share is bundled into the page's chunk,
// which then has no manifest entry of its own and cannot be prerendered.
import { parseOverlay, parseViews, type GlbView, type Overlay } from './glb'
import { frameOf } from './scan'
import type { Vec3 } from './vec'

export { mountScan, type Ghost, type Mark, type Scene, type ScanHandle } from './scan'

export interface Loaded {
  views: GlbView[]
  overlay: Overlay
  // The box round every view's subjects and the overlay, so turning from one view to the next
  // keeps the scale.
  frame: { lo: Vec3; hi: Vec3 }
}

const cache = new Map<string, Promise<Loaded>>()

// A kernel's drawing of a case, every view of it. Rejects when the file is not there.
export function loadViews(url: string): Promise<Loaded> {
  let p = cache.get(url)
  if (!p) {
    p = fetch(url, { mode: 'cors' })
      .then(r => {
        if (!r.ok) throw new Error(`${r.status} ${r.statusText}`)
        return r.arrayBuffer()
      })
      .then(buf => {
        const views = parseViews(buf)
        const overlay = parseOverlay(buf)
        const at = [...overlay.points, ...(overlay.box ?? [])]
        if (overlay.plane) at.push(overlay.plane.origin)
        const none = new Float32Array(0)
        const solids = views.flatMap(v =>
          v.solids
            .filter(s => s.role !== 'ghost' && s.role !== 'ghost2')
            .map(s => ({ ...s.shape, tone: 0 as const })),
        )
        const frame = frameOf({
          positions: none,
          normals: none,
          triangles: new Uint32Array(0),
          edges: none,
          alt: { positions: none, normals: none, triangles: new Uint32Array(0), edges: none },
          ghosts: solids,
          marks: at.map(p => ({ p, held: false })),
        })
        return { views, overlay, frame }
      })
    p.catch(() => cache.delete(url))
    cache.set(url, p)
  }
  return p
}
