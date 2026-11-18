// Which solids a view shows: its subjects, and the operands drawn faint behind them.
import type { GlbView, Shape } from './glb'
import type { Ghost } from './scan'

const EMPTY: Shape = {
  positions: new Float32Array(0),
  normals: new Float32Array(0),
  triangles: new Uint32Array(0),
  edges: new Float32Array(0),
}

// The views whose subjects are drawn faint behind a view, by the suite's view names: an
// intersection stands over both operands, a difference over the operand it takes away, a step's
// result over the solid before it, a move's over the solid unmoved.
export function sourcesOf(name: string): string[] {
  if (name.includes('∩')) return ['a', 'b']
  const cut = /^\s*(\S+)\s*∖\s*(\S+)\s*$/.exec(name)
  if (cut) return [cut[2]!]
  return { after: ['before'], moved: ['solid'], limits: ['input'], cut: ['solid'] }[name] ?? []
}

export function merge(shapes: Shape[]): Shape {
  if (shapes.length === 0) return EMPTY
  if (shapes.length === 1) return shapes[0]!
  const cat = (pick: (s: Shape) => Float32Array) => {
    const out = new Float32Array(shapes.reduce((n, s) => n + pick(s).length, 0))
    let at = 0
    for (const s of shapes) {
      out.set(pick(s), at)
      at += pick(s).length
    }
    return out
  }
  const triangles = new Uint32Array(shapes.reduce((n, s) => n + s.triangles.length, 0))
  let at = 0
  let base = 0
  for (const s of shapes) {
    for (let i = 0; i < s.triangles.length; i++) triangles[at + i] = s.triangles[i]! + base
    at += s.triangles.length
    base += s.positions.length / 3
  }
  return {
    positions: cat(s => s.positions),
    normals: cat(s => s.normals),
    triangles,
    edges: cat(s => s.edges),
  }
}

const isGhost = (role: string) => role === 'ghost' || role === 'ghost2'

export function subjectOf(v: GlbView): Shape {
  return merge(v.solids.filter(s => !isGhost(s.role)).map(s => s.shape))
}

// The ghosts behind view `v`: its operands whole, from the views they are the subjects of. The
// scan view hides what of them lies behind the subject or on its faces.
export function ghostsOf(views: GlbView[], v: GlbView): Ghost[] {
  // Each operand keeps its own tone in every view: `b` is the second, alone or not.
  return sourcesOf(v.name).flatMap(name => {
    const from = views.find(x => x.name === name)
    return from ? [{ ...subjectOf(from), tone: (name === 'b' ? 1 : 0) as 0 | 1 }] : []
  })
}

// What the toggle that draws a view's ghosts says, or null where the view has none.
export function operandsLabel(views: GlbView[], v: GlbView): string | null {
  const sources = sourcesOf(v.name)
  if (!sources.some(n => views.some(x => x.name === n))) return null
  if (v.name.includes('∩')) return 'show operands'
  if (v.name.includes('∖')) return `show operand ${sources[0]}`
  return v.name === 'after' ? 'show starting solid' : 'show original'
}

// The view to open on: the first whose subject has faces, so a touching case's empty
// intersection does not open on nothing.
export function firstView(views: GlbView[]): number {
  const i = views.findIndex(v => subjectOf(v).triangles.length > 0)
  return i < 0 ? 0 : i
}
