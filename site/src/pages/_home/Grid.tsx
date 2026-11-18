import { useEffect, useRef, useState } from 'react'
import { Segmented } from '@/ds'
import { fmt } from '@/components'
import { url } from '@/site/href'
import {
  caseTitle,
  distinctFilters,
  FILTER_LABEL,
  FILTERS,
  keep,
  matchesQuery,
  passing,
  relativeTimes,
  shownShare,
  total,
  type Cell,
  type Filter,
  type GridGroup,
} from './model'
import { TierBar } from './TierBar'

const ANY_KERNEL = 'any'

// A row's bars fill and its shares count up the first time it comes into view, as the leaderboard's
// do, and never again; a test row first comes into view when its operation is first opened. With
// reduced motion everything is drawn as it is.
const COUNT_MS = 1100
function countUp(row: Element) {
  for (const el of row.querySelectorAll<HTMLElement>('[data-to]')) {
    const text = el.firstChild
    if (!text) continue
    const to = Number(el.dataset.to)
    const t0 = performance.now()
    const step = (t: number) => {
      const x = Math.min(1, (t - t0) / COUNT_MS)
      text.nodeValue = (to * (1 - Math.pow(1 - x, 3))).toFixed(1) + '%'
      if (x < 1) requestAnimationFrame(step)
    }
    requestAnimationFrame(step)
  }
}
function useRowsPlay(root: React.RefObject<HTMLDivElement | null>) {
  const io = useRef<IntersectionObserver | null>(null)
  const seen = useRef(new WeakSet<Element>())
  // The rows that have played, by name: a row the filter or the search draws again is shown as it
  // is, not played twice.
  const played = useRef(new Set<string>())
  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    root.current?.classList.add('hm-grid--play')
    io.current = new IntersectionObserver(entries => {
      for (const e of entries) {
        if (!e.isIntersecting) continue
        e.target.classList.add('is-lit')
        countUp(e.target)
        played.current.add((e.target as HTMLElement).dataset.row ?? '')
        io.current?.unobserve(e.target)
      }
    })
    return () => io.current?.disconnect()
  }, [root])
  // Rows come and go with the filter and the search: each new one is watched once.
  useEffect(() => {
    if (!io.current || !root.current) return
    for (const row of root.current.querySelectorAll<HTMLElement>('.hm-row[data-row]')) {
      if (seen.current.has(row)) continue
      seen.current.add(row)
      if (played.current.has(row.dataset.row!)) row.classList.add('is-done')
      else io.current.observe(row)
    }
  })
}

// A row's one winner, the only cell in it marked in the accent: the highest share passing at 1e-7,
// a tie going to the faster cell, or on an operation's row, which has no time of its own, to the
// kernel that wins more of its tests. Still level, the first listed. -1 where nothing passed.
const shareOf = (c: Cell | null) => (c && total(c) ? passing(c) / total(c) : -1)
function winner(cells: (Cell | null)[], tiebreak: (i: number) => number): number {
  let w = -1
  cells.forEach((c, i) => {
    const s = shareOf(c)
    if (s <= 0) return
    const ws = w < 0 ? -1 : shareOf(cells[w]!)
    if (s > ws || (s === ws && tiebreak(i) < tiebreak(w))) w = i
  })
  return w
}
const testWinner = (cells: (Cell | null)[]) =>
  winner(cells, i => cells[i]?.medianMs ?? Number.POSITIVE_INFINITY)
function opWinner(g: GridGroup): number {
  const wins = g.cells.map(() => 0)
  for (const t of g.tests) {
    const w = testWinner(t.cells)
    if (w >= 0) wins[w]!++
  }
  return winner(g.cells, i => -wins[i]!)
}

