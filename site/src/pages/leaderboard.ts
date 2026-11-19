import { kernelInfo } from '@/data/kernels'
import { GRADES, type CaseTime, type Grade, type SuiteView } from '@/data/results'

export interface LeaderboardKernel {
  kernel: string
  version: string
  // An SPDX expression; null for a kernel the registry does not list.
  license: string | null
  // The suite's maintainer makes the kernel.
  maintainers: boolean
  // Case counts in `grades` order.
  counts: number[]
  // Case times in milliseconds over the cases every kernel answered; null with no such case.
  time: CaseTime | null
  run: {
    // When the run ended, as an ISO date and time; empty when the report has no time.
    date: string
    os: string
    arch: string
    cores: number | null
    tolerance: number | null
    verified: boolean
  }
}

// The current leaderboard as data, for a page elsewhere to draw. Kernels are in rank order.
export interface Leaderboard {
  suite: string
  cases: number
  grades: readonly Grade[]
  kernels: LeaderboardKernel[]
}

export function leaderboardOf(v: SuiteView): Leaderboard {
  return {
    suite: v.suite,
    cases: v.leaderboard[0]?.summary.total ?? 0,
    grades: GRADES,
    kernels: v.leaderboard.map(row => {
      const report = v.latest.find(r => r.identity.kernel === row.kernel)!
      const info = kernelInfo(row.kernel)
      const env = report.environment
      return {
        kernel: row.kernel,
        version: row.version,
        license: info?.license ?? null,
        maintainers: info?.maintainers ?? false,
        counts: GRADES.map(g => row.summary.grades[g]),
        time: row.time,
        run: {
          date: env.date,
          os: env.os,
          arch: env.arch,
          cores: env.cores,
          tolerance: report.identity.declaredTolerance,
          verified: env.verified,
        },
      }
    }),
  }
}
