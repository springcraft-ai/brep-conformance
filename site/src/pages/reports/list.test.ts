import { describe, expect, it } from 'vitest'
import type { SuiteView } from '@/data/results'
import { reportRows } from './list'

// A suite with a kernel run on each date, ranked as given, each passing `rate` of 10 cases.
function view(suite: string, runs: [kernel: string, date: string, rate: number][]): SuiteView {
  return {
    suite,
    leaderboard: runs.map(([kernel, date, rate]) => ({
      kernel,
      date,
      summary: { total: 10, passRate: rate },
    })),
  } as unknown as SuiteView
}

describe('reportRows', () => {
  it('lists the newest run first, whatever the suite version', () => {
    const rows = reportRows(
      [
        view('2.0.0', [['a', '2026-11-01T10:00:00Z', 0.9]]),
        view('1.0.0', [
          ['a', '2026-10-01T10:00:00Z', 0.8],
          ['b', '2026-12-01T10:00:00Z', 0.5],
        ]),
        view('0.9.0', [['a', '2026-09-01T10:00:00Z', 0.7]]),
      ],
      '2.0.0',
    )
    expect(rows.map(r => r.suite)).toEqual(['1.0.0', '2.0.0', '0.9.0'])
    expect(rows[0]!.runs).toEqual(['2026-10-01', '2026-12-01'])
    expect(rows.map(r => r.current)).toEqual([false, true, false])
  })

  it('breaks a tie on the day by version, and puts a report with no dated run last', () => {
    const rows = reportRows(
      [
        view('1.0.0', [['a', '', 1]]),
        view('1.1.0', [['a', '2026-10-01T10:00:00Z', 1]]),
        view('1.2.0', [['a', '2026-10-01T23:00:00Z', 1]]),
      ],
      '1.2.0',
    )
    expect(rows.map(r => r.suite)).toEqual(['1.2.0', '1.1.0', '1.0.0'])
    expect(rows[2]!.runs).toEqual([])
  })

  it('names the first kernel on the leaderboard and counts the kernels and cases', () => {
    const [row] = reportRows(
      [
        view('1.0.0', [
          ['a', '2026-10-01', 0.9],
          ['b', '2026-10-02', 0.4],
        ]),
      ],
      '1.0.0',
    )
    expect(row).toMatchObject({ kernels: 2, cases: 10, leader: { kernel: 'a', rate: 0.9 } })
    expect(reportRows([view('1.0.0', [])], '1.0.0')[0]!.leader).toBeNull()
  })
})
