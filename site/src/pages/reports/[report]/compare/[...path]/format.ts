import type { Crumb } from '@/components'
import { TIERS, type Grade, type Tier } from '@/data/results'
import { url } from '@/site/href'
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

// The last level of the test's path, as its page names it: in words, not hyphens.
export function testName(test: string): string {
  return test.split('/').at(-1)!.replaceAll('-', ' ')
}

// Articles, conjunctions and short prepositions stay lower case inside a title.
const MINOR = new Set(
  'a an and as at but by for from in into nor of on onto or per the to via with'.split(' '),
)

// A test's or a level's name in title case, for the browser's title of its page.
export function titleOf(path: string): string {
  const words = testName(path).split(' ')
  return words
    .map((w, i) =>
      i > 0 && i < words.length - 1 && MINOR.has(w) ? w : w.charAt(0).toUpperCase() + w.slice(1),
    )
    .join(' ')
}

// The pages a test or a level sits under: its report, then each level of its path above it.
export function trailOf(suite: string, path: string): Crumb[] {
  const parts = path.split('/')
  return [
    { label: `Report ${suite}`, href: url.report(suite) },
    ...parts.slice(0, -1).map((label, i) => ({
      label,
      href: url.compare(suite, parts.slice(0, i + 1).join('/')),
    })),
  ]
}

// Each case id as a list shows it beside its number: its family's name where the ids are of more
// than one family, and nothing where they share one. The whole id stays the case's name elsewhere.
export function caseFamilies(ids: readonly string[]): string[] {
  const families = ids.map(id => (id.includes('-') ? id.slice(0, id.lastIndexOf('-')) : ''))
  return new Set(families).size > 1 ? families : families.map(() => '')
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

// The grades under which a kernel made no result: a wrong result ('fail') is still a solid to see.
const NO_RESULT = new Set<Grade>(['refused', 'crash', 'timeout', 'unsupported'])

// Whether a grade is one of a kernel that made no result. Such a kernel's drawing holds only what
// it made before it stopped, as an operand, or nothing.
export function madeNoResult(grade: Grade | null): boolean {
  return !!grade && NO_RESULT.has(grade)
}

// Whose drawing of a case to show, as an index into the kernels, among those that wrote one: the
// kernel `asked` for wherever it wrote one, partial or not; else the best tier's, a kernel with a
// result before one with none, a tie going by `order`.
export function drawnBy(
  files: readonly ({ glb: string | null } | null)[],
  grades: readonly (Grade | null)[],
  order: readonly number[],
  asked = -1,
): number | null {
  const drew = files.flatMap((f, i) => (f?.glb ? [i] : []))
  if (!drew.length) return null
  if (drew.includes(asked)) return asked
  const whole = drew.filter(i => !madeNoResult(grades[i] ?? null))
  const pool = whole.length ? whole : drew
  const best = Math.min(...pool.map(i => tierOf(grades[i] ?? null)))
  return order.find(i => pool.includes(i) && tierOf(grades[i] ?? null) === best) ?? pool[0]!
}

// What a STEP file's link says: a case of one solid has one file, <id>.<hash>.step, and a case of
// two has one each, <id>.<hash>.<role>.step, told apart by the role.
export function stepLabel(url: string): string {
  const parts = fileName(url).split('.')
  return parts.length > 3 ? `step · ${parts.at(-2)}` : 'step'
}

// The zoom key as the reader's keyboard names it. Some browsers report the newer platform as an
// empty string, so an empty one falls back to the older.
export function zoomKeyOf(uaPlatform: string | undefined, platform: string): string {
  return /mac|iphone|ipad/i.test(uaPlatform || platform) ? '⌘' : 'ctrl'
}
