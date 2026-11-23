import { useEffect, useRef, useState } from 'react'
import { Segmented } from '@/ds'
import { fmt, SearchBox } from '@/components'
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
  type GridTest,
  type GridGroup,
} from './model'
import { KernelPicker } from './KernelPicker'
import { TierBar } from './TierBar'

const ANY_KERNEL = 'any'

// The operations' bars fill and their shares count up once, as the page loads, as the
// leaderboard's do; the tests under them are drawn as they are. With reduced motion everything is
// drawn as it is.
const COUNT_MS = 1100
// The last block's fill starts as late as the fill time itself, then takes that long again.
const PLAY_MS = 2 * COUNT_MS
function countUp(el: HTMLElement) {
  const text = el.firstChild
  if (!text) return
  const to = Number(el.dataset.to)
  const t0 = performance.now()
  const step = (t: number) => {
    const x = Math.min(1, Math.max(0, (t - t0) / COUNT_MS))
    text.nodeValue = (to * (1 - Math.pow(1 - x, 3))).toFixed(1) + '%'
    if (x < 1) requestAnimationFrame(step)
  }
  requestAnimationFrame(step)
}
function usePlayOnLoad(root: React.RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    const el = root.current
    if (!el || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    el.classList.add('hm-grid--play')
    for (const n of el.querySelectorAll<HTMLElement>('.hm-row--op [data-to]')) countUp(n)
    // Dropped once played, so an operation the filter or the search draws again is not replayed.
    const t = setTimeout(() => el.classList.remove('hm-grid--play'), PLAY_MS)
    return () => clearTimeout(t)
  }, [root])
}

// Every test against the kernels compared, one <details> per operation so it reads without JS.
// Hydrated, it adds the kernel picker, a filter, the search, and open/close all. A cell opens that
// kernel's cases on the test.
export function Grid({
  groups,
  kernels,
  shown,
  onToggleKernel,
  suite,
  pages,
  level,
  q,
  onSearch,
}: {
  groups: GridGroup[]
  kernels: string[]
  shown: string[]
  // Shows or hides one kernel's column, under toggleCompare's limits.
  onToggleKernel: (kernel: string) => void
  // The report the grid is of, a suite version, and each kernel's page in it.
  suite: string
  pages: Record<string, string>
  // The level of the suite's tree the grid is the page of, such as booleans/degenerate: its tests
  // are listed alone, named by their path below it, with no operation's row above them.
  level?: string
  q: string
  onSearch: (q: string) => void
}) {
  const root = useRef<HTMLDivElement>(null)
  usePlayOnLoad(root)
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
    `${url.kernel(suite, pages[kernel]!)}?test=${encodeURIComponent(test)}#cases`

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
        <SearchBox
          label="search tests"
          placeholder="e.g. fillets or cylinders"
          value={q}
          onChange={onSearch}
        />
        {/* The count, and at the right edge the kernel picker: outside the filters, which on a
            phone scroll sideways and would clip its panel. */}
        <span className="hm-grid__end">
          <span className="bc-annot hm-grid__count">
            showing {tests.length} of {allTests}
            {scope !== null &&
              ` · ${shownFilter === 'all' ? 'all tests' : FILTER_LABEL[shownFilter]} for ${focus}`}
            {searching && ` · matching “${q.trim()}”`}
          </span>
          {kernels.length > 0 && (
            <KernelPicker kernels={kernels} shown={names} onToggle={onToggleKernel} />
          )}
        </span>
      </div>
      {/* With every kernel unticked, there is no column to draw. */}
      {names.length === 0 ? (
        <p className="bc-annot hm-grid__empty">no kernel shown · tick one under kernels</p>
      ) : (
        <div className="hm-grid__scroll">
          <div className="hm-grid__head">
            <span className="bc-annot">{level ? 'test' : 'area · test'}</span>
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
          {level !== undefined ? (
            <div className="hm-tests">
              {tests.map(t =>
                testRow(
                  t,
                  t.test
                    .slice(level.length + 1)
                    .replaceAll('/', ' ')
                    .replaceAll('-', ' '),
                ),
              )}
            </div>
          ) : (
            visible.map(g => (
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
                      <Share cell={c} />
                      <TierBar
                        thin
                        counts={c?.counts ?? []}
                        label={caseTitle(names[i]!, g.operation, c)}
                      />
                    </span>
                  ))}
                </summary>
                {g.tests.map(t => testRow(t, t.slug.replaceAll('-', ' ')))}
              </details>
            ))
          )}
          {!visible.length && <p className="bc-annot hm-grid__empty">no test matches</p>}
        </div>
      )}
    </div>
  )

  function testRow(t: GridTest, name: string) {
    const rel = relativeTimes(t.cells)
    const timed = rel.filter(r => r !== null).length
    return (
      <div key={t.test} className="hm-row" data-row={t.test}>
        <a
          className={`hm-row__name hm-row__test${level !== undefined ? ' hm-row__test--flat' : ''}`}
          href={url.compare(suite, t.test)}
        >
          {name}
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
              <Frac cell={c} />
              <TierBar thin counts={c.counts} label={title} />
              <Time cell={c} rel={rel[i]!} compared={timed > 1} />
            </a>
          )
        })}
      </div>
    )
  }
}

function Share({ cell }: { cell: Cell | null }) {
  if (!cell || !total(cell)) return null
  const share = shownShare(passing(cell), total(cell))
  return (
    <span className="hm-frac bc-machine" data-to={share * 100}>
      {fmt.pct(share)}
    </span>
  )
}

function Frac({ cell }: { cell: Cell | null }) {
  if (!cell) return null
  return (
    <span className="hm-frac bc-machine">
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
  return (
    <span className="hm-time bc-machine">
      {fmt.ms(cell.medianMs)}
      {compared && (
        <span className="hm-time__rel">
          <span className="hm-time__sep"> · </span>×{rel.toFixed(1)}
        </span>
      )}
    </span>
  )
}
