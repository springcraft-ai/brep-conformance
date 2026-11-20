import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { kernelInfo, KERNELS } from './kernels'
import { ASSET_BASE, buildSite, LOCAL_ASSETS, parseReport, type Report, type Site } from './results'

// Build-time only: a page's *.data.ts may import this, a *.page.tsx never does.

// With no BREP_REPORTS_DIR the site is built with no results.
export const REPORTS_DIR = process.env.BREP_REPORTS_DIR
  ? resolve(process.env.BREP_REPORTS_DIR)
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

function subdirs(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true })
    .filter(e => e.isDirectory() && !e.name.startsWith('.'))
    .map(e => e.name)
    .sort()
}

// The test fixtures' kernels are made up, so the tests read them without the registry.
const ANY_KERNEL = process.env.BREP_ANY_KERNEL === '1'

// Only a kernel in the registry is shown, and an official run's adapter must come from the
// repository the registry names; its commit may be an earlier one, for an earlier report.
function checkRegistered(file: string, report: Report): void {
  const id = report.identity
  const info = kernelInfo(id.kernel)
  if (!info)
    throw new Error(
      `${file}: ${id.kernel} is not in kernels.json, which lists ` +
        KERNELS.map(k => k.kernel).join(', '),
    )
  if (id.adapter && id.adapter.repo !== info.adapter.repo)
    throw new Error(`${file}: ran the adapter in ${id.adapter.repo}, not ${info.adapter.repo}`)
}

// <suite>/<kernel>/<version>/report.json: one kernel release keeps a report per suite version, so a
// rerun on a new suite never replaces the result an older link names.
export function readReports(dir = REPORTS_DIR, anyKernel = ANY_KERNEL): Report[] {
  if (!dir) return []
  const out: Report[] = []
  for (const suite of subdirs(dir)) {
    for (const kernel of subdirs(join(dir, suite))) {
      for (const version of subdirs(join(dir, suite, kernel))) {
        const file = join(dir, suite, kernel, version, 'report.json')
        if (!existsSync(file)) continue
        const raw = JSON.parse(readFileSync(file, 'utf8'))
        const report = parseReport(raw, file, assetFolder(suite, kernel, version))
        const id = report.identity
        const on = report.methodology.suite
        if (on !== suite || id.kernel !== kernel || id.version !== version)
          throw new Error(`${file}: says ${id.kernel} ${id.version} on suite ${on}`)
        if (!anyKernel) checkRegistered(file, report)
        out.push(report)
      }
    }
  }
  return out
}

let cached: Site | undefined

export function site(): Site {
  cached ??= buildSite(readReports())
  return cached
}
