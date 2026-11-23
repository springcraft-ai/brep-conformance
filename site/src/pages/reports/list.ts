import { runDays } from '@/components/format'
import { compareVersions, type SuiteView } from '@/data/results'

// One published report, a suite version every kernel was run on, as the list of reports shows it.
export interface ReportRow {
  suite: string
  // The report the home page shows.
  current: boolean
  // The days its runs ended, earliest and latest, as ISO dates; one where they agree, none with no
  // dated run.
  runs: string[]
  kernels: number
  cases: number
}

// Every report, newest run first: by the last day any of its runs ended, then by suite version,
// newest first. A report with no dated run goes after every dated one.
export function reportRows(suites: SuiteView[], current: string): ReportRow[] {
  const rows = suites.map(v => {
    const top = v.leaderboard[0]
    return {
      suite: v.suite,
      current: v.suite === current,
      runs: runDays(v.leaderboard.map(r => r.date)),
      kernels: v.leaderboard.length,
      cases: top?.summary.total ?? 0,
    }
  })
  const last = (r: ReportRow) => r.runs[r.runs.length - 1] ?? ''
  return rows.sort((a, b) => {
    const da = last(a)
    const db = last(b)
    if (da !== db) return da < db ? 1 : -1
    return compareVersions(b.suite, a.suite)
  })
}
