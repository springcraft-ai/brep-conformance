// The published runs (reports.json at the repository's root, described in doc/formats.md): which
// runs the site shows, the file of each kernel's report on the CDN, and each suite's leaderboard.
// Pure and without imports, so the scripts run it under Node as well as the site's build.

// One kernel's report in a run.
export interface RunReport {
  // Its name under reports/<run>/ on the CDN: <kernel>-<first 16 hex of its SHA-256>.json.
  file: string
  // Its pass ran every test, so it may be on a leaderboard.
  complete: boolean
  // How long the pass took, for estimating the next run's time.
  seconds: number | null
}

// What a run was asked to cover: every kernel kernels.json listed at its commit, or the ones named.
export const SCOPES = ['all', 'named'] as const
export type Scope = (typeof SCOPES)[number]

export interface RunEntry {
  // The Benchmark workflow's run and attempt.
  run: string
  suite: string
  scope: Scope
  reports: Record<string, RunReport>
}

// A suite's leaderboard: every kernel that ran every test in one run, and kernels whose entry comes
// from another.
export interface Board {
  run: string
  kernels: Record<string, string>
}

export interface RunsIndex {
  runs: RunEntry[]
  leaderboard: Record<string, Board>
}

export const RUN_ID = /^\d+-\d+$/
// A kernel's name and a suite version each name a folder and a page: kernels.ts says why.
const SEGMENT = /^[A-Za-z0-9][A-Za-z0-9._-]*$/
const HASH = /^[0-9a-f]{16}$/

export function reportFile(kernel: string, hash: string): string {
  return `${kernel}-${hash}.json`
}

// The hash a report's file name pins.
export function fileHash(kernel: string, file: string): string | null {
  const prefix = `${kernel}-`
  if (!file.startsWith(prefix) || !file.endsWith('.json')) return null
  const hash = file.slice(prefix.length, -'.json'.length)
  return HASH.test(hash) ? hash : null
}

type Obj = Record<string, unknown>

function obj(v: unknown, at: string): Obj {
  if (typeof v !== 'object' || v === null || Array.isArray(v))
    throw new Error(`${at}: not an object`)
  return v as Obj
}

function text(v: unknown, at: string, pattern: RegExp): string {
  if (typeof v !== 'string' || !pattern.test(v)) throw new Error(`${at}: ${String(v)} is not valid`)
  return v
}

