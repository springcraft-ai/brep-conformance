// Everything the Scan view needs in the browser, loaded as one chunk after the page hydrates.
import { parseViews, type GlbView } from './glb'
import { frameOf } from './scan'
import type { Vec3 } from './vec'
import { merge, subjectOf } from './views'

export { mountScan, type Ghost, type Mark, type Scene, type ScanHandle } from './scan'

export interface Loaded {
  views: GlbView[]
  // The box round every view's subjects, so turning from one view to the next keeps the scale.
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
        const all = merge(views.map(subjectOf))
        return { views, frame: frameOf({ ...all, ghosts: [], marks: [] }) }
      })
    p.catch(() => cache.delete(url))
    cache.set(url, p)
  }
  return p
}