// Every test against the kernels compared, one <details> per operation so it reads without JS.
// Hydrated, it adds a filter, the search, and open/close all. A cell opens that kernel's cases on
// the test.
export function Grid({
  groups,
  kernels,
  shown,
  suite,
  versions,
  q,
}: {
  groups: GridGroup[]
  kernels: string[]
  shown: string[]
  // Every kernel in the grid is on this suite version.
  suite: string
  versions: Record<string, string>
  q: string
}) {
  const root = useRef<HTMLDivElement>(null)
  useRowsPlay(root)
  const [filter, setFilter] = useState<Filter>('all')
  const [focus, setFocus] = useState(ANY_KERNEL)
  const [open, setOpen] = useState<Record<string, boolean>>({})
  const [jump, setJump] = useState<string | null>(null)

  // A link to an operation (#booleans) opens its tests and brings them to the top: closed, the
  // group is one row near the page's foot, which the browser cannot scroll up to.
  useEffect(() => {
    const show = () => {
      let op: string
      try {
        op = decodeURIComponent(location.hash.slice(1))
      } catch {
        return
      }
      if (!groups.some(g => g.operation === op)) return
      setOpen(o => ({ ...o, [op]: true }))
      setJump(op)
    }
    show()
    window.addEventListener('hashchange', show)
    return () => window.removeEventListener('hashchange', show)
  }, [groups])
  // Scrolls once the group has been drawn open, when the page is tall enough to scroll to it.
  useEffect(() => {
    if (!jump) return
    document.getElementById(jump)?.scrollIntoView({ block: 'start' })
    setJump(null)
  }, [jump])
  const cols = shown.map(k => kernels.indexOf(k)).filter(i => i >= 0)
  const pick = <T,>(xs: T[]) => cols.map(i => xs[i]!)
  const scope = shown.includes(focus) ? shown.indexOf(focus) : null
  const filters = distinctFilters(
    groups.flatMap(g => g.tests.map(t => ({ ...t, cells: pick(t.cells) }))),
    scope,
  )
  const shownFilter = filters.includes(filter) ? filter : 'all'
  const searching = q.trim() !== ''
  const visible = groups
    .map(g => ({
      ...g,
      cells: pick(g.cells),
      tests: g.tests
        .filter(t => matchesQuery(t, g.operation, q))
        .map(t => ({ ...t, cells: pick(t.cells) }))
        .filter(t => keep(t, shownFilter, scope)),
    }))
    .filter(g => g.tests.length)
  const tests = visible.flatMap(g => g.tests)
  const allTests = groups.reduce((n, g) => n + g.tests.length, 0)
  const isOpen = (op: string) => open[op] ?? searching
  const names = cols.map(i => kernels[i]!)
  const caseLink = (kernel: string, test: string) =>
    `${url.report(suite, kernel, versions[kernel]!)}?test=${encodeURIComponent(test)}#cases`

  return (
    <div ref={root} className="hm-grid" style={{ ['--hm-k' as string]: names.length }}>
      <div className="hm-grid__tools">
        <span className="hm-grid__filters">
          <Segmented
            label="show"
            options={FILTERS.map(f => ({
              id: f,
              label: FILTER_LABEL[f],
              disabled: !filters.includes(f),
            }))}
            value={shownFilter}
            onChange={id => setFilter(id as Filter)}
          />
        </span>
        <span className="bc-annot hm-grid__count">
          showing {tests.length} of {allTests}
          {scope !== null &&
            ` · ${shownFilter === 'all' ? 'all tests' : FILTER_LABEL[shownFilter]} for ${focus}`}
          {' · '}
          {names.length} of {kernels.length} kernels
          {searching && ` · matching “${q.trim()}”`}
        </span>
      </div>
      <div className="hm-grid__scroll">
        <div className="hm-grid__head">
          <span className="bc-annot">area · test</span>
          {/* A kernel's name scopes the filters to its results; again, back to every kernel's. */}
          {names.map(k => {
            const on = scope !== null && focus === k
            return (
              <button
                key={k}
                type="button"
                className={`bc-annot hm-grid__kernel sc-focus${on ? ' is-on' : ''}`}
                aria-pressed={on}
                title={on ? 'Filter by every kernel again' : `Filter by ${k}’s results`}
                onClick={() => setFocus(on ? ANY_KERNEL : k)}
              >
                {k}
                {on && <span aria-hidden="true"> ×</span>}
              </button>
            )
          })}
        </div>
        {visible.map(g => (
          <details
            key={g.operation}
            id={g.operation}
            className="hm-op"
            open={isOpen(g.operation)}
            onToggle={e => {
              const v = (e.currentTarget as HTMLDetailsElement).open
              if (v !== isOpen(g.operation)) setOpen(o => ({ ...o, [g.operation]: v }))
            }}
          >
            <summary className="hm-row hm-row--op" data-row={g.operation}>
              <span className="hm-row__name">
                <span className="hm-op__name">{g.operation}</span>
                <span className="hm-op__count">{g.tests.length}</span>
              </span>
              {g.cells.map((c, i) => (
                <span key={names[i]} className="hm-cell">
                  <Share cell={c} best={i === opWinner(g)} />
                  <TierBar
                    thin
                    top={i === opWinner(g)}
                    counts={c?.counts ?? []}
                    label={caseTitle(names[i]!, g.operation, c)}
                  />
                </span>
              ))}
            </summary>
            {g.tests.map(t => {
              const rel = relativeTimes(t.cells)
              const timed = rel.filter(r => r !== null).length
              return (
                <div key={t.slug} className="hm-row" data-row={`${g.operation}/${t.slug}`}>
                  <a className="hm-row__name hm-row__test" href={url.test(g.operation, t.slug)}>
                    {t.slug.replaceAll('-', ' ')}
                  </a>
                  {t.cells.map((c, i) => {
                    const title = caseTitle(names[i]!, t.slug, c)
                    if (!c)
                      return (
                        <span key={names[i]} className="hm-cell hm-cell--none bc-annot">
                          not run
                        </span>
                      )
                    return (
                      <a
                        key={names[i]}
                        className="hm-cell"
                        data-grade={c.grade}
                        href={caseLink(names[i]!, t.test)}
                        title={`${title} · open these cases`}
                      >
                        <Frac cell={c} best={i === testWinner(t.cells)} />
                        <TierBar
                          thin
                          top={i === testWinner(t.cells)}
                          counts={c.counts}
                          label={title}
                        />
                        <Time cell={c} rel={rel[i]!} compared={timed > 1} />
                      </a>
                    )
                  })}
                </div>
              )
            })}
          </details>
        ))}
        {!visible.length && <p className="bc-annot hm-grid__empty">no test matches</p>}
      </div>
    </div>
  )
}

