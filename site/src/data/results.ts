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

// The report's folder on the suite's CDN, without its closing slash: null where the run wrote none.
// A report is its kernel's own word, so `assets` must be the report's own folder on the suite's
// CDN: no other host, and no other kernel's folder.
function cdnFolder(
  assets: unknown,
  suite: string,
  kernel: string,
  version: string,
  at: string,
): string | null {
  if (assets === null) return null
  const own = `${ASSET_BASE}/${suite}/${kernel}/${version}/`
  if (assets !== undefined && assets !== own)
    throw new Error(`${at}.assets: ${String(assets)} is not ${own}`)
  return own.slice(0, -1)
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
  // The B17 round trip through STEP, which timeMs leaves out; null where none ran.
  stepMs: number | null
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

// What the benchmark session recorded about the run; each field null for a run made elsewhere.
export interface RunInfo {
  // The Benchmark workflow's run and attempt, which reports.json lists.
  id: string | null
  // The pass ran every test.
  complete: boolean | null
  suiteCommit: string | null
  // The machine's settings, as environment.txt gives them.
  machine: Record<string, string> | null
  calibrationMs: number | null
}

export interface Report {
  identity: Identity
  // The kernel is in kernels.json. A kernel that left keeps its published reports, and is on no
  // current leaderboard.
  registered: boolean
  environment: Environment
  methodology: Methodology
  run: RunInfo
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
    stepMs: numOrNull(o, 'step_ms', at),
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

// `folder` is where the report's files load from in place of its CDN folder, such as a local copy.
export function parseReport(raw: unknown, at = 'report', folder?: string): Report {
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
  const cdn = cdnFolder(o.assets, suiteVersion, name, version, at)
  const assetBase = cdn === null ? null : (folder ?? cdn)
  const machine =
    env.machine === undefined || env.machine === null
      ? null
      : Object.fromEntries(
          Object.entries(obj(env.machine, `${at}.environment.machine`)).map(([a, b]) => [
            a,
            String(b),
          ]),
        )
  return {
    registered: true,
    run: {
      id: typeof run.id === 'string' ? run.id : null,
      complete: typeof run.complete === 'boolean' ? run.complete : null,
      suiteCommit: typeof suite.commit === 'string' ? suite.commit : null,
      machine,
      calibrationMs: numOrNull(run, 'calibration_ms', `${at}.run`),
    },
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

// Each version's own page: the leaderboard's run of it where there is one, else its newest run.
export function selectedRuns(reports: Report[], board: Report[]): Report[] {
  const best = new Map<string, Report>()
  for (const r of reports) {
    const key = `${r.identity.kernel}@${r.identity.version}`
    const prev = best.get(key)
    if (!prev || r.environment.date > prev.environment.date) best.set(key, r)
  }
  for (const r of board) best.set(`${r.identity.kernel}@${r.identity.version}`, r)
  return [...best.values()]
}

// The segments that name a kernel's pages in its report's URL. The kernel's name is its leaderboard
// entry and never moves; name@version is that version's own page; name@version~run is one run,
// for good.
export function boardSegment(r: Report): string {
  return r.identity.kernel
}

export function versionSegment(r: Report): string {
  return `${r.identity.kernel}@${r.identity.version}`
}

export function runSegment(r: Report): string {
  return `${versionSegment(r)}~${r.run.id ?? 'unlisted'}`
}

// The machine settings a leaderboard's rows must share: a row from another run than the board's
// own is comparable only on the same machine and image.
export const MACHINE_KEYS = ['instance_type', 'ami_id', 'kernel'] as const

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
  // How many cases the times are over: the kernel's passed cases.
  cases: number
  // The median of the round trips through STEP among those cases; null where none ran one.
  step: number | null
}

// A kernel's case times over its own passed cases, so no other kernel's results move them. Null
// when it passed no timed case.
export function caseTime(r: Report): CaseTime | null {
  const v = r.cases
    .filter(c => c.result === '1e-7' && c.timeMs !== null)
    .map(c => c.timeMs!)
    .sort((a, b) => a - b)
  if (!v.length) return null
  const steps = r.cases
    .filter(c => c.result === '1e-7' && c.timeMs !== null && c.stepMs !== null)
    .map(c => c.stepMs!)
    .sort((a, b) => a - b)
  return {
    median: quantile(v, 0.5),
    q1: quantile(v, 0.25),
    q3: quantile(v, 0.75),
    cases: v.length,
    step: steps.length ? quantile(steps, 0.5) : null,
  }
}

export interface LeaderRow {
  kernel: string
  // The kernel's page in its report: boardSegment.
  segment: string
  name: string
  version: string
  suite: string
  date: string
  adapter: Adapter | null
  summary: Summary
  time: CaseTime | null
  // The run the row comes from.
  run: string | null
}

// Ordered by the headline, the 1e-7 pass rate; ties by the median time of the passed cases, faster
// first, a kernel with no time last; then by name.
export function leaderboard(latest: Report[]): LeaderRow[] {
  return latest
    .map(r => ({
      kernel: r.identity.kernel,
      segment: boardSegment(r),
      name: r.identity.name,
      version: r.identity.version,
      suite: r.methodology.suite,
      date: r.environment.date,
      adapter: r.identity.adapter,
      summary: summarize(r.cases),
      time: caseTime(r),
      run: r.run.id,
    }))
    .sort(
      (a, b) =>
        b.summary.passRate - a.summary.passRate ||
        (a.time?.median ?? Infinity) - (b.time?.median ?? Infinity) ||
        a.kernel.localeCompare(b.kernel),
    )
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
  // Every published report on the suite, older releases and other runs of a kernel included.
  reports: Report[]
  // Each kernel's leaderboard entry on the suite: the ones its pages compare.
  latest: Report[]
  // Each kernel release's own page: selectedRuns.
  selected: Report[]
  // The run most of the leaderboard comes from; null on a suite with no leaderboard.
  boardRun: string | null
  kernels: string[]
  leaderboard: LeaderRow[]
  tests: TestEntry[]
  operations: OperationEntry[]
}

// A suite's leaderboard as reports.json names it (runs.ts boardRuns): each kernel and its run.
// Refuses a row from another run than the board's own on a different machine or image, and a row
// whose report did not run every test.
export function suiteView(
  reports: Report[],
  suite: string,
  board: Map<string, string> = new Map(),
  boardRun: string | null = null,
): SuiteView {
  const on = reports.filter(r => r.methodology.suite === suite)
  const find = (kernel: string, run: string) =>
    on.find(r => r.identity.kernel === kernel && r.run.id === run)
  const latest: Report[] = []
  for (const [kernel, run] of board) {
    const r = find(kernel, run)
    if (!r) throw new Error(`suite ${suite}: no report of ${kernel} from run ${run}`)
    if (r.run.complete !== true)
      throw new Error(`suite ${suite}: ${kernel} did not run every test in run ${run}`)
    latest.push(r)
  }
  latest.sort((a, b) => a.identity.kernel.localeCompare(b.identity.kernel))
  const own = on.find(r => r.run.id === boardRun)
  for (const r of latest) {
    if (r.run.id === boardRun) continue
    const where = `suite ${suite}: ${r.identity.kernel} from run ${r.run.id}`
    if (!own?.run.machine || !r.run.machine)
      throw new Error(`${where}: no machine record to compare with run ${boardRun}`)
    for (const key of MACHINE_KEYS) {
      if (own.run.machine[key] !== r.run.machine[key])
        throw new Error(
          `${where}: ${key} ${r.run.machine[key]} is not run ${boardRun}'s ${own.run.machine[key]}`,
        )
    }
  }
  return {
    suite,
    reports: on,
    latest,
    selected: selectedRuns(on, latest),
    boardRun,
    kernels: latest.map(r => r.identity.kernel),
    leaderboard: leaderboard(latest),
    tests: testIndex(latest),
    operations: operationIndex(latest),
  }
}

// The current suite's view, which the home page shows, and every suite's, newest first.
export interface Site extends SuiteView {
  all: Report[]
  suites: SuiteView[]
}

// `boards` is each suite's leaderboard (runs.ts boardRuns) and the run it is drawn from; `current`
// is the suite the home page shows.
export function buildSite(
  reports: Report[],
  boards: Map<string, { run: string; kernels: Map<string, string> }> = new Map(),
  current = '',
): Site {
  const names = [...new Set([...reports.map(r => r.methodology.suite), ...boards.keys()])]
    .sort(compareVersions)
    .reverse()
  const suites = names.map(s => {
    const b = boards.get(s)
    return suiteView(reports, s, b?.kernels, b?.run ?? null)
  })
  const home = suites.find(v => v.suite === current) ?? suiteView([], '')
  return { ...home, all: reports, suites }
}
