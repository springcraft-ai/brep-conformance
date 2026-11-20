// Reads the flat report `brep-conformance-report --flat` writes (report::flat in
// suite/src/report.rs, described in doc/formats.md): the kernel, suite and run, then one row per
// case. Every page reads the parsed types and the derivations below, never the raw JSON.
//
// This module is pure, so a hydrated page may import its types and helpers. Reading files is
// source.ts's job.

export const TIERS = ['1e-7', '1e-6', '1e-5', '1e-3'] as const
export type Tier = (typeof TIERS)[number]

export const KINDS = ['exact', 'sound', 'rejection'] as const
export type Kind = (typeof KINDS)[number]

export const FAILURES = ['geometry', 'measurement'] as const
export type Failure = (typeof FAILURES)[number]

// A case's result: the tier it reached, else why it reached none. Ordered best first; a tier ranks
// above every outcome that reached none.
export const GRADES = [...TIERS, 'unsupported', 'refused', 'timeout', 'fail', 'crash'] as const
export type Grade = (typeof GRADES)[number]
export const RESULTS = GRADES
export type Result = Grade

// Where shape files are published: `assets::ASSET_BASE` in suite/src/assets.rs. Each kernel's files
// lie under <suite>/<kernel>/<version>/ there, as its report does in the reports directory. A
// report names that folder as `assets`; this is for a report written before it did.
export const ASSET_BASE = 'https://cdn.springcraft.ai/brep-conformance'

// Where the dev and preview servers serve BREP_ASSETS_DIR, a local copy of the CDN's folders, under
// the site's base (vite.config.ts).
export const LOCAL_ASSETS = 'local-assets'

// The folder a report's files load from, under `from`, without its closing slash: null where the run
// wrote none. A report is its kernel's own word, so `assets` must be the report's own folder on the
// suite's CDN: no other host, and no other kernel's folder.
function assetFolder(
  assets: unknown,
  suite: string,
  kernel: string,
  version: string,
  at: string,
  from: string,
): string | null {
  if (assets === null) return null
  const own = `${ASSET_BASE}/${suite}/${kernel}/${version}/`
  if (assets !== undefined && assets !== own)
    throw new Error(`${at}.assets: ${String(assets)} is not ${own}`)
  return `${from}/${suite}/${kernel}/${version}`
}

export interface FailPoint {
  p: [number, number, number]
  got: boolean
  want: boolean
}

// A failed check's numbers. The flat report does not carry checks yet, so rows have none.
export interface Check {
  clause: string
  measure: string
  got: number
  want: number
  error: number
  bound: number
  unit: string
  points: FailPoint[]
}

export interface CaseRow {
  id: string
  test: string
  slug: string
  operation: string
  kind: Kind
  result: Result
  tier: Tier | null
  grade: Grade
  why: string
  timeMs: number | null
  groupMedianMs: number | null
  groupLowConfidence: boolean
  // The files this kernel wrote for the case, as URLs: its drawing, null where it drew none, and
  // one STEP file per solid of its first view, none where it wrote none.
  glb: string | null
  steps: string[]
  failure: Failure | null
  checks: Check[]
}

// The adapter an official run used, at the commit the registry pinned; null on any other run.
export interface Adapter {
  repo: string
  commit: string
}

export interface Identity {
  // The kernel's name as its adapter reports it, which also names its directory of reports.
  kernel: string
  name: string
  version: string
  declaredTolerance: number | null
  configuration: string | null
  adapter: Adapter | null
}

export interface Environment {
  os: string
  arch: string
  cores: number | null
  threads: number | null
  // When the run ended, as an ISO date and time; empty when the report has no time.
  date: string
  // Run on the suite's own benchmark machine.
  verified: boolean
}

export interface Methodology {
  suite: string
  contract: string
  // Null where the run wrote no shape files.
  assetBase: string | null
}

export interface Report {
  identity: Identity
  environment: Environment
  methodology: Methodology
  // The functions the kernel does not offer, and what the suite sends in place of a primitive it
  // has no constructor for.
  unsupported: string[]
  standIns: Record<string, string>
  cases: CaseRow[]
}

type Obj = Record<string, unknown>

function obj(v: unknown, at: string): Obj {
  if (typeof v !== 'object' || v === null || Array.isArray(v))
    throw new Error(`${at}: not an object`)
  return v as Obj
}

function str(o: Obj, k: string, at: string): string {
  const v = o[k]
  if (typeof v !== 'string') throw new Error(`${at}.${k}: not a string`)
  return v
}

function strOr(o: Obj, k: string, fallback: string): string {
  return typeof o[k] === 'string' ? (o[k] as string) : fallback
}

