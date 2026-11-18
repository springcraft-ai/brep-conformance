import { kernelInfo } from '@/data/kernels'
import { site } from '@/data/source'
import {
  compareRuns,
  summarize,
  type Environment,
  type Identity,
  type Methodology,
  type Summary,
} from '@/data/results'
import type { PageEntry } from '@/site/page'
import { operations } from '../../../../operations/doc'
import { packCases, pageKey, totalTimeMs, type PackedCases } from './cases'

export interface Release {
  version: string
  suite: string
  date: string
}

export interface Props {
  identity: Identity
  environment: Environment
  methodology: Methodology
  summary: Summary
  totalMs: number
  // The functions the kernel does not offer, each with whether the operations pages document it,
  // and the stand-in sent for each missing constructor.
  functions: { name: string; documented: boolean }[]
  standIns: Record<string, string>
  cases: PackedCases
  // Every report of this kernel, newest first, this one included.
  releases: Release[]
  // The release the leaderboard and the tests' pages show for this kernel, when it is not this one.
  current: Release | null
  license: string | null
  adapterLicense: string | null
  // The kernel's own maintainers who review its adapter; null for a kernel not in the registry.
  reviewers: string[] | null
  // Where the results page ranks this report, when it is the kernel's release ranked there.
  standing: { place: number; of: number; speedFactor: number | null } | null
}

export function entries(): PageEntry<Props>[] {
  const s = site()
  const standingOf = (suite: string, kernel: string, version: string): Props['standing'] => {
    const at = s.leaderboard.findIndex(
      l => l.suite === suite && l.kernel === kernel && l.version === version,
    )
    if (at < 0) return null
    return {
      place: at + 1,
      of: s.leaderboard.length,
      speedFactor: s.leaderboard[at]!.speed.factor,
    }
  }
  const pages = new Map(s.tests.map(t => [pageKey(t.operation, t.slug), new Set(t.cases)]))
  const documented = new Set(operations().operations.map(o => o.id))
  return s.reports.map(r => {
    const id = r.identity
    const summary = summarize(r.cases)
    const releases = s.reports
      .filter(o => o.identity.kernel === id.kernel)
      .map(o => ({
        version: o.identity.version,
        suite: o.methodology.suite,
        date: o.environment.date,
      }))
      .sort((a, b) => compareRuns(b, a))
    const suite = r.methodology.suite
    const shown = s.latest.find(l => l.identity.kernel === id.kernel)
    const isShown = shown?.methodology.suite === suite && shown.identity.version === id.version
    return {
      params: { suite, kernel: id.kernel, version: id.version },
      props: {
        identity: id,
        environment: r.environment,
        methodology: r.methodology,
        summary,
        totalMs: totalTimeMs(r.cases),
        functions: r.unsupported.map(name => ({ name, documented: documented.has(name) })),
        standIns: r.standIns,
        cases: packCases(r.cases, pages),
        releases,
        current:
          !shown || isShown
            ? null
            : {
                version: shown.identity.version,
                suite: shown.methodology.suite,
                date: shown.environment.date,
              },
        license: kernelInfo(id.kernel)?.license ?? null,
        adapterLicense: kernelInfo(id.kernel)?.adapter.license ?? null,
        reviewers: kernelInfo(id.kernel)?.reviewers ?? null,
        standing: standingOf(suite, id.kernel, id.version),
      },
      title: `${id.name} ${id.version}`,
      description:
        `${id.name} ${id.version} on suite ${r.methodology.suite}: ` +
        `${(summary.passRate * 100).toFixed(1)}% of ${summary.total} cases pass at 1e-7.`,
    }
  })
}
