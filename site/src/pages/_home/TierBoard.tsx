import { useEffect, useState } from 'react'
import { fmt, GradeMark, groupLabel, markGroups } from '@/components'
import { GRADES, type Grade } from '@/data/results'
import { url } from '@/site/href'
import { BarCard, tipAt } from './BarCard'
import { bandGrades, drawn, type BoardRow } from './model'
import { TierBar } from './TierBar'

const n = (x: number) => x.toLocaleString('en-US')

// A build with no release number reports its commit; it shows as a short revision.

// An SPDX expression in a few words: "/" between licences a user picks from, "+ exception" for a
// licence with an added exception. The whole expression is the title.
function License({ spdx }: { spdx: string }) {
  const short = spdx
    .replace(/-only\b/g, '')
    .replace(/ WITH [\w.-]+/g, ' + exception')
    .replace(/ OR /g, ' / ')
  const title = spdx.includes(' OR ')
    ? `${spdx}: either licence, at your choice`
    : spdx.includes(' WITH ')
      ? `${spdx}: the licence with an added exception`
      : spdx
  return <span title={title}>{short}</span>
}

// The share of a kernel's cases that pass at `tier` or tighter.
export const within = (r: BoardRow, tier: Grade) =>
  r.counts.slice(0, GRADES.indexOf(tier) + 1).reduce((a, b) => a + b, 0) / r.total

// Each column's one winner, the only figure in it marked in the accent: the highest share passing,
// a tie going to the faster kernel, and the lowest time. Still level, the first listed.
export function bests(board: BoardRow[]) {
  const time = (r: BoardRow) => r.time?.median ?? Number.POSITIVE_INFINITY
  const top = (score: (r: BoardRow) => number) =>
    board.reduce((w, r) =>
      score(r) > score(w) || (score(r) === score(w) && time(r) < time(w)) ? r : w,
    ).kernel
  return {
    rate: top(r => r.rate),
    loose: top(r => within(r, '1e-5')),
    time: board.reduce((w, r) => (time(r) < time(w) ? r : w)).kernel,
  }
}
const best = (on: boolean) => (on ? ' is-best' : '')

function Ruler() {
  return (
    <span className="hm-ruler" aria-hidden="true">
      {[0, 25, 50, 75, 100].map(p => (
        <span key={p}>{p}%</span>
      ))}
    </span>
  )
}

// Counts up from zero to `value` as the bars fill, over the same time, as springrep's board does;
// rendered at its value, so a page without script or with reduced motion shows it as it is.
const COUNT_MS = 1100
function Count({ value, digits, suffix = '' }: { value: number; digits: number; suffix?: string }) {
  const [shown, setShown] = useState(value)
  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const t0 = performance.now()
    let frame = 0
    const step = (t: number) => {
      const x = Math.min(1, Math.max(0, (t - t0) / COUNT_MS))
      setShown(value * (1 - Math.pow(1 - x, 3)))
      if (x < 1) frame = requestAnimationFrame(step)
    }
    setShown(0)
    frame = requestAnimationFrame(step)
    return () => cancelAnimationFrame(frame)
  }, [value])
  return (
    <>
      {shown.toFixed(digits)}
      {suffix}
    </>
  )
}

const decimals = (s: string) => s.split('.')[1]?.length ?? 0

// The median time of the kernel's passed cases, its unit set small; its hover card gives P25 and
// P75 and how many cases they are over.
function Speed({ r, isBest }: { r: BoardRow; isBest: boolean }) {
  const [tip, setTip] = useState<{ x: number; y: number } | null>(null)
  if (!r.time) return <span className="hm-fig hm-fig--none">–</span>
  const [value, unit] = fmt.ms(r.time.median).split(' ')
  const at = (el: HTMLElement) => {
    const b = el.getBoundingClientRect()
    setTip({ x: b.left, y: b.bottom - 10 })
  }
  return (
    <span
      className={`hm-fig hm-speed${best(isBest)}`}
      tabIndex={0}
      aria-label={`P50 ${fmt.ms(r.time.median)} per passed case; P25 ${fmt.ms(r.time.q1)}, P75 ${fmt.ms(r.time.q3)}, over ${n(r.time.cases)} passed cases`}
      onMouseMove={e => setTip({ x: e.clientX, y: e.clientY })}
      onFocus={e => at(e.currentTarget)}
      onMouseLeave={() => setTip(null)}
      onBlur={() => setTip(null)}
    >
      <Count value={Number(value)} digits={decimals(value!)} />
      <small>{unit}</small>
      {tip && (
        <span className="hm-tip" aria-hidden="true" style={tipAt(tip.x, tip.y)}>
          P50 {fmt.ms(r.time.median)} per passed case
          <small>
            P25 {fmt.ms(r.time.q1)} · P75 {fmt.ms(r.time.q3)}, over its {n(r.time.cases)} passed
            cases
          </small>
        </span>
      )}
    </span>
  )
}

