// The published runs' reports, for the Pages build and the Benchmark workflow (doc/formats.md):
//
//   node scripts/reports.ts fetch <reports.json> <dir>
//       Copies reports.json into <dir> and fetches each report it lists from the CDN into
//       <dir>/<run>/<file>, as the site's BREP_REPORTS_DIR reads them. A file already there with
//       the hash its name pins is kept.
//   node scripts/reports.ts add <reports.json> <run dir> <body.md>
//       Adds the run in <run dir> (its entry.json and report files, as bench/publish-reports.sh
//       leaves them) to reports.json, moves the leaderboard, and writes the pull request's body.
//       Exits 3, changing nothing, when the run moves nothing.
//
// Node runs this file as it is, so it imports only modules without imports of their own.

import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { ASSET_BASE, caseTime, parseReport, summarize, type Report } from '../src/data/results.ts'
import {
  addRun,
  boardRuns,
  fileHash,
  formatIndex,
  parseIndex,
  type Change,
  type RunEntry,
  type RunsIndex,
} from '../src/data/runs.ts'

const hash16 = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex').slice(0, 16)

const reportUrl = (run: string, file: string) => `${ASSET_BASE}/reports/${run}/${file}`

async function download(run: string, kernel: string, file: string): Promise<Uint8Array> {
  const url = reportUrl(run, file)
  const res = await fetch(url)
  // The CDN answers 403, not 404, for a key that is not there.
  if (!res.ok) throw new Error(`run ${run}: ${kernel}'s report ${url} answers ${res.status}`)
  const bytes = new Uint8Array(await res.arrayBuffer())
  const got = hash16(bytes)
  if (got !== fileHash(kernel, file))
    throw new Error(`run ${run}: ${url} hashes to ${got}, not the name it is listed under`)
  return bytes
}

async function fetchAll(indexFile: string, dir: string): Promise<void> {
  const text = readFileSync(indexFile, 'utf8')
  const index = parseIndex(JSON.parse(text), indexFile)
  mkdirSync(dir, { recursive: true })
  writeFileSync(join(dir, 'reports.json'), text)
  let fetched = 0
  for (const entry of index.runs) {
    mkdirSync(join(dir, entry.run), { recursive: true })
    for (const [kernel, r] of Object.entries(entry.reports)) {
      const path = join(dir, entry.run, r.file)
      if (existsSync(path) && hash16(readFileSync(path)) === fileHash(kernel, r.file)) continue
      writeFileSync(path, await download(entry.run, kernel, r.file))
      fetched++
    }
  }
  const files = index.runs.reduce((n, r) => n + Object.keys(r.reports).length, 0)
  console.log(`${files} reports of ${index.runs.length} runs in ${dir}, ${fetched} fetched`)
}

const pct = (x: number) => `${(x * 100).toFixed(1)}%`
const ms = (r: Report) => {
  const t = caseTime(r)
  return t ? t.median.toFixed(1) : '–'
}

function row(kernel: string, now: Report, before: Report | null, entry: RunEntry): string {
  const s = summarize(now.cases)
  const b = before ? summarize(before.cases) : null
  const delta = b ? `${((s.passRate - b.passRate) * 100).toFixed(1)} pt` : 'new'
  const was = before ? `${before.identity.version}, ${pct(b!.passRate)}, ${ms(before)} ms` : '–'
  const note = entry.reports[kernel]!.complete ? '' : ' not every test'
  return (
    `| ${kernel} | ${now.identity.version} | ${pct(s.passRate)} | ${ms(now)} | ${was} | ` +
    `${delta} |${note}`
  )
}

function changeText(change: Change, entry: RunEntry): string {
  switch (change.kind) {
    case 'default':
      return change.replaced
        ? `Makes run ${entry.run} suite ${change.suite}'s leaderboard, in place of run ` +
            `${change.replaced}, and clears its other runs' rows.`
        : `Makes run ${entry.run} the leaderboard of suite ${change.suite}, its first.`
    case 'overrides':
      return (
        `Takes ${change.kernels.join(', ')} on suite ${change.suite}'s leaderboard from run ` +
        `${entry.run}; the other rows stay as they are.`
      )
    case 'listed':
      return (
        `Lists run ${entry.run} with its pages. Suite ${change.suite} has no leaderboard ` +
        'yet, and only a run of every kernel starts one.'
      )
    case 'none':
      return change.why
  }
}

async function add(indexFile: string, runDir: string, bodyFile: string): Promise<number> {
  const index: RunsIndex = parseIndex(JSON.parse(readFileSync(indexFile, 'utf8')), indexFile)
  const entry = parseIndex({
    runs: [JSON.parse(readFileSync(join(runDir, 'entry.json'), 'utf8'))],
  }).runs[0]!
  const { index: next, change } = addRun(index, entry)
  console.log(changeText(change, entry))
  if (change.kind === 'none') return 3
  parseIndex(next)

  const before = boardRuns(index, entry.suite)
  const listed = new Map(index.runs.map(r => [r.run, r]))
  const rows: string[] = []
  for (const [kernel, r] of Object.entries(entry.reports).sort()) {
    const now = parseReport(JSON.parse(readFileSync(join(runDir, r.file), 'utf8')), r.file)
    const from = before.get(kernel)
    const old = from ? listed.get(from)!.reports[kernel]! : null
    const was = old
      ? parseReport(JSON.parse(new TextDecoder().decode(await download(from!, kernel, old.file))))
      : null
    rows.push(row(kernel, now, was, entry))
  }
  writeFileSync(indexFile, formatIndex(next))
  const link = `https://github.com/${process.env.GITHUB_REPOSITORY ?? ''}/actions/runs/`
  const runId = entry.run.split('-')[0]
  writeFileSync(
    bodyFile,
    [
      '## Summary',
      '',
      `- Publishes [Benchmark run ${entry.run}](${link}${runId}), a run of ` +
        (entry.scope === 'all' ? 'every kernel' : 'named kernels') +
        `: ${Object.keys(entry.reports).sort().join(', ')} on suite ${entry.suite}.`,
      `- ${changeText(change, entry)}`,
      '- Each report is on the CDN under `reports/' + entry.run + '/`, named by its hash.',
      '',
      '| Kernel | Version | At 1e-7 | P50 ms | On the leaderboard now | Change |',
      '| --- | --- | --- | --- | --- | --- |',
      ...rows,
      '',
      '## Test plan',
      '',
      '- [x] The publish job uploaded every report this run lists, and `reports.json` checks.',
      '- [ ] The Pages check on this pull request fetches every listed report and builds the site.',
      '',
    ].join('\n'),
  )
  return 0
}

const [what, ...args] = process.argv.slice(2)
try {
  if (what === 'fetch' && args.length === 2) await fetchAll(args[0]!, args[1]!)
  else if (what === 'add' && args.length === 3)
    process.exitCode = await add(args[0]!, args[1]!, args[2]!)
  else {
    console.error('usage: reports.ts fetch <reports.json> <dir>')
    console.error('       reports.ts add <reports.json> <run dir> <body.md>')
    process.exitCode = 2
  }
} catch (e) {
  console.error((e as Error).message)
  process.exitCode = 1
}
