import {
  GRADES,
  TIERS,
  worstGrade,
  type CaseTime,
  type Grade,
  type LeaderRow,
  type TestEntry,
} from '@/data/results'
import { kernelInfo, repoName } from '@/data/kernels'
import { markGroups } from '@/components/Grade'

// Pure shaping of the shared derivations into what the home page draws. Hydrated, so it ships to
// the browser: no file access here.

// Case counts in GRADES order, best first.
export type Counts = number[]

// Counts as the page draws them. The board reads passes at 1e-7 and at 1e-5 and nothing else, so a
// case held at 1e-6 is drawn with those held at 1e-5, and one held only at 1e-3 passes neither and
// is drawn with the failures.
export function drawn(counts: Counts): Counts {
  const out = counts.slice()
  const fold = (from: Grade, into: Grade) => {
    const f = GRADES.indexOf(from)
    const i = GRADES.indexOf(into)
    out[i] = (out[i] ?? 0) + (out[f] ?? 0)
    out[f] = 0
  }
  fold('1e-6', '1e-5')
  fold('1e-3', 'fail')
  return out
}

// The grades a band of a bar holds: those that share its mark, and those drawn into it. A band
// lists its cases by this set, so it lists every case it draws.
export function bandGrades(g: Grade): Grade[] {
  const group = markGroups().find(m => m.includes(g)) ?? [g]
  const into: Grade[] = [...group]
  if (into.includes('1e-5')) into.push('1e-6')
  if (into.includes('fail')) into.push('1e-3')
  return GRADES.filter(x => into.includes(x))
}

export interface Cell {
  grade: Grade
  counts: Counts
  // The test's median case time; null on an operation's row, which spans tests.
  medianMs: number | null
}

export interface GridTest {
  slug: string
  test: string
  cells: (Cell | null)[]
}

export interface GridGroup {
  operation: string
  cells: (Cell | null)[]
  tests: GridTest[]
}

export interface BoardRow {
  rank: number
  kernel: string
  version: string
  suite: string
  rate: number
  passing: number
  total: number
  counts: Counts
  // A kernel's time over the fastest kernel's, test by test; sorts ascending as faster, and a
  // kernel with no comparable test sorts last.
  speed: number
  // The tests the factor is taken over.
  speedGroups: number
  time: CaseTime | null
  // Sorts ascending as faster; a kernel with no common case sorts last.
  medianMs: number
  license: string | null
  source: string | null
  // The adapter's repository, from the report of an official run; null on any other.
  adapter: string | null
}

export function countsOf(grades: Record<Grade, number>): Counts {
  return GRADES.map(g => grades[g])
}

function addCounts(into: Counts, more: Counts): Counts {
  return into.map((n, i) => n + more[i]!)
}

function cellOf(counts: Counts, medianMs: number | null = null): Cell {
  return { grade: worstGrade(GRADES.filter((_, i) => counts[i]! > 0)), counts, medianMs }
}

export function passing(cell: Cell): number {
  return cell.counts[0]!
}

export function total(cell: Cell): number {
  return cell.counts.reduce((a, b) => a + b, 0)
}

export function boardRows(board: LeaderRow[]): BoardRow[] {
  return board.map((r, i) => {
    const info = kernelInfo(r.kernel)
    return {
      rank: i + 1,
      kernel: r.kernel,
      version: r.version,
      suite: r.suite,
      rate: r.summary.passRate,
      passing: r.summary.passing,
      total: r.summary.total,
      counts: countsOf(r.summary.grades),
      speed: r.speed.factor ?? Number.POSITIVE_INFINITY,
      speedGroups: r.speed.groups,
      time: r.time,
      medianMs: r.time?.median ?? Number.POSITIVE_INFINITY,
      license: info?.license ?? null,
      source: info?.source ?? null,
      adapter: r.adapter ? repoName(r.adapter.repo) : null,
    }
  })
}

// Every test in operation order, one cell per kernel in `kernels` order; null where a kernel's
// report has no such test.
export function grid(tests: TestEntry[], kernels: string[]): GridGroup[] {
  const groups = new Map<string, GridGroup>()
  for (const t of tests) {
    let g = groups.get(t.operation)
    if (!g) {
      g = { operation: t.operation, cells: kernels.map(() => null), tests: [] }
      groups.set(t.operation, g)
    }
    const cells = kernels.map(k => {
      const tk = t.kernels[k]
      return tk ? cellOf(countsOf(tk.summary.grades), tk.medianMs) : null
    })
    g.tests.push({ slug: t.slug, test: t.test, cells })
    g.cells = g.cells.map((c, i) => {
      const add = cells[i]
      if (!add) return c
      return cellOf(c ? addCounts(c.counts, add.counts) : add.counts)
    })
  }
  return [...groups.values()]
}

