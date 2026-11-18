// Reads a test's definition and its clauses' promises out of the suite's own specs, so the page
// quotes doc/api.md and doc/contract.md rather than restating them. Server only.
import type { Kind } from '@/data/results'

// Text with `code` spans kept apart, so the page sets them in the machine face.
export type Run = string | { code: string }

export interface Definition {
  // The section of api.md it was read from, as its heading names it.
  heading: string
  signatures: string[]
  paragraphs: Run[][]
}

// The api.md section each test area (a test path's first part, the report's `operation`) calls.
// The report does not name a test's functions, so a test is defined by its area's section. Parts
// combine many functions, so they have none.
export const AREA_SECTION: Record<string, string> = {
  booleans: 'Booleans',
  chamfers: 'Blends',
  constructions: 'Constructors',
  fillets: 'Blends',
  modify: 'Changing a solid',
  queries: 'Queries',
  step: 'Exchange',
}

function runs(text: string): Run[] {
  const plain = text.replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').replace(/\s+/g, ' ')
  return plain
    .split(/(`[^`]+`)/)
    .filter(Boolean)
    .map(s => (s.startsWith('`') ? { code: s.slice(1, -1) } : s))
}

const isSignature = (p: string) => /^`[^`]+`(, `[^`]+`)*$/.test(p)

// An area's section: every signature its functions state, and the section's opening paragraphs.
export function definitionOf(area: string, apiMd: string): Definition | null {
  const heading = AREA_SECTION[area]
  if (!heading) return null
  const lines = apiMd.split('\n')
  const start = lines.findIndex(l => l.trim() === `## ${heading}`)
  if (start < 0) return null
  const intro: string[] = []
  const body: string[] = []
  for (const line of lines.slice(start + 1)) {
    if (/^## /.test(line)) break
    if (/^### /.test(line) || body.length) body.push(line)
    else intro.push(line)
  }
  const blocks = (text: string[]) =>
    text
      .join('\n')
      .split(/\n\s*\n/)
      .map(b => b.trim())
      .filter(Boolean)
  const signatures: string[] = []
  for (const b of blocks(body))
    if (isSignature(b.replace(/\s*\n\s*/g, ' ')))
      for (const m of b.matchAll(/`([^`]+)`/g)) signatures.push(m[1]!)
  const paragraphs = blocks(intro)
    .filter(b => !/^(\||```|[-*] |\d+\. )/.test(b))
    .map(runs)
  return { heading, signatures, paragraphs }
}

// Every clause id in contract.md's tables, with its promise.
export function clausesOf(contractMd: string): Record<string, string> {
  const out: Record<string, string> = {}
  for (const m of contractMd.matchAll(/^\|\s*([BQ]\d+)\s*\|\s*(.+?)\s*\|\s*$/gm)) out[m[1]!] = m[2]!
  return out
}

// What each kind of test asks, in the terms of doc/process.md.
export const KIND_TEXT: Record<Kind, string> = {
  exact:
    'The definition fixes the result as a set of points. The test checks the answer against it: a ' +
    'closed form, or the suite’s own sample.',
  sound:
    'The definition leaves the result open within limits. The test checks that the result is ' +
    'valid and keeps what every acceptable answer has, not one answer. Scored apart.',
  rejection:
    'The input is outside what the function allows. The kernel must refuse it; building a solid ' +
    'from it fails, however plausible the solid.',
}
