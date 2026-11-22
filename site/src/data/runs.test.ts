import { describe, expect, it } from 'vitest'
import {
  addRun,
  boardRuns,
  currentSuite,
  emptyIndex,
  fileHash,
  parseIndex,
  type RunEntry,
} from './runs'

const H = '0123456789abcdef'

function run(
  id: string,
  suite: string,
  kernels: Record<string, boolean>,
  scope: 'all' | 'named' = 'all',
): RunEntry {
  return {
    run: id,
    suite,
    scope,
    reports: Object.fromEntries(
      Object.entries(kernels).map(([k, complete]) => [
        k,
        { file: `${k}-${H}.json`, complete, seconds: 60 },
      ]),
    ),
  }
}

describe('parseIndex', () => {
  const full = run('10-1', '2.0.0', { alpha: true, beta: true, gamma: true })

  it('reads runs and a leaderboard with an override', () => {
    const later = run('11-1', '2.0.0', { beta: true }, 'named')
    const index = parseIndex({
      runs: [full, later],
      leaderboard: { '2.0.0': { run: '10-1', kernels: { beta: '11-1' } } },
    })
    expect([...boardRuns(index, '2.0.0')]).toEqual([
      ['alpha', '10-1'],
      ['beta', '11-1'],
      ['gamma', '10-1'],
    ])
  })

  it('leaves a kernel that did not run every test off the board of its run', () => {
    const partial = run('12-1', '2.0.0', { alpha: true, beta: false })
    const index = parseIndex({ runs: [partial], leaderboard: { '2.0.0': { run: '12-1' } } })
    expect([...boardRuns(index, '2.0.0').keys()]).toEqual(['alpha'])
  })

  it('refuses a leaderboard naming a run, suite or report it does not list', () => {
    expect(() => parseIndex({ runs: [full], leaderboard: { '2.0.0': { run: '9-1' } } })).toThrow(
      /run 9-1 is not in runs/,
    )
    expect(() => parseIndex({ runs: [full], leaderboard: { '1.0.0': { run: '10-1' } } })).toThrow(
      /ran suite 2\.0\.0/,
    )
    const over = { '2.0.0': { run: '10-1', kernels: { delta: '10-1' } } }
    expect(() => parseIndex({ runs: [full], leaderboard: over })).toThrow(/no report of delta/)
  })

  it('refuses an override whose report did not run every test', () => {
    const partial = run('13-1', '2.0.0', { beta: false }, 'named')
    const over = { '2.0.0': { run: '10-1', kernels: { beta: '13-1' } } }
    expect(() => parseIndex({ runs: [full, partial], leaderboard: over })).toThrow(
      /did not run every test/,
    )
  })

  it('refuses a run listed twice, a bad run id or scope, and a file that pins no hash', () => {
    expect(() => parseIndex({ runs: [full, full] })).toThrow(/listed twice/)
    expect(() => parseIndex({ runs: [{ ...full, scope: 'some' }] })).toThrow(/not all or named/)
    expect(() => parseIndex({ runs: [{ ...full, run: '10' }] })).toThrow(/is not valid/)
    const bad = run('14-1', '2.0.0', { alpha: true })
    bad.reports.alpha!.file = 'alpha.json'
    expect(() => parseIndex({ runs: [bad] })).toThrow(/is not alpha-<hash>/)
  })

  it('takes the newest suite with a leaderboard as current', () => {
    const old = run('1-1', '1.10.0', { alpha: true })
    const newer = run('2-1', '1.9.0', { alpha: true })
    const unboarded = run('3-1', '2.0.0', { alpha: true })
    const index = parseIndex({
      runs: [old, newer, unboarded],
      leaderboard: { '1.10.0': { run: '1-1' }, '1.9.0': { run: '2-1' } },
    })
    expect(currentSuite(index)).toBe('1.10.0')
  })
})

describe('fileHash', () => {
  it('reads the hash a file name pins', () => {
    expect(fileHash('next.BREP', `next.BREP-${H}.json`)).toBe(H)
    expect(fileHash('next.BREP', `next.BREP-${H}.json.gz`)).toBeNull()
    expect(fileHash('alpha', `beta-${H}.json`)).toBeNull()
  })
})

describe('addRun', () => {
  const full = run('10-1', '2.0.0', { alpha: true, beta: true, gamma: true })
  const base = addRun(emptyIndex(), full).index

  it('makes a run of every kernel the first leaderboard of its suite', () => {
    const { index, change } = addRun(emptyIndex(), full)
    expect(change).toEqual({ kind: 'default', suite: '2.0.0', replaced: null })
    expect(index.leaderboard['2.0.0']).toEqual({ run: '10-1', kernels: {} })
  })

  it('lists a run of named kernels on a suite with no leaderboard, and makes it none', () => {
    const { index, change } = addRun(base, run('11-1', '2.0.1', { delta: true }, 'named'))
    expect(change).toEqual({ kind: 'listed', suite: '2.0.1' })
    expect(index.runs.map(r => r.run)).toEqual(['10-1', '11-1'])
    expect(index.leaderboard['2.0.1']).toBeUndefined()
  })

  it('replaces the leaderboard with a run of every kernel, clearing rows from other runs', () => {
    const one = addRun(base, run('11-1', '2.0.0', { beta: true }, 'named')).index
    const again = run('12-1', '2.0.0', { alpha: true, beta: true, gamma: false })
    const { index, change } = addRun(one, again)
    expect(change).toEqual({ kind: 'default', suite: '2.0.0', replaced: '10-1' })
    expect(index.leaderboard['2.0.0']).toEqual({ run: '12-1', kernels: {} })
    // A kernel that did not run every test in it is left off, not carried over from before.
    expect([...boardRuns(index, '2.0.0').keys()]).toEqual(['alpha', 'beta'])
  })

  it('sets only the named kernels’ rows, for a retest or a kernel joining', () => {
    const { index, change } = addRun(
      base,
      run('11-1', '2.0.0', { beta: true, delta: true }, 'named'),
    )
    expect(change).toEqual({ kind: 'overrides', suite: '2.0.0', kernels: ['beta', 'delta'] })
    expect(index.leaderboard['2.0.0']).toEqual({
      run: '10-1',
      kernels: { beta: '11-1', delta: '11-1' },
    })
    expect([...boardRuns(index, '2.0.0').keys()].sort()).toEqual([
      'alpha',
      'beta',
      'delta',
      'gamma',
    ])
    expect(() => parseIndex(index)).not.toThrow()
  })

  it('proposes nothing for a run with no complete report, or one already listed', () => {
    expect(addRun(base, run('11-1', '2.0.0', { beta: false }, 'named')).change.kind).toBe('none')
    expect(addRun(base, run('12-1', '2.0.0', { beta: false })).change.kind).toBe('none')
    expect(addRun(base, full).change.kind).toBe('none')
  })
})
