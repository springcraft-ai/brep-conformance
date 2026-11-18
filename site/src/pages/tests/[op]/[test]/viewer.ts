// Everything the Scan view needs in the browser, loaded as one chunk after the page hydrates.
import { parseGlb, type Shape } from './glb'

export { mountScan, type Mark, type Scene, type ScanHandle } from './scan'

export interface Loaded {
  shape: Shape
  triangles: number
}

const cache = new Map<string, Promise<Loaded>>()

// The expected shape from its GLB. Rejects when the file is not there.
export function loadShape(url: string): Promise<Loaded> {
  let p = cache.get(url)
  if (!p) {
    p = fetch(url, { mode: 'cors' })
      .then(r => {
        if (!r.ok) throw new Error(`${r.status} ${r.statusText}`)
        return r.arrayBuffer()
      })
      .then(buf => {
        const shape = parseGlb(buf)
        return { shape, triangles: shape.triangles.length / 3 }
      })
    p.catch(() => cache.delete(url))
    cache.set(url, p)
  }
  return p
}