function num(o: Obj, k: string, at: string): number {
  const v = o[k]
  if (typeof v !== 'number') throw new Error(`${at}.${k}: not a number`)
  return v
}

function numOrNull(o: Obj, k: string, at: string): number | null {
  return o[k] === null || o[k] === undefined ? null : num(o, k, at)
}

function oneOf<T extends string>(o: Obj, k: string, values: readonly T[], at: string): T {
  const v = str(o, k, at)
  if (!values.includes(v as T)) throw new Error(`${at}.${k}: ${v} is not one of ${values}`)
  return v as T
}

function arr(o: Obj, k: string, at: string): unknown[] {
  const v = o[k] ?? []
  if (!Array.isArray(v)) throw new Error(`${at}.${k}: not an array`)
  return v
}

export function testSlug(test: string): string {
  return test.split('/').slice(1).join('-')
}

export function gradeOf(result: Result): Grade {
  return result
}

function tierOf(result: Result): Tier | null {
  return (TIERS as readonly string[]).includes(result) ? (result as Tier) : null
}

// One part of a key: a plain name, so a key can hold no query, fragment, escape or climb.
const PLAIN_NAME = /^[A-Za-z0-9][A-Za-z0-9._+-]*$/

// A report is its kernel's own word, so a key may not climb out of that kernel's files.
function fileUrl(base: string | null, key: unknown, ext: string, at: string): string | null {
  if (key === null || key === undefined || base === null) return null
  if (typeof key !== 'string') throw new Error(`${at}: not a string`)
  const plain = key.split('/').every(part => PLAIN_NAME.test(part) && !part.includes('..'))
  if (!plain || !key.endsWith(`.${ext}`)) throw new Error(`${at}: ${key} is not a .${ext} key`)
  return `${base}/${key}`
}

function parseCase(v: unknown, at: string, base: string | null): CaseRow {
  const o = obj(v, at)
  const exports = obj(o.exports ?? {}, `${at}.exports`)
  const test = str(o, 'test', at)
  const result = oneOf(o, 'result', RESULTS, at)
  return {
    id: str(o, 'id', at),
    test,
    slug: testSlug(test),
    operation: str(o, 'operation', at),
    kind: oneOf(o, 'kind', KINDS, at),
    result,
    tier: tierOf(result),
    grade: gradeOf(result),
    why: typeof o.why === 'string' ? o.why : '',
    timeMs: numOrNull(o, 'time_ms', at),
    groupMedianMs: numOrNull(o, 'group_median_ms', at),
    groupLowConfidence: o.group_low_confidence !== false,
    glb: fileUrl(base, exports.glb, 'glb', `${at}.exports.glb`),
    steps: arr(exports, 'step', `${at}.exports`).flatMap((key, i) => {
      const url = fileUrl(base, key, 'step', `${at}.exports.step[${i}]`)
      return url ? [url] : []
    }),
    failure: o.failure ? oneOf(o, 'failure', FAILURES, at) : null,
    checks: [],
  }
}

function adapterOf(v: unknown, at: string): Adapter | null {
  if (v === undefined || v === null) return null
  const o = obj(v, at)
  return { repo: str(o, 'repo', at), commit: str(o, 'commit', at) }
}

// `assetsFrom` is where the CDN's folders load from: the CDN itself, or the local copy.
export function parseReport(raw: unknown, at = 'report', assetsFrom = ASSET_BASE): Report {
  const o = obj(raw, at)
  const k = obj(o.kernel, `${at}.kernel`)
  const suite = obj(o.suite, `${at}.suite`)
  const run = obj(o.run ?? {}, `${at}.run`)
  const env = obj(o.environment ?? {}, `${at}.environment`)
  const unix = numOrNull(run, 'unix', `${at}.run`)
  const standIns = obj(o.stand_ins ?? {}, `${at}.stand_ins`)
  const name = str(k, 'name', `${at}.kernel`)
  const version = str(k, 'version', `${at}.kernel`)
  const suiteVersion = str(suite, 'version', `${at}.suite`)
  const assetBase = assetFolder(o.assets, suiteVersion, name, version, at, assetsFrom)
  return {
    identity: {
      kernel: name,
      name,
      version,
      declaredTolerance: numOrNull(k, 'tolerance', `${at}.kernel`),
      configuration:
        typeof k.configuration === 'string' && k.configuration ? k.configuration : null,
      adapter: adapterOf(o.adapter, `${at}.adapter`),
    },
    environment: {
      os: strOr(env, 'os', ''),
      arch: strOr(env, 'arch', ''),
      cores: numOrNull(env, 'cores', `${at}.environment`),
      threads: numOrNull(env, 'threads', `${at}.environment`),
      date: unix ? new Date(unix * 1000).toISOString() : '',
      verified: run.verified === true,
    },
    methodology: {
      suite: suiteVersion,
      contract: strOr(suite, 'contract', ''),
      assetBase,
    },
    unsupported: arr(o, 'unsupported', at).map(String),
    standIns: Object.fromEntries(Object.entries(standIns).map(([a, b]) => [a, String(b)])),
    cases: arr(o, 'cases', at).map((c, i) => parseCase(c, `${at}.cases[${i}]`, assetBase)),
  }
}

