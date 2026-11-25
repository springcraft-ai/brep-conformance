import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { kernelInfo } from './kernels'
import { ASSET_BASE, buildSite, LOCAL_ASSETS, parseReport, type Report, type Site } from './results'
import { boardRuns, currentSuite, emptyIndex, fileHash, parseIndex, type RunsIndex } from './runs'

// Build-time only: a page's *.data.ts may import this, a *.page.tsx never does.

// A folder holding reports.json and, below it, <run>/<file> for each report it lists, as
// `node scripts/reports.ts fetch` leaves them: BREP_REPORTS_DIR, or else .reports where
// `npm run reports` fetched it. With neither the site has no results.
const DEFAULT_REPORTS_DIR = resolve('.reports')
export const REPORTS_DIR = process.env.BREP_REPORTS_DIR
  ? resolve(process.env.BREP_REPORTS_DIR)
  : existsSync(join(DEFAULT_REPORTS_DIR, 'reports.json'))
    ? DEFAULT_REPORTS_DIR
    : null

const ASSETS_DIR = process.env.BREP_ASSETS_DIR ? resolve(process.env.BREP_ASSETS_DIR) : null

// The folder a report's files load from: its copy in BREP_ASSETS_DIR, which the dev and preview
// servers serve, where there is one, and its folder on the CDN where there is not.
function assetFolder(suite: string, kernel: string, version: string): string {
  if (ASSETS_DIR && existsSync(join(ASSETS_DIR, suite, kernel, version))) {
    const base = (process.env.SITE_BASE ?? '/').replace(/\/*$/, '/')
    return `${base}${LOCAL_ASSETS}/${suite}/${kernel}/${version}`
  }
  return `${ASSET_BASE}/${suite}/${kernel}/${version}`
}

// The test fixtures' kernels are made up, so the tests read them without the registry.
const ANY_KERNEL = process.env.BREP_ANY_KERNEL === '1'

// A report of a kernel kernels.json no longer lists is kept and marked, for its pages; a
// registered kernel's adapter must come from the repository the registry names, at any commit,
// for an earlier report.
function checkRegistered(file: string, report: Report): void {
  const id = report.identity
  const info = kernelInfo(id.kernel)
  if (!info) {
    report.registered = false
    return
  }
  if (id.adapter && id.adapter.repo !== info.adapter.repo)
    throw new Error(`${file}: ran the adapter in ${id.adapter.repo}, not ${info.adapter.repo}`)
}

export interface Published {
  index: RunsIndex
  reports: Report[]
}

// Every report reports.json lists. A file must hash to the name it is listed under, and say the
// run, suite and kernel its entry does.
export function readPublished(dir = REPORTS_DIR, anyKernel = ANY_KERNEL): Published {
  if (!dir) return { index: emptyIndex(), reports: [] }
  const indexFile = join(dir, 'reports.json')
  const index = parseIndex(JSON.parse(readFileSync(indexFile, 'utf8')), indexFile)
  const reports: Report[] = []
  for (const entry of index.runs) {
    for (const [kernel, listed] of Object.entries(entry.reports)) {
      const file = join(dir, entry.run, listed.file)
      if (!existsSync(file)) throw new Error(`${file}: not fetched; run scripts/reports.ts fetch`)
      const bytes = readFileSync(file)
      const hash = createHash('sha256').update(bytes).digest('hex').slice(0, 16)
      if (hash !== fileHash(kernel, listed.file))
        throw new Error(`${file}: its contents hash to ${hash}, not the name reports.json lists`)
      const raw = JSON.parse(bytes.toString('utf8'))
      const folder = assetFolder(
        String(raw?.suite?.version),
        String(raw?.kernel?.name),
        String(raw?.kernel?.version),
      )
      const report = parseReport(raw, file, folder)
      const id = report.identity
      if (report.run.id !== entry.run) throw new Error(`${file}: says run ${report.run.id}`)
      if (id.kernel !== kernel) throw new Error(`${file}: is ${id.kernel}'s, not ${kernel}'s`)
      if (report.methodology.suite !== entry.suite)
        throw new Error(`${file}: ran suite ${report.methodology.suite}, not ${entry.suite}`)
      if (report.run.complete !== listed.complete)
        throw new Error(`${file}: complete is ${report.run.complete}, not ${listed.complete}`)
      if (!anyKernel) checkRegistered(file, report)
      reports.push(report)
    }
  }
  return { index, reports }
}

// Each suite's leaderboard: its run and each kernel's. A kernel that left kernels.json leaves the
// current leaderboard; an older suite's leaderboard is the record of that version, and keeps it.
export function boardsOf(
  index: RunsIndex,
  reports: Report[],
): Map<string, { run: string; kernels: Map<string, string> }> {
  const current = currentSuite(index)
  const gone = new Set(reports.filter(r => !r.registered).map(r => r.identity.kernel))
  return new Map(
    Object.entries(index.leaderboard).map(([suite, b]) => {
      const kernels = boardRuns(index, suite)
      if (suite === current) for (const k of gone) kernels.delete(k)
      return [suite, { run: b.run, kernels }]
    }),
  )
}

let cached: Site | undefined

export function site(): Site {
  if (!cached) {
    const { index, reports } = readPublished()
    cached = buildSite(reports, boardsOf(index, reports), currentSuite(index))
  }
  return cached
}
