import { kernelInfo } from '@/data/kernels'
import { site } from '@/data/source'
import {
  boardSegment,
  compareRuns,
  MACHINE_KEYS,
  runSegment,
  summarize,
  versionSegment,
  type Environment,
  type Identity,
  type Methodology,
  type Report,
  type RunInfo,
  type Summary,
  type SuiteView,
} from '@/data/results'
import type { PageEntry } from '@/site/page'
import { operations } from '../../../operations/doc'
import { packCases, pageKey, totalTimeMs, type PackedCases } from './cases'

export interface Release {
  version: string
  // The report it is in, and its most lasting page there: the leaderboard's, else the release's
  // own, else the run's.
  suite: string
  segment: string
  date: string
  run: string | null
  // This page's own run.
  here: boolean
  // What another run of this page's release did differently: commits, machine, calibration.
  differs: string[]
}

export interface Props {
  identity: Identity
  environment: Environment
  methodology: Methodology
  run: RunInfo
  // False for a kernel kernels.json no longer lists.
  registered: boolean
  summary: Summary
  // The functions the kernel does not offer, each with whether the operations pages document it,
  // and the stand-in sent for each missing constructor.
  functions: { name: string; documented: boolean }[]
  standIns: Record<string, string>
  cases: PackedCases
  // Every published run of this kernel, newest first, this one included.
  releases: Release[]
  // Whether its report's compare pages show this run: its kernel's leaderboard entry on the suite.
  shown: boolean
  // Every case's time added up: the time the run spent in the kernel.
  totalMs: number
  license: string | null
  adapterLicense: string | null
  // The kernel's own maintainers who review its adapter; null for a kernel not in the registry.
  // Where the results page ranks this report, when it is the kernel's release ranked there.
  standing: { place: number; of: number } | null
}

export function entries(): PageEntry<Props>[] {
  const s = site()
  const documented = new Set(operations().operations.map(o => o.id))
  return s.suites.flatMap(v => {
    const pages = new Map(v.tests.map(t => [pageKey(t.operation, t.slug), new Set(t.cases)]))
    return v.reports.flatMap(r => {
      const entry = entryOf(r, v, s.suites, pages, documented)
      return segmentsOf(r, v).map(kernel => ({ ...entry, params: { report: v.suite, kernel } }))
    })
  })
}

// Every page a run's report renders at: its run's for good, its release's while it is that
// release's selected run, and the kernel's while it is the leaderboard's entry.
export function segmentsOf(r: Report, v: SuiteView): string[] {
  return [
    ...(v.latest.includes(r) ? [boardSegment(r)] : []),
    ...(v.selected.includes(r) ? [versionSegment(r)] : []),
    runSegment(r),
  ]
}

// The page a link to a run should name: the most lasting of segmentsOf.
function lastingSegment(r: Report, suites: SuiteView[]): string {
  const v = suites.find(x => x.suite === r.methodology.suite)!
  return segmentsOf(r, v)[0]!
}

// What run `a` of a release did differently from run `b`, as the release list shows it.
export function differences(a: Report, b: Report): string[] {
  const out: string[] = []
  const short = (c: string | null | undefined) => (c ? c.slice(0, 7) : '–')
  if (a.run.suiteCommit !== b.run.suiteCommit)
    out.push(`suite ${short(b.run.suiteCommit)} → ${short(a.run.suiteCommit)}`)
  if (a.identity.adapter?.commit !== b.identity.adapter?.commit)
    out.push(`adapter ${short(b.identity.adapter?.commit)} → ${short(a.identity.adapter?.commit)}`)
  for (const key of MACHINE_KEYS) {
    const [x, y] = [a.run.machine?.[key], b.run.machine?.[key]]
    if (x !== y) out.push(`${key} ${y ?? '–'} → ${x ?? '–'}`)
  }
  const [ca, cb] = [a.run.calibrationMs?.toFixed(1), b.run.calibrationMs?.toFixed(1)]
  if (ca && cb && ca !== cb) out.push(`calibration ${cb} → ${ca} ms`)
  return out
}

function entryOf(
  r: Report,
  v: SuiteView,
  suites: SuiteView[],
  pages: Map<string, Set<string>>,
  documented: Set<string>,
): PageEntry<Props> {
  const id = r.identity
  const summary = summarize(r.cases)
  const releases = suites
    .flatMap(x => x.reports)
    .filter(o => o.identity.kernel === id.kernel)
    .map(o => {
      const sameRelease = o.methodology.suite === v.suite && o.identity.version === id.version
      return {
        version: o.identity.version,
        suite: o.methodology.suite,
        segment: lastingSegment(o, suites),
        date: o.environment.date,
        run: o.run.id,
        here: o === r,
        differs: sameRelease && o !== r ? differences(o, r) : [],
      }
    })
    .sort((a, b) => compareRuns(b, a))
  const at = v.latest.includes(r) ? v.leaderboard.findIndex(l => l.kernel === id.kernel) : -1
  return {
    props: {
      identity: id,
      environment: r.environment,
      methodology: r.methodology,
      run: r.run,
      registered: r.registered,
      summary,
      functions: r.unsupported.map(name => ({ name, documented: documented.has(name) })),
      standIns: r.standIns,
      cases: packCases(r.cases, pages),
      releases,
      shown: v.latest.includes(r),
      totalMs: totalTimeMs(r.cases),
      license: kernelInfo(id.kernel)?.license ?? null,
      adapterLicense: kernelInfo(id.kernel)?.adapter.license ?? null,
      standing: at < 0 ? null : { place: at + 1, of: v.leaderboard.length },
    },
    title: `${id.name} ${id.version}`,
    description:
      `${id.name} ${id.version} on suite ${v.suite}: ` +
      `${(summary.passRate * 100).toFixed(1)}% of ${summary.total} cases pass at 1e-7.`,
  }
}
