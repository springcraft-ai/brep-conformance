import { kernelInfo } from '@/data/kernels'
import { site } from '@/data/source'
import {
  compareRuns,
  kernelSegment,
  summarize,
  type Environment,
  type Identity,
  type Methodology,
  type Report,
  type Summary,
} from '@/data/results'
import type { PageEntry } from '@/site/page'
import { operations } from '../../../operations/doc'
import { packCases, pageKey, totalTimeMs, type PackedCases } from './cases'

export interface Release {
  version: string
  // The report it is in, and its page there: kernelSegment.
  suite: string
  segment: string
  date: string
}

export interface Props {
  identity: Identity
  environment: Environment
  methodology: Methodology
  summary: Summary
  // The functions the kernel does not offer, each with whether the operations pages document it,
  // and the stand-in sent for each missing constructor.
  functions: { name: string; documented: boolean }[]
  standIns: Record<string, string>
  cases: PackedCases
  // Every report of this kernel, newest first, this one included.
  releases: Release[]
  // Whether its report's compare pages show this release: its kernel's latest on the suite.
  shown: boolean
  // Every case's time added up: the time the run spent in the kernel.
  totalMs: number
  license: string | null
  adapterLicense: string | null
  // The kernel's own maintainers who review its adapter; null for a kernel not in the registry.
  // Where the results page ranks this report, when it is the kernel's release ranked there.
  standing: { place: number; of: number; speedFactor: number | null } | null
}

export function entries(): PageEntry<Props>[] {
  const s = site()
  const documented = new Set(operations().operations.map(o => o.id))
  return s.suites.flatMap(v => {
    const standingOf = (kernel: string, version: string): Props['standing'] => {
      const at = v.leaderboard.findIndex(l => l.kernel === kernel && l.version === version)
      if (at < 0) return null
      return {
        place: at + 1,
        of: v.leaderboard.length,
        speedFactor: v.leaderboard[at]!.speed.factor,
      }
    }
    const pages = new Map(v.tests.map(t => [pageKey(t.operation, t.slug), new Set(t.cases)]))
    return v.reports.map(r => entryOf(r, v.suite, v.latest, s.all, pages, documented, standingOf))
  })
}

function entryOf(
  r: Report,
  suite: string,
  latest: Report[],
  all: Report[],
  pages: Map<string, Set<string>>,
  documented: Set<string>,
  standingOf: (kernel: string, version: string) => Props['standing'],
): PageEntry<Props> {
  {
    const id = r.identity
    const summary = summarize(r.cases)
    const releases = all
      .filter(o => o.identity.kernel === id.kernel)
      .map(o => ({
        version: o.identity.version,
        suite: o.methodology.suite,
        segment: kernelSegment(all, o),
        date: o.environment.date,
      }))
      .sort((a, b) => compareRuns(b, a))
    const shown = latest.find(l => l.identity.kernel === id.kernel)
    const isShown = shown?.identity.version === id.version
    return {
      params: { report: suite, kernel: kernelSegment(all, r) },
      props: {
        identity: id,
        environment: r.environment,
        methodology: r.methodology,
        summary,
        functions: r.unsupported.map(name => ({ name, documented: documented.has(name) })),
        standIns: r.standIns,
        cases: packCases(r.cases, pages),
        releases,
        shown: isShown,
        totalMs: totalTimeMs(r.cases),
        license: kernelInfo(id.kernel)?.license ?? null,
        adapterLicense: kernelInfo(id.kernel)?.adapter.license ?? null,
        standing: standingOf(id.kernel, id.version),
      },
      title: `${id.name} ${id.version}`,
      description:
        `${id.name} ${id.version} on suite ${suite}: ` +
        `${(summary.passRate * 100).toFixed(1)}% of ${summary.total} cases pass at 1e-7.`,
    }
  }
}
