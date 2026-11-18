import { Fragment, useEffect, useMemo, useRef, useState } from 'react'
import { OutlineButton } from '@/ds'
import { Select } from '@/ds'
import { fmt, GradeLabel, GradeMark, gradeLabel, SearchBox, Table, type Column } from '@/components'
import { GRADES } from '@/data/results'
import { url } from '@/site/href'
import {
  caseHash,
  filterToSearch,
  gradesOf,
  groupByTest,
  isUnfiltered,
  matches,
  NO_FILTER,
  sameFilter,
  shortCase,
  sortRows,
  type Filter,
  type Group,
  type Row,
} from './cases'

const PAGE_SIZE = 50

// A report's cases, gathered by test: a test is one row saying what its cases came to and why, and
// opens to the cases themselves. Most of a test's cases share one reason, which is then said once.
// It filters by grade and area, which the bars above set too, and by its search box.
export function CaseTable({
  rows,
  kernel,
  filter,
  opening,
  onFilter,
}: {
  rows: Row[]
  // The kernel as the tests' pages show it: null when they show another of its reports.
  kernel: string | null
  filter: Filter
  // The view the report opens on.
  opening: Filter
  onFilter: (f: Filter) => void
}) {
  // The page belongs to one view: a filter changed anywhere, in this table or outside it, starts
  // the new view at its first page.
  const view = filterToSearch(filter)
  const [paged, setPaged] = useState({ view, page: 0 })
  const page = paged.view === view ? paged.page : 0
  const setPage = (n: number) => setPaged({ view, page: n })
  const [open, setOpen] = useState<Record<string, boolean>>({})

  const counts = useMemo(() => {
    const grade = new Map<string, number>()
    const operation = new Map<string, number>()
    for (const r of rows) {
      grade.set(r.grade, (grade.get(r.grade) ?? 0) + 1)
      operation.set(r.operation, (operation.get(r.operation) ?? 0) + 1)
    }
    return { grade, operation }
  }, [rows])

  const { shown, groups } = useMemo(() => {
    const shown = sortRows(
      rows.filter(r => matches(r, filter)),
      'order',
      'asc',
    )
    return { shown, groups: groupByTest(shown, rows) }
  }, [rows, filter])

  const pages = Math.max(1, Math.ceil(groups.length / PAGE_SIZE))
  const at = Math.min(page, pages - 1)
  const slice = groups.slice(at * PAGE_SIZE, (at + 1) * PAGE_SIZE)

  const set = (patch: Partial<Filter>) => onFilter({ ...filter, ...patch })

  const short = rows.length - (counts.grade.get('1e-7') ?? 0)
  const chosen = gradesOf(filter.grade)
  const gradeOptions = [
    { id: 'all', label: 'all grades', meta: String(rows.length) },
    { id: 'below', label: 'not 1e-7', meta: String(short) },
    ...GRADES.filter(g => counts.grade.has(g)).map(g => ({
      id: g,
      label: g,
      meta: String(counts.grade.get(g)),
    })),
    // A band's set of grades, chosen from a bar, is an option while it is the filter.
    ...(chosen && chosen.length > 1
      ? [
          {
            id: filter.grade,
            label: chosen.join(' + '),
            meta: String(chosen.reduce((n, g) => n + (counts.grade.get(g) ?? 0), 0)),
          },
        ]
      : []),
  ]
  const opOptions = [
    { id: 'all', label: 'all areas', meta: String(rows.length) },
    ...[...counts.operation]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([op, n]) => ({ id: op, label: op, meta: String(n) })),
  ]

  const atOpening = sameFilter(filter, opening)
  const pager = (where: string) =>
    pages > 1 && (
      <nav className="rp-pager" aria-label={`test pages, ${where} the list`}>
        <OutlineButton size="sm" disabled={at === 0} onClick={() => setPage(at - 1)}>
          previous
        </OutlineButton>
        <span className="bc-machine">
          tests {at * PAGE_SIZE + 1}–{at * PAGE_SIZE + slice.length} of {groups.length}
        </span>
        <OutlineButton size="sm" disabled={at >= pages - 1} onClick={() => setPage(at + 1)}>
          next
        </OutlineButton>
      </nav>
    )

  return (
    <div>
      <div className="rp-filters" role="search" aria-label="filter cases">
        <Select
          id="rp-grade"
          label="grade"
          options={gradeOptions}
          value={filter.grade}
          onChange={id => set({ grade: id as Filter['grade'] })}
        />
        <Select
          id="rp-op"
          label="area"
          options={opOptions}
          value={filter.operation}
          onChange={id => set({ operation: id })}
        />
        <SearchBox label="search cases" placeholder="e.g. fillets or a case id" />
      </div>
      <div className="rp-count" aria-live="polite">
        <span className="bc-machine">
          {groups.length} {groups.length === 1 ? 'test' : 'tests'} ·{' '}
          {shown.length === rows.length
            ? `${rows.length} cases`
            : `${shown.length} of ${rows.length} cases`}
        </span>
        {filter.test !== 'all' && (
          <span className="rp-only">
            <span className="bc-annot">test</span>
            <span className="bc-machine">{filter.test}</span>
            <button
              type="button"
              className="rp-clear sc-focus"
              aria-label="show every test"
              onClick={() => set({ test: 'all' })}
            >
              ×
            </button>
          </span>
        )}
        {atOpening && !isUnfiltered(filter) && (
          <button type="button" className="rp-clear sc-focus" onClick={() => set(NO_FILTER)}>
            show all {rows.length} cases
          </button>
        )}
        {!atOpening && !isUnfiltered(filter) && (
          <button type="button" className="rp-clear sc-focus" onClick={() => set(NO_FILTER)}>
            clear filters
          </button>
        )}
        {!atOpening && isUnfiltered(filter) && short > 0 && (
          <button type="button" className="rp-clear sc-focus" onClick={() => set(opening)}>
            show only the {short} short of 1e-7
          </button>
        )}
        {pager('above')}
      </div>
      <div className="rp-groups">
        <div className="rp-group__head" aria-hidden="true">
          <span>test</span>
          <span>cases</span>
          <span>result</span>
          <span>why</span>
          <span>median time</span>
        </div>
        {slice.map(g => (
          <TestGroup
            key={g.test}
            group={g}
            kernel={kernel}
            open={open[g.test] ?? groups.length === 1}
            onToggle={v => setOpen(o => ({ ...o, [g.test]: v }))}
          />
        ))}
        {groups.length === 0 && (
          <p className="bc-annot rp-groups__empty">no case matches these filters</p>
        )}
      </div>
      {pager('below')}
    </div>
  )
}

