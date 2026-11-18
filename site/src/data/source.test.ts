import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { readReports } from './source'

const SUITE = '1.0.0'
const ALPHA = `src/fixtures/reports/${SUITE}/alpha`

describe('readReports', () => {
  let dir = ''
  afterEach(() => rmSync(dir, { recursive: true, force: true }))

  it('skips plain files beside the suite, kernel and version directories', () => {
    dir = mkdtempSync(join(tmpdir(), 'brep-reports-'))
    cpSync(ALPHA, join(dir, SUITE, 'alpha'), { recursive: true })
    writeFileSync(join(dir, 'README.md'), 'not a suite')
    writeFileSync(join(dir, SUITE, 'notes.txt'), 'not a kernel')
    writeFileSync(join(dir, SUITE, 'alpha', 'notes.txt'), 'not a version')
    expect(readReports(dir).map(r => r.identity.kernel)).toEqual(['alpha'])
  })

  it('keeps one report per suite for a kernel release run on two suites', () => {
    dir = mkdtempSync(join(tmpdir(), 'brep-reports-'))
    const file = join(ALPHA, '1.0.0', 'report.json')
    const report = JSON.parse(readFileSync(file, 'utf8'))
    for (const suite of [SUITE, '1.1.0']) {
      mkdirSync(join(dir, suite, 'alpha', '1.0.0'), { recursive: true })
      const on = { ...report, suite: { ...report.suite, version: suite } }
      writeFileSync(join(dir, suite, 'alpha', '1.0.0', 'report.json'), JSON.stringify(on))
    }
    const reports = readReports(dir)
    expect(reports.map(r => [r.methodology.suite, r.identity.version])).toEqual([
      [SUITE, '1.0.0'],
      ['1.1.0', '1.0.0'],
    ])
  })

  // The alpha fixture's report, filed as springrep's with the adapter given.
  function asSpringrep(adapter: unknown): string {
    dir = mkdtempSync(join(tmpdir(), 'brep-reports-'))
    const report = JSON.parse(readFileSync(join(ALPHA, '1.0.0', 'report.json'), 'utf8'))
    const at = join(dir, SUITE, 'springrep', '1.0.0')
    mkdirSync(at, { recursive: true })
    const as = { ...report, kernel: { ...report.kernel, name: 'springrep' }, adapter }
    writeFileSync(join(at, 'report.json'), JSON.stringify(as))
    return dir
  }

  it('reads a kernel the registry lists, and refuses one it does not', () => {
    const repo = 'https://github.com/springcraft-ai/brep-conformance-springrep'
    const commit = '0123456789abcdef0123456789abcdef01234567'
    const [r] = readReports(asSpringrep({ repo, commit }), false)
    expect(r!.identity.adapter).toEqual({ repo, commit })
    rmSync(dir, { recursive: true, force: true })
    dir = mkdtempSync(join(tmpdir(), 'brep-reports-'))
    cpSync(ALPHA, join(dir, SUITE, 'alpha'), { recursive: true })
    expect(() => readReports(dir, false)).toThrow(/alpha is not in kernels\.json/)
  })

  it('refuses an official run of an adapter from another repository than the registry names', () => {
    const repo = 'https://example.com/elsewhere'
    const commit = '0123456789abcdef0123456789abcdef01234567'
    expect(() => readReports(asSpringrep({ repo, commit }), false)).toThrow(/not https:/)
  })

  it('refuses a report filed under another suite than the one it ran on', () => {
    dir = mkdtempSync(join(tmpdir(), 'brep-reports-'))
    cpSync(ALPHA, join(dir, '9.9.9', 'alpha'), { recursive: true })
    expect(() => readReports(dir)).toThrow(/on suite 1\.0\.0/)
  })
})