const count = <K extends string>(keys: readonly K[]) =>
  Object.fromEntries(keys.map(k => [k, 0])) as Record<K, number>

export interface Summary {
  total: number
  passing: number
  passRate: number
  grades: Record<Grade, number>
  failures: Record<Failure, number>
  kinds: Record<Kind, { total: number; passing: number }>
}

// The headline: the share of all cases that pass at 1e-7. Every other outcome counts against it.
export function summarize(cases: CaseRow[]): Summary {
  const grades = count(GRADES)
  const failures = count(FAILURES)
  const kinds = Object.fromEntries(
    KINDS.map(k => [k, { total: 0, passing: 0 }]),
  ) as Summary['kinds']
  for (const c of cases) {
    grades[c.grade]++
    if (c.failure) failures[c.failure]++
    kinds[c.kind].total++
    if (c.grade === '1e-7') kinds[c.kind].passing++
  }
  const passing = grades['1e-7']
  return {
    total: cases.length,
    passing,
    passRate: cases.length ? passing / cases.length : 0,
    grades,
    failures,
    kinds,
  }
}

export function worstGrade(grades: Iterable<Grade>): Grade {
  let worst = 0
  for (const g of grades) worst = Math.max(worst, GRADES.indexOf(g))
  return GRADES[worst]!
}

export function compareVersions(a: string, b: string): number {
  const pa = a.split(/[.-]/)
  const pb = b.split(/[.-]/)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? '0'
    const y = pb[i] ?? '0'
    const d = /^\d+$/.test(x) && /^\d+$/.test(y) ? Number(x) - Number(y) : x.localeCompare(y)
    if (d) return Math.sign(d)
  }
  return 0
}

export function currentSuite(reports: Report[]): string {
  return (
    reports
      .map(r => r.methodology.suite)
      .sort(compareVersions)
      .at(-1) ?? ''
  )
}

const RELEASE_NUMBER = /^\d+(\.\d+)*$/

// Two runs of one kernel, older first: by release number when both have one, else by when they ran.
export function compareRuns(
  a: { version: string; date: string },
  b: { version: string; date: string },
): number {
  const numbered = RELEASE_NUMBER.test(a.version) && RELEASE_NUMBER.test(b.version)
  const order = numbered ? compareVersions(a.version, b.version) : 0
  return order || (a.date < b.date ? -1 : a.date > b.date ? 1 : 0)
}

const runOf = (r: Report) => ({ version: r.identity.version, date: r.environment.date })

// One report per kernel: its latest run on the newest suite version it has a run on.
export function latestByKernel(reports: Report[]): Report[] {
  const best = new Map<string, Report>()
  for (const r of reports) {
    const prev = best.get(r.identity.kernel)
    const suite = prev ? compareVersions(r.methodology.suite, prev.methodology.suite) : 1
    if (suite > 0 || (suite === 0 && compareRuns(runOf(r), runOf(prev!)) > 0))
      best.set(r.identity.kernel, r)
  }
  return [...best.values()].sort((a, b) => a.identity.kernel.localeCompare(b.identity.kernel))
}

// One report per kernel: its latest release run on the current suite version.
export function latestReports(reports: Report[]): Report[] {
  const suite = currentSuite(reports)
  return latestByKernel(reports.filter(r => r.methodology.suite === suite))
}

function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b)
  const m = s.length >> 1
  return s.length % 2 ? s[m]! : (s[m - 1]! + s[m]!) / 2
}

export interface Speed {
  // Geometric mean over groups of this kernel's median time over the fastest kernel's; 1 is
  // fastest everywhere. Null when no group had a case every kernel passed at one tier.
  factor: number | null
  groups: number
  medians: Record<string, number>
}

