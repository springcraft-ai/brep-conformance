import { describe, expect, it } from 'vitest'
import { GRADES, summarize } from '@/data/results'
import { site } from '@/data/source'
import { leaderboardOf } from './leaderboard'

describe('the leaderboard as data', () => {
  const s = site()
  const board = leaderboardOf(s)

  it('lists the current suite’s kernels in rank order', () => {
    expect(board.suite).toBe(s.suite)
    expect(board.kernels.length).toBeGreaterThan(0)
    expect(board.kernels.map(k => k.kernel)).toEqual(s.leaderboard.map(r => r.kernel))
  })

  it('counts every case of a kernel once, in the order it names', () => {
    expect(board.grades).toEqual(GRADES)
    for (const k of board.kernels) {
      const report = s.latest.find(r => r.identity.kernel === k.kernel)!
      const grades = summarize(report.cases).grades
      expect(k.counts).toEqual(GRADES.map(g => grades[g]))
      expect(k.counts.reduce((a, b) => a + b, 0)).toBe(report.cases.length)
      expect(k.version).toBe(report.identity.version)
      expect(k.run.date).toBe(report.environment.date)
    }
  })

  it('is empty with no report', () => {
    const none = leaderboardOf({ ...s, suite: '', latest: [], leaderboard: [] })
    expect(none).toMatchObject({ cases: 0, kernels: [] })
  })
})
