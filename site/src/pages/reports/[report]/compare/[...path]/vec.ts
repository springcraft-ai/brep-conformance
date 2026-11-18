export type Vec3 = [number, number, number]

export function bounds(xs: ArrayLike<number>): { lo: Vec3; hi: Vec3 } | null {
  if (xs.length < 3) return null
  const lo: Vec3 = [Infinity, Infinity, Infinity]
  const hi: Vec3 = [-Infinity, -Infinity, -Infinity]
  for (let i = 0; i + 2 < xs.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      lo[k] = Math.min(lo[k]!, xs[i + k]!)
      hi[k] = Math.max(hi[k]!, xs[i + k]!)
    }
  }
  return { lo, hi }
}