export const FILTERS = ['all', 'failed', 'unsupported', 'looser', 'pass'] as const
export type Filter = (typeof FILTERS)[number]

export const FILTER_LABEL: Record<Filter, string> = {
  all: 'all tests',
  failed: 'failed',
  unsupported: 'unsupported',
  looser: 'looser tolerance',
  pass: 'all pass',
}

const LOOSER = GRADES.filter(g => g !== '1e-7' && (TIERS as readonly string[]).includes(g))
const FAILED: Grade[] = ['refused', 'timeout', 'fail', 'crash']

function has(cell: Cell | null, grades: readonly Grade[]): boolean {
  return cell !== null && grades.some(g => cell.counts[GRADES.indexOf(g)]! > 0)
}

// Whether a filter keeps a test, asked of the kernel at `scope` or, with null, of every kernel
// shown. A test may be kept by several: one kernel fails it and another does not offer it.
export function keep(test: GridTest, filter: Filter, scope: number | null = null): boolean {
  const cells = scope === null ? test.cells : [test.cells[scope] ?? null]
  switch (filter) {
    case 'all':
      return true
    case 'failed':
      return cells.some(c => has(c, FAILED))
    case 'unsupported':
      return cells.some(c => has(c, ['unsupported']))
    case 'looser':
      return cells.some(c => has(c, LOOSER))
    case 'pass':
      return cells.every(c => c !== null && total(c) > 0 && passing(c) === total(c))
  }
}

// The filters worth choosing on these tests: 'all', and each other one unless it keeps no test or
// every test. Two that keep the same tests both stay, so a filter is never off while it has tests.
export function distinctFilters(tests: GridTest[], scope: number | null = null): Filter[] {
  return FILTERS.filter(f => {
    if (f === 'all') return true
    const kept = tests.filter(t => keep(t, f, scope)).length
    return kept > 0 && kept < tests.length
  })
}

// The share to print for `part` of `whole`: one failure in thousands rounds to 100.0%, so only a
// full pass shows as 100% and only none as 0%.
export function shownShare(part: number, whole: number): number {
  if (part === whole) return 1
  if (part === 0) return 0
  return Math.min(0.999, Math.max(0.001, part / whole))
}

export function caseTitle(kernel: string, name: string, cell: Cell | null): string {
  if (!cell) return `${kernel} · ${name}: not run`
  const total = cell.counts.reduce((a, b) => a + b, 0)
  const parts = GRADES.flatMap((g, i) => (cell.counts[i] ? [`${cell.counts[i]} ${g}`] : []))
  return `${kernel} · ${name}: ${parts.join(', ')} of ${total} cases`
}

// The most kernels the grid compares at once, and the fewest it lets a reader narrow it to.
export const COMPARE_MAX = 4
export const COMPARE_MIN = 2

// The first COMPARE_MAX kernels in rank order.
export function defaultCompare(board: BoardRow[]): string[] {
  return board.slice(0, COMPARE_MAX).map(r => r.kernel)
}

export function toggleCompare(picked: string[], kernel: string, order: string[]): string[] {
  const on = picked.includes(kernel)
  if (on && picked.length > COMPARE_MIN) return picked.filter(k => k !== kernel)
  if (!on && picked.length < COMPARE_MAX)
    return order.filter(k => k === kernel || picked.includes(k))
  return picked
}

// Each cell's time against the fastest of the cells shown that passed a case: 1 is fastest, null
// where a cell has no time or passed nothing. A report rounds a time under its resolution to 0,
// which no ratio can be taken against, so a 0 is not compared either.
export function relativeTimes(cells: (Cell | null)[]): (number | null)[] {
  const timed = cells.map(c => (c && c.medianMs && passing(c) > 0 ? c.medianMs : null))
  const known = timed.filter((t): t is number => t !== null)
  const best = known.length ? Math.min(...known) : null
  return timed.map(t => (t === null || best === null ? null : t / best))
}

export function matchesQuery(test: GridTest, operation: string, q: string): boolean {
  const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean)
  const hay = `${operation} ${test.test} ${test.slug}`.toLowerCase()
  return words.every(w => hay.includes(w))
}
