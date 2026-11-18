import { GRADES, KINDS, type CaseRow, type Grade, type Kind } from '@/data/results'

// The case table as the page ships it: tests and reasons interned, one short tuple per case, so
// ~2,900 rows hydrate as tens of kilobytes rather than the report's hundreds.
export interface PackedCases {
  tests: [test: string, slug: string, operation: string][]
  whys: string[]
  // [case id, test index, kind index, grade index, why index, time in ms or null]
  rows: [string, number, number, number, number, number | null][]
  // The tests with no page of their own, and the cases their test's page does not list, by index:
  // a test's page is drawn from each kernel's latest report alone.
  unpaged: number[]
  unlisted: number[]
}

// The cases each test's page lists, by `pageKey`.
export type TestPages = ReadonlyMap<string, ReadonlySet<string>>

export function pageKey(operation: string, slug: string): string {
  return `${operation}/${slug}`
}

export interface Row {
  order: number
  id: string
  test: string
  slug: string
  operation: string
  kind: Kind
  grade: Grade
  rank: number
  why: string
  timeMs: number | null
  // Whether the test has a page, and whether that page lists this case.
  paged: boolean
  listed: boolean
}

function interner<T>(key: (t: T) => string) {
  const index = new Map<string, number>()
  const list: T[] = []
  return {
    list,
    add(t: T): number {
      const k = key(t)
      let i = index.get(k)
      if (i === undefined) {
        i = list.length
        list.push(t)
        index.set(k, i)
      }
      return i
    },
  }
}

// With no `pages`, every test has a page that lists each of its cases.
export function packCases(cases: CaseRow[], pages?: TestPages): PackedCases {
  const tests = interner<[string, string, string]>(t => `${t[2]}/${t[0]}`)
  const whys = interner<string>(w => w)
  const listing = (c: CaseRow) => pages?.get(pageKey(c.operation, c.slug))
  return {
    unlisted: pages ? cases.flatMap((c, i) => (listing(c)?.has(c.id) ? [] : [i])) : [],
    rows: cases.map(c => [
      c.id,
      tests.add([c.test, c.slug, c.operation]),
      KINDS.indexOf(c.kind),
      GRADES.indexOf(c.grade),
      whys.add(c.why),
      c.timeMs,
    ]),
    tests: tests.list,
    whys: whys.list,
    unpaged: pages
      ? tests.list.flatMap(([, slug, op], i) => (pages.has(pageKey(op, slug)) ? [] : [i]))
      : [],
  }
}

export function unpackCases(packed: PackedCases): Row[] {
  const unpaged = new Set(packed.unpaged)
  const unlisted = new Set(packed.unlisted)
  return packed.rows.map(([id, t, k, g, w, timeMs], order) => {
    const [test, slug, operation] = packed.tests[t]!
    return {
      order,
      id,
      test,
      slug,
      operation,
      kind: KINDS[k]!,
      grade: GRADES[g]!,
      rank: g,
      why: packed.whys[w]!,
      timeMs,
      paged: !unpaged.has(t),
      listed: !unlisted.has(order),
    }
  })
}

// 'below' is every case that did not pass at 1e-7: the cases the headline counts against. A set,
// grades joined by commas in GRADES order, is the cases of any of them: a bar's band can draw
// several, such as fail with crash and 1e-3, and lists them all.
export type GradeFilter = 'all' | 'below' | Grade | `${Grade},${string}`

// The grades a filter keeps, or null for 'all' and 'below'.
export function gradesOf(f: GradeFilter): Grade[] | null {
  return f === 'all' || f === 'below' ? null : (f.split(',') as Grade[])
}

// A set of grades as a filter: one grade alone, several in GRADES order.
export function gradeSet(grades: readonly Grade[]): GradeFilter {
  return GRADES.filter(g => grades.includes(g)).join(',') as GradeFilter
}

export interface Filter {
  grade: GradeFilter
  operation: string
  // One test's path, as the results grid links to it; 'all' for every test.
  test: string
  kind: 'all' | Kind
  q: string
}

export const NO_FILTER: Filter = { grade: 'all', operation: 'all', test: 'all', kind: 'all', q: '' }

// The filter a report opens on: the cases that fall short, where it has any, since those are what
// a reader comes for.
export function openingFilter(short: number): Filter {
  return short > 0 ? { ...NO_FILTER, grade: 'below' } : NO_FILTER
}

export function isUnfiltered(f: Filter): boolean {
  return (
    f.grade === 'all' &&
    f.operation === 'all' &&
    f.test === 'all' &&
    f.kind === 'all' &&
    !f.q.trim()
  )
}