// Groups are tests. A group's first case is a warmup and is dropped; the rest count only where
// every kernel compared reached the same tier, so no kernel is timed on work another skipped.
export function speedFactors(reports: Report[]): Record<string, Speed> {
  const kernels = reports.map(r => r.identity.kernel)
  const byId = reports.map(r => new Map(r.cases.map(c => [c.id, c])))
  const groups = new Map<string, string[]>()
  for (const c of reports[0]?.cases ?? []) {
    const ids = groups.get(c.test) ?? []
    ids.push(c.id)
    groups.set(c.test, ids)
  }
  const out: Record<string, Speed> = Object.fromEntries(
    kernels.map(k => [k, { factor: null, groups: 0, medians: {} }]),
  )
  const logs: number[][] = kernels.map(() => [])
  for (const [test, ids] of groups) {
    const common = ids.slice(1).filter(id => {
      const rows = byId.map(m => m.get(id))
      const tier = rows[0]?.tier
      return rows.every(c => c && c.tier === tier && c.tier !== null && c.timeMs !== null)
    })
    if (!common.length) continue
    const meds = byId.map(m => median(common.map(id => m.get(id)!.timeMs!)))
    const fastest = Math.min(...meds)
    // A median below the timer's resolution has no ratio; counting it as a tie would pull every
    // kernel toward 1.
    if (!(fastest > 0)) continue
    meds.forEach((med, i) => {
      out[kernels[i]!]!.medians[test] = med
      logs[i]!.push(Math.log(med / fastest))
    })
  }
  kernels.forEach((k, i) => {
    const l = logs[i]!
    out[k] = {
      ...out[k]!,
      groups: l.length,
      factor: l.length ? Math.exp(l.reduce((a, b) => a + b, 0) / l.length) : null,
    }
  })
  return out
}

function quantile(sorted: number[], q: number): number {
  const i = (sorted.length - 1) * q
  const lo = Math.floor(i)
  const hi = Math.min(lo + 1, sorted.length - 1)
  return sorted[lo]! + (sorted[hi]! - sorted[lo]!) * (i - lo)
}

export interface CaseTime {
  median: number
  q1: number
  q3: number
  // How many cases the times are over: the same for every kernel compared.
  cases: number
}

// One kernel's case times over the cases every kernel answered, with a tier or a fail, so each is
// timed on the same work. Null for every kernel when no case was answered by all.
export function caseTimes(reports: Report[]): Record<string, CaseTime | null> {
  const byId = reports.map(r => new Map(r.cases.map(c => [c.id, c])))
  const answered = (c: CaseRow | undefined) =>
    !!c && c.timeMs !== null && (c.tier !== null || c.result === 'fail')
  const common = (reports[0]?.cases ?? []).filter(c => byId.every(m => answered(m.get(c.id))))
  return Object.fromEntries(
    reports.map((r, i) => {
      if (!common.length) return [r.identity.kernel, null]
      const v = common.map(c => byId[i]!.get(c.id)!.timeMs!).sort((a, b) => a - b)
      const time = { median: quantile(v, 0.5), q1: quantile(v, 0.25), q3: quantile(v, 0.75) }
      return [r.identity.kernel, { ...time, cases: v.length }]
    }),
  )
}

// The segment that names a kernel's page in its report's URL: the kernel's name, or name@version
// where the suite holds more than one release of the kernel. `reports` is every report read.
export function kernelSegment(reports: Report[], r: Report): string {
  const releases = reports.filter(
    o => o.methodology.suite === r.methodology.suite && o.identity.kernel === r.identity.kernel,
  )
  return releases.length > 1 ? `${r.identity.kernel}@${r.identity.version}` : r.identity.kernel
}

export interface LeaderRow {
  kernel: string
  // The kernel's page in its report: kernelSegment.
  segment: string
  name: string
  version: string
  suite: string
  date: string
  adapter: Adapter | null
  summary: Summary
  speed: Speed
  time: CaseTime | null
}

// Ranked by the headline alone, ties by name: no kernel gets a place of its own. `all` is every
// report read, which names each kernel's page.
export function leaderboard(latest: Report[], all: Report[] = latest): LeaderRow[] {
  const speed = speedFactors(latest)
  const time = caseTimes(latest)
  return latest
    .map(r => ({
      kernel: r.identity.kernel,
      segment: kernelSegment(all, r),
      name: r.identity.name,
      version: r.identity.version,
      suite: r.methodology.suite,
      date: r.environment.date,
      adapter: r.identity.adapter,
      summary: summarize(r.cases),
      speed: speed[r.identity.kernel]!,
      time: time[r.identity.kernel]!,
    }))
    .sort((a, b) => b.summary.passRate - a.summary.passRate || a.kernel.localeCompare(b.kernel))
}

