import { TIERS, type Grade, type Tier } from '@/data/results'
import { fileName } from './files'

// A measured value as the suite wrote it: counts whole, tiny and huge values in exponent form, the
// rest to six significant figures with no trailing zeros.
export function value(x: number): string {
  if (!Number.isFinite(x)) return String(x)
  if (Number.isInteger(x) && Math.abs(x) < 1e9) return String(x)
  const a = Math.abs(x)
  if (a !== 0 && (a < 1e-3 || a >= 1e7)) return x.toExponential(2)
  return String(Number(x.toPrecision(6)))
}

export function point(p: readonly number[]): string {
  return p.map(value).join(', ')
}

// The test's path below its top-level directory, as its page names it: in words, not hyphens.
export function testName(test: string): string {
  return (test.split('/').slice(1).join(' / ') || test).replaceAll('-', ' ')
}

const SHORT_HASH = 6

// Each case id as a list shows it: the id's hash cut short, after its family's name only where the
// ids are of more than one family. The whole id stays the case's name everywhere else.
export function shortCaseIds(ids: readonly string[]): string[] {
  const parts = ids.map(id => {
    const cut = id.lastIndexOf('-')
    return cut < 0
      ? { family: '', hash: id }
      : { family: id.slice(0, cut), hash: id.slice(cut + 1) }
  })
  const mixed = new Set(parts.map(p => p.family)).size > 1
  return parts.map(p => {
    const hash = p.hash.slice(0, SHORT_HASH)
    return mixed && p.family ? `${p.family} ${hash}` : hash
  })
}

// A grade's place among the tiers, best first; anything but a pass after every tier.
const tierOf = (g: Grade | null) => {
  const t = TIERS.indexOf(g as Tier)
  return t < 0 ? TIERS.length : t
}

// The kernels, as indices, best on this test first: by how many of its cases each passed at
// each tier in turn, then by fewer failures, then by the leaderboard's `ranks`.
export function kernelOrder(
  cases: readonly (readonly (Grade | null)[])[],
  ranks: number[],
): number[] {
  const score = ranks.map((_, k) => {
    const s = new Array<number>(TIERS.length + 1).fill(0)
    for (const g of cases) {
      const t = tierOf(g[k] ?? null)
      if (t < TIERS.length) s[t]! += 1
      else if (g[k] !== null && g[k] !== undefined) s[TIERS.length]! -= 1
    }
    return s
  })
  const better = (a: number, b: number) => {
    for (let t = 0; t <= TIERS.length; t++)
      if (score[a]![t] !== score[b]![t]) return score[b]![t]! - score[a]![t]!
    return ranks[a]! - ranks[b]!
  }
  return ranks.map((_, k) => k).sort(better)
}

// Whose drawing of a case to show, as an index into the kernels, among those that drew it: the
// reader's own kernel where it passed, else the best tier's, a tie going by `order`.
export function drawnBy(
  files: readonly ({ glb: string | null } | null)[],
  grades: readonly (Grade | null)[],
  order: readonly number[],
  chosen: number,
): number | null {
  const drew = files.flatMap((f, i) => (f?.glb ? [i] : []))
  if (!drew.length) return null
  if (drew.includes(chosen) && tierOf(grades[chosen] ?? null) < TIERS.length) return chosen
  const best = Math.min(...drew.map(i => tierOf(grades[i] ?? null)))
  return order.find(i => drew.includes(i) && tierOf(grades[i] ?? null) === best) ?? drew[0]!
}

// What a STEP file's link says: a case of one solid has one file, <id>.<hash>.step, and a case of
// two has one each, <id>.<hash>.<role>.step, told apart by the role.
export function stepLabel(url: string): string {
  const parts = fileName(url).split('.')
  return parts.length > 3 ? `step · ${parts.at(-2)}` : 'step'
}