function compare(a: string, b: string): number {
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

// Refuses an index whose leaderboard names a run, kernel or report it does not list, or a report
// that is not complete.
export function parseIndex(raw: unknown, at = 'reports.json'): RunsIndex {
  const o = obj(raw, at)
  if (!Array.isArray(o.runs)) throw new Error(`${at}.runs: not an array`)
  const runs: RunEntry[] = o.runs.map((r, i) => {
    const ra = `${at}.runs[${i}]`
    const e = obj(r, ra)
    const reports = obj(e.reports, `${ra}.reports`)
    if (!SCOPES.includes(e.scope as Scope))
      throw new Error(`${ra}.scope: ${String(e.scope)} is not all or named`)
    return {
      run: text(e.run, `${ra}.run`, RUN_ID),
      suite: text(e.suite, `${ra}.suite`, SEGMENT),
      scope: e.scope as Scope,
      reports: Object.fromEntries(
        Object.entries(reports).map(([kernel, v]) => {
          const ka = `${ra}.reports.${kernel}`
          text(kernel, ka, SEGMENT)
          const k = obj(v, ka)
          const file = text(k.file, `${ka}.file`, /./)
          if (!fileHash(kernel, file))
            throw new Error(`${ka}.file: ${file} is not ${kernel}-<hash>`)
          if (typeof k.complete !== 'boolean') throw new Error(`${ka}.complete: not a boolean`)
          const seconds = k.seconds ?? null
          if (seconds !== null && typeof seconds !== 'number')
            throw new Error(`${ka}.seconds: not a number`)
          return [kernel, { file, complete: k.complete, seconds }]
        }),
      ),
    }
  })
  const byId = new Map<string, RunEntry>()
  for (const r of runs) {
    if (byId.has(r.run)) throw new Error(`${at}: run ${r.run} is listed twice`)
    byId.set(r.run, r)
  }
  const boards = obj(o.leaderboard ?? {}, `${at}.leaderboard`)
  const leaderboard: Record<string, Board> = {}
  for (const [suite, v] of Object.entries(boards)) {
    const ba = `${at}.leaderboard.${suite}`
    const b = obj(v, ba)
    const run = text(b.run, `${ba}.run`, RUN_ID)
    const kernels = obj(b.kernels ?? {}, `${ba}.kernels`) as Record<string, unknown>
    const board: Board = { run, kernels: {} }
    for (const [kernel, id] of Object.entries(kernels))
      board.kernels[kernel] = text(id, `${ba}.kernels.${kernel}`, RUN_ID)
    const check = (id: string, kernel: string | null, where: string) => {
      const entry = byId.get(id)
      if (!entry) throw new Error(`${where}: run ${id} is not in runs`)
      if (entry.suite !== suite) throw new Error(`${where}: run ${id} ran suite ${entry.suite}`)
      if (kernel === null) return
      const report = entry.reports[kernel]
      if (!report) throw new Error(`${where}: run ${id} has no report of ${kernel}`)
      if (!report.complete) throw new Error(`${where}: ${kernel} did not run every test in ${id}`)
    }
    check(run, null, `${ba}.run`)
    for (const [kernel, id] of Object.entries(board.kernels))
      check(id, kernel, `${ba}.kernels.${kernel}`)
    leaderboard[suite] = board
  }
  return { runs, leaderboard }
}

export function emptyIndex(): RunsIndex {
  return { runs: [], leaderboard: {} }
}

// The newest suite version with a leaderboard: the one the home page shows.
export function currentSuite(index: RunsIndex): string {
  return Object.keys(index.leaderboard).sort(compare).at(-1) ?? ''
}

// Each kernel on a suite's leaderboard, and the run its entry comes from.
export function boardRuns(index: RunsIndex, suite: string): Map<string, string> {
  const board = index.leaderboard[suite]
  const out = new Map<string, string>()
  if (!board) return out
  const run = index.runs.find(r => r.run === board.run)!
  for (const [kernel, r] of Object.entries(run.reports)) if (r.complete) out.set(kernel, board.run)
  for (const [kernel, id] of Object.entries(board.kernels)) out.set(kernel, id)
  return out
}

// What publishing a run does to the leaderboard, for the pull request that proposes it.
export type Change =
  | { kind: 'none'; why: string }
  | { kind: 'default'; suite: string; replaced: string | null }
  | { kind: 'overrides'; suite: string; kernels: string[] }
  | { kind: 'listed'; suite: string }

// The index with a run added, and its leaderboard moved as doc/formats.md says. Only a run of every
// kernel creates or replaces a suite's leaderboard, so a kernel that registers or leaves while a
// run goes on changes nothing. A run of named kernels sets those kernels' rows on a suite that has
// a leaderboard, and on one that has none is listed with no leaderboard.
export function addRun(index: RunsIndex, entry: RunEntry): { index: RunsIndex; change: Change } {
  if (index.runs.some(r => r.run === entry.run))
    return { index, change: { kind: 'none', why: `run ${entry.run} is already listed` } }
  const complete = Object.entries(entry.reports)
    .filter(([, r]) => r.complete)
    .map(([kernel]) => kernel)
    .sort()
  if (!complete.length)
    return {
      index,
      change: { kind: 'none', why: `no kernel of run ${entry.run} ran every test` },
    }
  const next: RunsIndex = {
    runs: [...index.runs, entry],
    leaderboard: { ...index.leaderboard },
  }
  const board = index.leaderboard[entry.suite]
  if (entry.scope === 'all') {
    next.leaderboard[entry.suite] = { run: entry.run, kernels: {} }
    return {
      index: next,
      change: { kind: 'default', suite: entry.suite, replaced: board?.run ?? null },
    }
  }
  if (!board) return { index: next, change: { kind: 'listed', suite: entry.suite } }
  const kernels = { ...board.kernels }
  for (const k of complete) kernels[k] = entry.run
  next.leaderboard[entry.suite] = { run: board.run, kernels }
  return { index: next, change: { kind: 'overrides', suite: entry.suite, kernels: complete } }
}

// reports.json as the repository keeps it: two-space indent, a closing newline.
export function formatIndex(index: RunsIndex): string {
  return JSON.stringify(index, null, 2) + '\n'
}
