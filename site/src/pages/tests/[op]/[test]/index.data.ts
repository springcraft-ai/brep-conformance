import { site } from '@/data/source'
import { readDoc } from '@/docs/source'
import {
  leaderboard,
  type CaseRow,
  type Check,
  type Failure,
  type Grade,
  type Kind,
  type Report,
  type TestEntry,
} from '@/data/results'
import type { PageEntry } from '@/site/page'
import { clausesOf, definitionOf, KIND_TEXT, type Definition } from './definition'

// The failing points a page carries per check; the sample synthesizes at most a dozen.
const MAX_POINTS = 64

export interface CheckView extends Check {
  promise: string
}

// One case a kernel failed, with its checks.
export interface CaseView {
  id: string
  checks: CheckView[]
}

// What one kernel wrote for a case: its drawing, and a STEP file for each solid the case shows.
export interface CaseFiles {
  glb: string | null
  steps: string[]
}

// One case of the test, with each kernel's grade on it and the files that kernel wrote, both in
// `kernels` order: null where its report has no such case, or it wrote no file.
export interface CaseOption {
  id: string
  grades: (Grade | null)[]
  files: (CaseFiles | null)[]
}

export interface KernelView {
  kernel: string
  version: string
  suite: string
  grade: Grade
  why: string
  failure: Failure | null
  passing: number
  total: number
  // Its place on the leaderboard, from 0.
  rank: number
  // The kernel's worst case, when it is not a pass.
  worst: CaseView | null
}

export interface Props {
  test: string
  slug: string
  operation: string
  kind: Kind
  kindText: string
  definition: Definition | null
  cases: number
  kernels: KernelView[]
  caseList: CaseOption[]
}

let docs: { clauses: Record<string, string>; api: string } | undefined
const specs = () =>
  (docs ??= { clauses: clausesOf(readDoc('contract.md')), api: readDoc('api.md') })

function caseView(c: CaseRow): CaseView {
  return {
    id: c.id,
    checks: c.checks.map(k => ({
      ...k,
      promise: specs().clauses[k.clause] ?? '',
      points: k.points.slice(0, MAX_POINTS),
    })),
  }
}

export function propsOf(t: TestEntry, latest: Report[]): Props {
  const kernels: KernelView[] = []
  const rowsOf: Map<string, CaseRow>[] = []
  const board = leaderboard(latest).map(r => r.kernel)
  for (const r of latest) {
    const k = t.kernels[r.identity.kernel]
    if (!k) continue
    const rows = r.cases.filter(c => c.operation === t.operation && c.slug === t.slug)
    rowsOf.push(new Map(rows.map(c => [c.id, c])))
    const worst = rows.find(c => c.grade === k.grade)
    kernels.push({
      kernel: r.identity.kernel,
      version: r.identity.version,
      suite: r.methodology.suite,
      grade: k.grade,
      why: k.why,
      failure: k.failure,
      passing: k.summary.passing,
      total: k.summary.total,
      rank: board.indexOf(r.identity.kernel),
      worst: worst && worst.grade !== '1e-7' ? caseView(worst) : null,
    })
  }
  const caseList = t.cases.flatMap((id): CaseOption[] => {
    const rows = rowsOf.map(rows => rows.get(id))
    if (!rows.some(Boolean)) return []
    return [
      {
        id,
        grades: rows.map(c => c?.grade ?? null),
        files: rows.map(c => (c?.glb || c?.steps.length ? { glb: c.glb, steps: c.steps } : null)),
      },
    ]
  })
  return {
    test: t.test,
    slug: t.slug,
    operation: t.operation,
    kind: t.kind,
    kindText: KIND_TEXT[t.kind],
    definition: definitionOf(t.operation, specs().api),
    cases: t.cases.length,
    kernels,
    caseList,
  }
}

export function entries(): PageEntry<Props>[] {
  const s = site()
  return s.tests.map(t => ({
    params: { op: t.operation, test: t.slug },
    props: propsOf(t, s.latest),
    title: t.test,
    description: `${t.test}: its definition, checks and each kernel's result on it.`,
  }))
}