export function matches(row: Row, f: Filter): boolean {
  const grades = gradesOf(f.grade)
  if (f.grade === 'below' ? row.grade === '1e-7' : grades && !grades.includes(row.grade))
    return false
  if (f.operation !== 'all' && row.operation !== f.operation) return false
  if (f.test !== 'all' && row.test !== f.test) return false
  if (f.kind !== 'all' && row.kind !== f.kind) return false
  const q = f.q.trim().toLowerCase()
  if (!q) return true
  return q
    .split(/\s+/)
    .every(
      w =>
        row.test.toLowerCase().includes(w) ||
        row.id.toLowerCase().includes(w) ||
        row.why.toLowerCase().includes(w) ||
        row.operation.includes(w),
    )
}

export type SortKey = 'order' | 'test' | 'operation' | 'kind' | 'grade' | 'why' | 'time'
export type SortDir = 'asc' | 'desc'

// Cases with no time sort last in either direction; ties keep the report's order.
export function sortRows(rows: Row[], key: SortKey, dir: SortDir): Row[] {
  const sign = dir === 'asc' ? 1 : -1
  const cmp: (a: Row, b: Row) => number = {
    order: (a: Row, b: Row) => a.order - b.order,
    test: (a: Row, b: Row) => a.test.localeCompare(b.test),
    operation: (a: Row, b: Row) => a.operation.localeCompare(b.operation),
    kind: (a: Row, b: Row) => KINDS.indexOf(a.kind) - KINDS.indexOf(b.kind),
    grade: (a: Row, b: Row) => a.rank - b.rank,
    why: (a: Row, b: Row) => a.why.localeCompare(b.why),
    time: (a: Row, b: Row) => (a.timeMs ?? 0) - (b.timeMs ?? 0),
  }[key]
  return [...rows].sort((a, b) => {
    if (key === 'time' && (a.timeMs === null) !== (b.timeMs === null))
      return a.timeMs === null ? 1 : -1
    return sign * cmp(a, b) || a.order - b.order
  })
}

// A grade filter from a query string: 'all', 'below', or grades this suite knows, as a set.
function gradeFilterOf(value: string): GradeFilter {
  if (value === 'all' || value === 'below') return value
  const parts = value.split(',')
  if (!parts.length || !parts.every(g => (GRADES as readonly string[]).includes(g))) return 'all'
  return gradeSet(parts as Grade[])
}
const FILTER_KEYS = ['grade', 'op', 'test', 'kind', 'q'] as const

// The filter lives in the query string, so a filtered view of a report is a link.
export function filterFromSearch(search: string): Filter {
  const p = new URLSearchParams(search)
  const grade = p.get('grade') ?? 'all'
  const kind = p.get('kind') ?? 'all'
  return {
    grade: gradeFilterOf(grade),
    operation: p.get('op') || 'all',
    test: p.get('test') || 'all',
    kind: (KINDS as readonly string[]).includes(kind) ? (kind as Kind) : 'all',
    q: p.get('q') ?? '',
  }
}

// The page's filter for a query string. An empty one opens on `opening`, so every other view says
// so in the query string, the whole table as `?grade=all`.
export function filterFromLocation(search: string, opening: Filter): Filter {
  // Only a filter's own keys count: a query string that names none of them, such as a campaign
  // tag, is still the opening view.
  const p = new URLSearchParams(search)
  return FILTER_KEYS.some(k => p.has(k)) ? filterFromSearch(search) : opening
}

export function filterToLocation(f: Filter, opening: Filter): string {
  if (sameFilter(f, opening)) return ''
  return filterToSearch(f) || '?grade=all'
}

// Two filters that show the same cases: a search of spaces alone is no search.
export function sameFilter(a: Filter, b: Filter): boolean {
  const key = (f: Filter) => filterToSearch({ ...f, q: f.q.trim() })
  return key(a) === key(b)
}

// What a test's page reads to show one case, as one kernel drew it where the kernel is named.
export function caseHash(kernel: string | null, id: string): string {
  const e = encodeURIComponent
  return `#${kernel ? `kernel=${e(kernel)}&` : ''}case=${e(id)}`
}

// The six characters of a case id's hash that a table shows; the whole id is the case's name.
export function shortCase(id: string): string {
  return id.slice(id.lastIndexOf('-') + 1).slice(0, 6)
}