export interface TestKernel {
  grade: Grade
  summary: Summary
  why: string
  failure: Failure | null
  // The test's median case time, as the suite reports it; null when no case gave one.
  medianMs: number | null
}

export interface TestEntry {
  test: string
  slug: string
  operation: string
  kind: Kind
  cases: string[]
  kernels: Record<string, TestKernel>
}

// A kernel's grade on a test is its worst case's, and the why is that case's.
export function testIndex(latest: Report[]): TestEntry[] {
  const tests = new Map<string, TestEntry>()
  const rows = new Map<string, Map<string, CaseRow[]>>()
  for (const r of latest) {
    for (const c of r.cases) {
      const key = `${c.operation}/${c.slug}`
      if (!tests.has(key)) {
        tests.set(key, {
          test: c.test,
          slug: c.slug,
          operation: c.operation,
          kind: c.kind,
          cases: [],
          kernels: {},
        })
        rows.set(key, new Map())
      }
      const byKernel = rows.get(key)!
      if (!byKernel.has(r.identity.kernel)) byKernel.set(r.identity.kernel, [])
      byKernel.get(r.identity.kernel)!.push(c)
    }
  }
  for (const [key, entry] of tests) {
    const listed = new Set<string>()
    for (const [kernel, cs] of rows.get(key)!) {
      // Every kernel's cases, each once: a case only a later kernel ran is still one of the test's,
      // placed after the case it follows in that kernel's report, so the order is the suite's.
      let prev: string | null = null
      for (const c of cs) {
        if (!listed.has(c.id)) {
          listed.add(c.id)
          entry.cases.splice(prev === null ? 0 : entry.cases.indexOf(prev) + 1, 0, c.id)
        }
        prev = c.id
      }
      const grade = worstGrade(cs.map(c => c.grade))
      const worst = cs.find(c => c.grade === grade)!
      entry.kernels[kernel] = {
        grade,
        summary: summarize(cs),
        why: worst.why,
        failure: worst.failure,
        medianMs: cs.find(c => c.groupMedianMs !== null)?.groupMedianMs ?? null,
      }
    }
  }
  return [...tests.values()].sort(
    (a, b) => a.operation.localeCompare(b.operation) || a.test.localeCompare(b.test),
  )
}

export interface OperationEntry {
  operation: string
  tests: string[]
  kernels: Record<string, { summary: Summary; supported: boolean }>
}

export function operationIndex(latest: Report[]): OperationEntry[] {
  const ops = new Map<string, { tests: Set<string>; cases: Map<string, CaseRow[]> }>()
  for (const r of latest) {
    for (const c of r.cases) {
      const op = ops.get(c.operation) ?? { tests: new Set(), cases: new Map() }
      ops.set(c.operation, op)
      op.tests.add(c.slug)
      const list = op.cases.get(r.identity.kernel) ?? []
      list.push(c)
      op.cases.set(r.identity.kernel, list)
    }
  }
  return [...ops.entries()]
    .map(([operation, { tests, cases }]) => ({
      operation,
      tests: [...tests].sort(),
      kernels: Object.fromEntries(
        [...cases].map(([k, cs]) => [
          k,
          { summary: summarize(cs), supported: cs.some(c => c.result !== 'unsupported') },
        ]),
      ),
    }))
    .sort((a, b) => a.operation.localeCompare(b.operation))
}

// One report of the site: every kernel run on one suite version, at /reports/<suite>/.
export interface SuiteView {
  suite: string
  // Every report on the suite, older releases of a kernel included.
  reports: Report[]
  // Each kernel's latest release on the suite: the ones its pages compare.
  latest: Report[]
  kernels: string[]
  leaderboard: LeaderRow[]
  tests: TestEntry[]
  operations: OperationEntry[]
}

export function suiteView(reports: Report[], suite: string): SuiteView {
  const on = reports.filter(r => r.methodology.suite === suite)
  const latest = latestByKernel(on)
  return {
    suite,
    reports: on,
    latest,
    kernels: latest.map(r => r.identity.kernel),
    leaderboard: leaderboard(latest, reports),
    tests: testIndex(latest),
    operations: operationIndex(latest),
  }
}

// The current suite's view, which the home page shows, and every suite's, newest first.
export interface Site extends SuiteView {
  all: Report[]
  suites: SuiteView[]
}

export function buildSite(reports: Report[]): Site {
  const names = [...new Set(reports.map(r => r.methodology.suite))].sort(compareVersions).reverse()
  const suites = names.map(s => suiteView(reports, s))
  const current = suites.find(v => v.suite === currentSuite(reports)) ?? suiteView([], '')
  return { ...current, all: reports, suites }
}