function TestGroup({
  group: g,
  kernel,
  open,
  onToggle,
}: {
  group: Group
  kernel: string | null
  open: boolean
  onToggle: (open: boolean) => void
}) {
  const test = url.test(g.operation, g.slug)
  // One reason every shown case shares is the group's own line; anything else needs the column:
  // several reasons, or one that only some of the cases give.
  const perCase = g.whys.length > 1 || (g.whys.length === 1 && g.reasoned < g.rows.length)
  // A test opened before the page's script ran is open in the document and not yet in its state.
  const details = useRef<HTMLDetailsElement>(null)
  useEffect(() => {
    if (details.current && details.current.open !== open) onToggle(details.current.open)
  }, [])
  const columns: Column<Row>[] = [
    {
      key: 'id',
      label: 'case',
      sortable: false,
      render: r =>
        r.paged && r.listed ? (
          <a
            className="rp-case"
            href={test + caseHash(kernel, r.id)}
            title={`${r.id} on the test’s page`}
          >
            {shortCase(r.id)}
          </a>
        ) : (
          <span className="rp-cell" title={`${r.id}: not among the current results`}>
            {shortCase(r.id)}
          </span>
        ),
    },
    {
      key: 'kind',
      label: 'kind',
      sortable: false,
      render: r => <span className="rp-cell">{r.kind}</span>,
    },
    {
      key: 'grade',
      label: 'grade',
      sortable: false,
      render: r => (
        <span title={r.why || undefined}>
          <GradeLabel grade={r.grade} />
        </span>
      ),
    },
    // One reason shared by every case shown is the group's own line, not a column of repeats.
    ...(perCase
      ? [
          {
            key: 'why',
            label: 'why',
            sortable: false,
            render: (r: Row) => <span className="rp-why">{fmt.condense(r.why)}</span>,
          },
        ]
      : []),
    {
      key: 'time',
      label: 'time',
      align: 'right',
      sortable: false,
      render: r => <span className="rp-cell rp-num">{fmt.ms(r.timeMs)}</span>,
    },
  ]
  return (
    <details
      ref={details}
      className="rp-group"
      open={open}
      onToggle={e => {
        const v = (e.currentTarget as HTMLDetailsElement).open
        if (v !== open) onToggle(v)
      }}
    >
      <summary className="rp-group__row">
        {/* Closed, a test is one line (<wbr> would break it even there); open, it may wrap. */}
        <span className="rp-group__test" title={g.test}>
          {open ? <Breakable path={g.test} /> : g.test}
        </span>
        <span className="rp-cell rp-num">
          <span className="bc-sr">cases </span>
          {g.rows.length === g.total ? g.total : `${g.rows.length} of ${g.total}`}
        </span>
        <span className="rp-group__grades">
          <span className="bc-sr">result </span>
          {g.grades.map(([grade, n]) => (
            <span key={grade} className="rp-group__grade" title={`${n} ${gradeLabel(grade)}`}>
              <GradeMark grade={grade} title={gradeLabel(grade)} />
              {n} {grade === '1e-7' ? '' : gradeLabel(grade)}
            </span>
          ))}
        </span>
        <span className="rp-why" title={fmt.condense(g.whys[0] ?? '') || undefined}>
          {g.whys.length > 0 && <span className="bc-sr">why </span>}
          {fmt.condense(g.whys[0] ?? '')}
          {g.whys.length > 1 && (
            <span className="bc-annot"> +{g.whys.length - 1} other reasons</span>
          )}
          {g.whys.length === 1 && g.reasoned < g.rows.length && (
            <span className="bc-annot">
              {' '}
              · {g.reasoned} of {g.rows.length} cases
            </span>
          )}
        </span>
        <span className="rp-cell rp-num">
          <span className="bc-sr">median time </span>
          {fmt.ms(g.medianMs)}
        </span>
      </summary>
      {open && (
        <div className="rp-group__cases">
          {g.paged && (
            <a className="rp-group__link" href={test}>
              {g.test} on its own page
            </a>
          )}
          <div className="rp-table">
            <Table<Row>
              aria-label={`cases of ${g.test}`}
              rowKey="id"
              sortBy={null}
              rows={g.rows}
              columns={columns}
            />
          </div>
        </div>
      )}
    </details>
  )
}

// A test path breaks only after a slash when the column is narrow: each segment, its slash with
// it, is kept whole, so a hyphen inside a name never breaks it.
function Breakable({ path }: { path: string }) {
  const parts = path.split('/')
  return parts.map((p, i) => (
    <Fragment key={i}>
      <span className="rp-seg">{i < parts.length - 1 ? `${p}/` : p}</span>
      {i < parts.length - 1 && <wbr />}
    </Fragment>
  ))
}