// The bands the board's bars use, with their marks: only those some kernel has.
export function BoardKey({ board }: { board: BoardRow[] }) {
  const shown = markGroups().filter(group =>
    board.some(r => group.some(g => drawn(r.counts)[GRADES.indexOf(g)])),
  )
  return (
    <ul className="hm-key" aria-label="Key">
      {shown.map(group => (
        <li key={group[0]}>
          <GradeMark grade={group[0]!} title={groupLabel(group)} />
          {group.join(' / ')}
        </li>
      ))}
    </ul>
  )
}

// The leaderboard: each kernel's cases split by the tightest tolerance passed, its pass rate at
// 1e-7 and at 1e-5, and its speed, ranked by the 1e-7 rate. A bar's hover card lists its cases.
export function TierBoard({
  board,
  picked,
  onCompare,
}: {
  board: BoardRow[]
  // The kernels ticked for the grid below; null where every kernel is compared and no tick shows.
  picked: string[] | null
  onCompare: (kernel: string) => void
}) {
  const b = bests(board)
  return (
    <div className={`hm-board hm-board--tiers${picked ? ' hm-board--compare' : ''}`}>
      <table aria-label="Each kernel’s cases by the tightest tolerance passed">
        <thead>
          <tr>
            {picked && (
              <th className="hm-th--pick">
                <span>compare</span>
              </th>
            )}
            <th className="hm-th--left">kernel</th>
            <th className="hm-th--left">cases by tightest tolerance passed, mm</th>
            <th>
              pass at <span className="hm-num">1e-7</span> mm
            </th>
            <th>
              pass at <span className="hm-num">1e-5</span> mm
            </th>
            <th>
              speed
              <span className="hm-th__sub">lower is better</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {board.map(r => {
            const on = picked?.includes(r.kernel) ?? false
            const loose = within(r, '1e-5')
            return (
              <tr key={r.kernel}>
                {picked && (
                  <td className="hm-td--pick">
                    <button
                      type="button"
                      className="hm-pick sc-focus"
                      aria-pressed={on}
                      aria-label={`${on ? 'Remove' : 'Add'} ${r.kernel} ${on ? 'from' : 'to'} the comparison`}
                      onClick={() => onCompare(r.kernel)}
                    >
                      {on ? '■' : '□'}
                    </button>
                  </td>
                )}
                <td className="hm-td--kernel">
                  <a className="hm-kname" href={url.kernel(r.suite, r.segment)}>
                    {r.kernel}
                  </a>
                  <span className="hm-license">
                    {fmt.version(r.version)}
                    {r.license && (
                      <>
                        {' · '}
                        {r.source ? (
                          <a href={r.source}>
                            <License spdx={r.license} />
                          </a>
                        ) : (
                          <License spdx={r.license} />
                        )}
                      </>
                    )}
                  </span>
                </td>
                <td className="hm-td--bar">
                  <BarCard counts={r.counts}>
                    <TierBar
                      counts={r.counts}
                      label={`${r.kernel}: ${n(r.passing)} of ${n(r.total)} cases at 1e-7`}
                      top={r.kernel === b.rate}
                      href={g =>
                        `${url.kernel(r.suite, r.segment)}?result=${bandGrades(g).join(',')}#cases`
                      }
                    />
                  </BarCard>
                  <Ruler />
                </td>
                <td>
                  <span
                    className={`hm-fig hm-fig--lead${best(r.kernel === b.rate)}`}
                    title={`${n(r.passing)} of ${n(r.total)} cases hold at 1e-7 mm`}
                  >
                    <Count value={r.rate * 100} digits={1} suffix="%" />
                  </span>
                </td>
                <td>
                  <span
                    className={`hm-fig${best(r.kernel === b.loose)}`}
                    title={`${fmt.pct(loose)} of cases hold at 1e-5 mm or tighter`}
                  >
                    <Count value={loose * 100} digits={1} suffix="%" />
                  </span>
                </td>
                <td>
                  <Speed r={r} isBest={!!r.time && r.kernel === b.time} />
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