function Share({ cell, best }: { cell: Cell | null; best: boolean }) {
  if (!cell || !total(cell)) return null
  const share = shownShare(passing(cell), total(cell))
  return (
    <span className={`hm-frac bc-machine${best ? ' is-best' : ''}`} data-to={share * 100}>
      {fmt.pct(share)}
    </span>
  )
}

function Frac({ cell, best }: { cell: Cell | null; best: boolean }) {
  if (!cell) return null
  return (
    <span className={`hm-frac bc-machine${best ? ' is-best' : ''}`}>
      {passing(cell)}/{total(cell)}
    </span>
  )
}

function Time({ cell, rel, compared }: { cell: Cell; rel: number | null; compared: boolean }) {
  // A bar with nothing passed says there is no time to give; a report rounds a time under its
  // resolution to 0, which is said as that and not as no time at all.
  if (cell.medianMs === null || !passing(cell)) return null
  if (cell.medianMs === 0) return <span className="hm-time bc-machine">&lt;0.01 ms</span>
  if (rel === null) return <span className="hm-time bc-machine">{fmt.ms(cell.medianMs)}</span>
  const best = compared && rel === 1
  return (
    <span className={`hm-time bc-machine${best ? ' hm-time--best' : ''}`}>
      {fmt.ms(cell.medianMs)}
      {compared && (
        <span className="hm-time__rel">
          <span className="hm-time__sep"> · </span>
          {best ? 'fastest' : `×${rel.toFixed(1)}`}
        </span>
      )}
    </span>
  )
}
