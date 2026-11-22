import { createHash } from 'node:crypto'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { boardsOf, readPublished } from './source'

const FIXTURES = 'src/fixtures/reports'
const fixtureIndex = JSON.parse(readFileSync(join(FIXTURES, 'reports.json'), 'utf8'))
const ALPHA = JSON.parse(
  readFileSync(join(FIXTURES, '1-1', fixtureIndex.runs[0].reports.alpha.file), 'utf8'),
)

type Raw = Record<string, any>

// A reports folder holding one run of the given reports, each filed under its own hash unless
// `file` names another.
function published(dir: string, reports: Record<string, Raw>, file?: string): string {
  const run = '7-1'
  const listed: Record<string, unknown> = {}
  for (const [kernel, report] of Object.entries(reports)) {
    const text = JSON.stringify(report)
    const hash = createHash('sha256').update(text).digest('hex').slice(0, 16)
    const name = file ?? `${kernel}-${hash}.json`
    mkdirSync(join(dir, run), { recursive: true })
    writeFileSync(join(dir, run, name), text)
    listed[kernel] = { file: name, complete: report.run.complete, seconds: null }
  }
  const index = { runs: [{ run, suite: '1.0.0', scope: 'all', reports: listed }], leaderboard: {} }
  writeFileSync(join(dir, 'reports.json'), JSON.stringify(index))
  return dir
}

function as(kernel: string, changes: Raw = {}): Raw {
  return {
    ...ALPHA,
    kernel: { ...ALPHA.kernel, name: kernel },
    run: { ...ALPHA.run, id: '7-1' },
    assets: null,
    ...changes,
  }
}

describe('readPublished', () => {
  let dir = ''
  afterEach(() => rmSync(dir, { recursive: true, force: true }))
  const fresh = () => (dir = mkdtempSync(join(tmpdir(), 'brep-reports-')))

  it('reads every report reports.json lists, with its run', () => {
    const { index, reports } = readPublished(FIXTURES)
    expect(index.runs.map(r => r.run)).toEqual(['1-1'])
    expect(reports.map(r => [r.identity.kernel, r.run.id, r.run.complete])).toEqual([
      ['alpha', '1-1', true],
      ['beta', '1-1', true],
    ])
    expect(reports[0]!.run.machine?.instance_type).toBe('m1.test')
  })

  it('refuses a file whose contents are not what its name pins', () => {
    published(fresh(), { alpha: as('alpha') }, 'alpha-0000000000000000.json')
    expect(() => readPublished(dir)).toThrow(/hash to [0-9a-f]{16}, not the name/)
  })

  it('refuses a report of another run, kernel or suite than its entry says', () => {
    published(fresh(), { alpha: as('alpha', { run: { ...ALPHA.run, id: '8-1' } }) })
    expect(() => readPublished(dir)).toThrow(/says run 8-1/)
    rmSync(dir, { recursive: true, force: true })
    published(fresh(), { alpha: as('beta') })
    expect(() => readPublished(dir)).toThrow(/is beta's, not alpha's/)
    rmSync(dir, { recursive: true, force: true })
    published(fresh(), { alpha: as('alpha', { suite: { ...ALPHA.suite, version: '9.9.9' } }) })
    expect(() => readPublished(dir)).toThrow(/ran suite 9\.9\.9, not 1\.0\.0/)
  })

  it('refuses a listed file that was not fetched', () => {
    published(fresh(), { alpha: as('alpha') })
    rmSync(join(dir, '7-1'), { recursive: true })
    expect(() => readPublished(dir)).toThrow(/not fetched/)
  })

  it('reads a kernel the registry lists, and keeps one that left it, marked', () => {
    const repo = 'https://github.com/springcraft-ai/brep-conformance-springrep'
    const commit = '0123456789abcdef0123456789abcdef01234567'
    published(fresh(), { springrep: as('springrep', { adapter: { repo, commit } }) })
    const [r] = readPublished(dir, false).reports
    expect(r!.identity.adapter).toEqual({ repo, commit })
    expect(r!.registered).toBe(true)
    rmSync(dir, { recursive: true, force: true })
    published(fresh(), { alpha: as('alpha') })
    expect(readPublished(dir, false).reports.map(r => [r.identity.kernel, r.registered])).toEqual([
      ['alpha', false],
    ])
  })

  it('refuses an official run of an adapter from another repository than the registry names', () => {
    const adapter = { repo: 'https://example.com/elsewhere', commit: '0123' }
    published(fresh(), { springrep: as('springrep', { adapter }) })
    expect(() => readPublished(dir, false)).toThrow(/not https:/)
  })

  it('takes a kernel that left kernels.json off the current leaderboard only', () => {
    const report = (kernel: string, suite: string, registered: boolean) => ({
      ...readPublished(FIXTURES).reports[0]!,
      identity: { ...readPublished(FIXTURES).reports[0]!.identity, kernel },
      methodology: { ...readPublished(FIXTURES).reports[0]!.methodology, suite },
      registered,
    })
    const entry = (run: string, suite: string) => ({
      run,
      suite,
      scope: 'all' as const,
      reports: {
        kept: { file: `kept-${'0'.repeat(16)}.json`, complete: true, seconds: null },
        gone: { file: `gone-${'0'.repeat(16)}.json`, complete: true, seconds: null },
      },
    })
    const index = {
      runs: [entry('1-1', '1.0.0'), entry('2-1', '2.0.0')],
      leaderboard: { '1.0.0': { run: '1-1', kernels: {} }, '2.0.0': { run: '2-1', kernels: {} } },
    }
    const reports = [report('kept', '2.0.0', true), report('gone', '2.0.0', false)]
    const boards = boardsOf(index, reports)
    expect([...boards.get('2.0.0')!.kernels.keys()]).toEqual(['kept'])
    expect([...boards.get('1.0.0')!.kernels.keys()]).toEqual(['kept', 'gone'])
  })

  it('builds no results without a reports folder', () => {
    expect(readPublished(null).reports).toEqual([])
  })
})