export function filterToSearch(f: Filter): string {
  const p = new URLSearchParams()
  if (f.grade !== 'all') p.set('grade', f.grade)
  if (f.operation !== 'all') p.set('op', f.operation)
  if (f.test !== 'all') p.set('test', f.test)
  if (f.kind !== 'all') p.set('kind', f.kind)
  if (f.q) p.set('q', f.q)
  const s = p.toString()
  return s ? `?${s}` : ''
}

// A reason without the entities it names: "… does not hold (+e1487, +f848)" and the same sentence
// about other edges are one reason, said of different cases.
export function reasonOf(why: string): string {
  return why
    .replace(/\s*\((?:[+-]?[a-z]+\d+(?:,\s*|\s+and\s+)?)+\)/g, '')
    .replace(/\s*[+-][a-z]+\d+\b/g, '')
    .trim()
}

// The shown cases of one test, as one row of the table: what they came to, and why.
export interface Group {
  test: string
  slug: string
  operation: string
  paged: boolean
  rows: Row[]
  // Every case of the test in the report, shown or not.
  total: number
  // Shown cases per grade, in GRADES order, grades with none left out.
  grades: [Grade, number][]
  // The distinct reasons among the shown cases, as reasonOf reads them, the commonest first.
  whys: string[]
  // How many of the shown cases give a reason at all.
  reasoned: number
  medianMs: number | null
}

function median(xs: number[]): number | null {
  if (!xs.length) return null
  const s = [...xs].sort((a, b) => a - b)
  const mid = s.length >> 1
  return s.length % 2 ? s[mid]! : (s[mid - 1]! + s[mid]!) / 2
}

// The shown rows gathered by test, tests in the order their first row comes, so a sort of the rows
// carries to the groups: worst first puts the test with the worst case first.
export function groupByTest(shown: Row[], all: Row[]): Group[] {
  const totals = new Map<string, number>()
  for (const r of all) totals.set(r.test, (totals.get(r.test) ?? 0) + 1)
  const groups = new Map<string, Row[]>()
  for (const r of shown) {
    const g = groups.get(r.test)
    if (g) g.push(r)
    else groups.set(r.test, [r])
  }
  return [...groups.values()].map(rows => {
    const whys = new Map<string, number>()
    for (const r of rows) {
      const why = reasonOf(r.why)
      if (why) whys.set(why, (whys.get(why) ?? 0) + 1)
    }
    const first = rows[0]!
    return {
      test: first.test,
      slug: first.slug,
      operation: first.operation,
      paged: first.paged,
      rows,
      total: totals.get(first.test) ?? rows.length,
      grades: GRADES.map((g): [Grade, number] => [
        g,
        rows.filter(r => r.grade === g).length,
      ]).filter(([, n]) => n > 0),
      whys: [...whys].sort((a, b) => b[1] - a[1]).map(([w]) => w),
      reasoned: [...whys.values()].reduce((a, b) => a + b, 0),
      medianMs: median(rows.flatMap(r => (r.timeMs === null ? [] : [r.timeMs]))),
    }
  })
}

export interface OperationRow {
  operation: string
  total: number
  passing: number
  // Cases per grade, in GRADES order.
  counts: number[]
}

// Each operation's cases by grade, operations by name.
export function byOperation(rows: Row[]): OperationRow[] {
  const ops = new Map<string, number[]>()
  for (const r of rows) {
    const counts = ops.get(r.operation) ?? GRADES.map(() => 0)
    ops.set(r.operation, counts)
    counts[r.rank]!++
  }
  return [...ops]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([operation, counts]) => ({
      operation,
      counts,
      passing: counts[0]!,
      total: counts.reduce((a, b) => a + b, 0),
    }))
}

// A place in a ranking as it is said: 1st, 2nd, 3rd, 4th, 11th, 22nd.
export function ordinal(n: number): string {
  const tens = n % 100
  const suffix = tens >= 11 && tens <= 13 ? 'th' : (['th', 'st', 'nd', 'rd'][n % 10] ?? 'th')
  return `${n}${suffix}`
}

export function totalTimeMs(cases: CaseRow[]): number {
  return cases.reduce((n, c) => n + (c.timeMs ?? 0), 0)
}

// A wall time as a reader says it: 840 ms, 12.4 s, 7 min 05 s, 2 h 14 min.
export function duration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`
  const s = ms / 1000
  if (s < 60) return `${s.toFixed(1)} s`
  const whole = Math.round(s)
  if (whole < 3600) {
    const m = Math.floor(whole / 60)
    return `${m} min ${String(whole % 60).padStart(2, '0')} s`
  }
  const m = Math.round(whole / 60)
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')} min`
}
