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

// Whose drawing of a case to show, as an index into the kernels: the preferred kernel's where it
// drew the case, else the first kernel's that did.
export function drawnBy(
  files: readonly ({ glb: string | null } | null)[],
  preferred: number,
): number | null {
  if (files[preferred]?.glb) return preferred
  const first = files.findIndex(f => f?.glb)
  return first < 0 ? null : first
}

// What a STEP file's link says: a case of one solid has one file, <id>.<hash>.step, and a case of
// two has one each, <id>.<hash>.<role>.step, told apart by the role.
export function stepLabel(url: string): string {
  const parts = url.slice(url.lastIndexOf('/') + 1).split('.')
  return parts.length > 3 ? `step · ${parts.at(-2)}` : 'step'
}
