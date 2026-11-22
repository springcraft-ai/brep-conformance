import { boardSegment } from '@/data/results'
import { site } from '@/data/source'
import type { PageEntry } from '@/site/page'
import { AREA_SECTION } from '../reports/[report]/compare/[...path]/definition'
import { operations } from './doc'

// A kernel's column is one run: the version it ran, against which suite, on which day.
export interface KernelColumn {
  kernel: string
  // Its leaderboard page in its report: boardSegment.
  segment: string
  version: string
  suite: string
  date: string
  // How many of the functions it offers.
  offered: number
}

export interface FunctionRow {
  id: string
  signature: string | null
  // Per kernel, in `kernels` order: offered, not offered, or the function the suite sends in its
  // place.
  cells: (boolean | string)[]
}

export interface GroupRows {
  group: string
  // The test areas that exercise the group's functions, for the reader to go to.
  areas: string[]
  functions: FunctionRow[]
}

export interface Props {
  total: number
  // The suite versions the runs were made against, one where they agree.
  suites: string[]
  kernels: KernelColumn[]
  groups: GroupRows[]
}

export function propsOf(): Props {
  const s = site()
  const ops = operations().operations
  const kernels: KernelColumn[] = s.latest.map(r => ({
    kernel: r.identity.kernel,
    segment: boardSegment(r),
    version: r.identity.version,
    suite: r.methodology.suite,
    date: r.environment.date,
    offered: ops.filter(o => o.offeredBy.includes(r.identity.kernel)).length,
  }))
  const groups: GroupRows[] = []
  for (const o of ops) {
    let g = groups.find(x => x.group === o.doc.group)
    if (!g) {
      const areas = Object.keys(AREA_SECTION).filter(a => AREA_SECTION[a] === o.doc.group)
      g = { group: o.doc.group, areas, functions: [] }
      groups.push(g)
    }
    g.functions.push({
      id: o.id,
      signature: o.doc.signatures[0] ?? null,
      cells: kernels.map(k => {
        const support = o.kernels.find(x => x.kernel === k.kernel)
        if (!support) return false
        return support.offered ? true : (support.standIn ?? false)
      }),
    })
  }
  return {
    total: ops.length,
    suites: [...new Set(kernels.map(k => k.suite))],
    kernels,
    groups,
  }
}

export function entries(): PageEntry<Props>[] {
  return [
    {
      props: propsOf(),
      title: 'operations',
      description: 'Every operation the suite specifies, and which kernels offer it.',
    },
  ]
}
