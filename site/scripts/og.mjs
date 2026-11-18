// Photographs scripts/og.html at 1200 × 630 into og.png, the image a link to the site previews
// with, for uploading to the CDN. Needs headless Chrome: set CHROME where it is not in the usual place.
import { execFileSync } from 'node:child_process'
import { existsSync, mkdtempSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const page = resolve(here, 'og.html')
const out = resolve(here, '../og.png')

const chrome = [
  process.env.CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
].find(c => c && existsSync(c))
if (!chrome) throw new Error('og.mjs: no Chrome found; set CHROME to one')

const profile = mkdtempSync(join(tmpdir(), 'og-'))
try {
  execFileSync(
    chrome,
    [
      '--headless=new',
      '--hide-scrollbars',
      '--force-device-scale-factor=1',
      '--window-size=1200,630',
      '--virtual-time-budget=5000',
      `--user-data-dir=${profile}`,
      `--screenshot=${out}`,
      pathToFileURL(page).href,
    ],
    { stdio: 'ignore' },
  )
} finally {
  rmSync(profile, { recursive: true, force: true })
}
console.log(`${out} (${statSync(out).size} bytes)`)
