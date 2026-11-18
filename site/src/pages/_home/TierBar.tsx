import type { CSSProperties } from 'react'
import { GRADES, type Grade } from '@/data/results'
import { drawn, type Counts } from './model'
import './bar.css'

// A share of cases per grade, best first, as one bar of blocks. Blocks are proportional; the label
// carries the counts for anyone not reading colour. Each block's fill starts after the share of
// FILL_MS the blocks before it take up, so a bar that animates fills left to right.
const FILL_MS = 1100

export function TierBar({
  counts,
  label,
  thin,
  top,
  href,
}: {
  counts: Counts
  label: string
  thin?: boolean
  // The bar of the kernel with the best pass rate, its 1e-7 block in the accent.
  top?: boolean
  // Makes each segment a link, to that grade's cases; its own card then names it, not a title.
  href?: (grade: Grade) => string
}) {
  counts = drawn(counts)
  const total = counts.reduce((a, b) => a + b, 0)
  const before = counts.map((_, i) => counts.slice(0, i).reduce((a, b) => a + b, 0))
  const fill = (i: number) =>
    ({
      flexGrow: counts[i],
      flexBasis: 0,
      '--fill-at': `${Math.round((before[i]! / total) * FILL_MS)}ms`,
    }) as CSSProperties
  return (
    // With links it is a group of them, since an image may not hold controls.
    <span
      className={`hm-bar${thin ? ' hm-bar--thin' : ''}${top ? ' hm-bar--top' : ''}`}
      role={href ? 'group' : 'img'}
      aria-label={label}
    >
      {GRADES.map((g, i) =>
        !counts[i] ? null : href ? (
          <a
            key={g}
            className="hm-bar__seg"
            data-grade={g}
            href={href(g)}
            aria-label={`${g}: ${counts[i]!.toLocaleString('en-US')} of ${total.toLocaleString('en-US')} cases, list them`}
            style={fill(i)}
          />
        ) : (
          <span
            key={g}
            className="hm-bar__seg"
            data-grade={g}
            style={fill(i)}
            title={total ? `${g}: ${counts[i]} of ${total}` : undefined}
          />
        ),
      )}
    </span>
  )
}
