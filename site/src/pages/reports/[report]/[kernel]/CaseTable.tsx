import { Fragment, useEffect, useMemo, useRef, useState } from 'react'
import { fmt, GradeLabel, GradeMark, gradeLabel, Table, type Column } from '@/components'
import { url } from '@/site/href'
import { TierBar } from '@/pages/_home/TierBar'
import {
  byOperation,
  caseSearch,
  groupByTest,
  matches,
  shortCase,
  sortRows,
  type Filter,
  type Group,
  type Row,
} from './cases'

// A report's cases in one table: an area per row, opening to its tests, each opening to its cases.
// Most of a test's cases share one reason, which is then said once. The grade rows above the table
// set its filter.
export function CaseTable({
  rows,
  suite,
  kernel,
  filter,
  onFilter,
}: {
  rows: Row[]
  // The report the cases are in, whose compare pages the tests link to.
  suite: string
  // The kernel as the compare pages show it: null when they show another of its releases.
  kernel: string | null
  filter: Filter
  onFilter: (f: Filter) => void
}) {
  const [open, setOpen] = useState<Record<string, boolean>>({})
  const [areaOpen, setAreaOpen] = useState<Record<string, boolean>>({})

  const shown = useMemo(
    () =>
      sortRows(
        rows.filter(r => matches(r, filter)),
        'order',
        'asc',
      ),
    [rows, filter],
  )
  const areas = useMemo(() => {
    const all = new Map(byOperation(rows).map(o => [o.operation, o]))
    const tests = groupByTest(shown, rows)
    return [...new Set(tests.map(t => t.operation))]
      .sort((x, y) => x.localeCompare(y))
      .map(op => ({ ...all.get(op)!, tests: tests.filter(t => t.operation === op) }))
  }, [rows, shown])
  // A grade chosen lists its cases alone, one row each; every grade shown gathers them by area.
  const flat = filter.grade !== 'all'

  const set = (patch: Partial<Filter>) => onFilter({ ...filter, ...patch })

  // An area opens by itself where it is the only one shown or holds the one test a link names.
  const areaIsOpen = (op: string, only: boolean) =>
    areaOpen[op] ?? (only || (filter.test !== 'all' && filter.test.startsWith(`${op}/`)))

  return (
    <div>
      {filter.test !== 'all' && (
        <div className="rp-filters">
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
        </div>
      )}
      {flat && (
        <CaseList
          rows={shown}
          suite={suite}
          kernel={kernel}
          grades={new Set(shown.map(r => r.grade)).size > 1}
        />
      )}
      <div className="rp-groups" hidden={flat}>
        <div className="rp-group__head" aria-hidden="true">
          <span>area · test</span>
          <span>cases</span>
          <span>result</span>
          <span>why</span>
          <span>median time</span>
        </div>
        {areas.map(area => (
          <details
            key={area.operation}
            className="rp-area"
            open={areaIsOpen(area.operation, areas.length === 1)}
            onToggle={e => {
              const v = (e.currentTarget as HTMLDetailsElement).open
              if (v !== areaIsOpen(area.operation, areas.length === 1))
                setAreaOpen(o => ({ ...o, [area.operation]: v }))
            }}
          >
            <summary className="rp-group__row rp-area__row">
              <span className="rp-group__test rp-area__name">{area.operation}</span>
              <span className="rp-cell rp-num">
                {area.passing}/{area.total}
              </span>
              <span className="rp-area__bar">
                <TierBar
                  thin
                  counts={area.counts}
                  label={`${area.operation}: ${area.passing} of ${area.total} cases at 1e-7`}
                />
              </span>
            </summary>
            {area.tests.map(g => (
              <TestGroup
                key={g.test}
                group={g}
                suite={suite}
                kernel={kernel}
                open={open[g.test] ?? (filter.test === g.test || area.tests.length === 1)}
                onToggle={v => setOpen(o => ({ ...o, [g.test]: v }))}
              />
            ))}
          </details>
        ))}
        {areas.length === 0 && (
          <p className="bc-annot rp-groups__empty">no case matches these filters</p>
        )}
      </div>
    </div>
  )
}

// The cases a grade filter keeps, one row each, in the report's order: a case links to it on its
// test's page.
function CaseList({
  rows,
  suite,
  kernel,
  grades,
}: {
  rows: Row[]
  suite: string
  kernel: string | null
  // Whether the rows are of more than one grade, which then has a column.
  grades: boolean
}) {
  const columns: Column<Row>[] = [
    {
      key: 'test',
      label: 'test',
      sortable: false,
      // The test and the case number open one address: this case on its test's page.
      render: r =>
        r.paged && r.listed ? (
          <a className="rp-case" href={url.compare(suite, r.test) + caseSearch(kernel, r.n)}>
            {r.test}
          </a>
        ) : (
          <span className="rp-cell">{r.test}</span>
        ),
    },
    {
      key: 'n',
      label: 'case',
      align: 'right',
      sortable: false,
      render: r =>
        r.paged && r.listed ? (
          <a
            className="rp-case rp-num"
            href={url.compare(suite, r.test) + caseSearch(kernel, r.n)}
            title={`${r.id} on the test’s page`}
          >
            {r.n}
          </a>
        ) : (
          <span className="rp-cell rp-num" title={`${r.id}: not among the current results`}>
            {r.n}
          </span>
        ),
    },
    ...(grades
      ? [
          {
            key: 'grade',
            label: 'result',
            sortable: false,
            render: (r: Row) => <GradeLabel grade={r.grade} />,
          },
        ]
      : []),
    {
      key: 'why',
      label: 'why',
      sortable: false,
      render: r => <span className="rp-why">{fmt.condense(r.why)}</span>,
    },
    {
      key: 'time',
      label: 'time',
      align: 'right',
      sortable: false,
      render: r => <span className="rp-cell rp-num">{fmt.ms(r.timeMs)}</span>,
    },
  ]
  if (!rows.length)
    return <p className="bc-annot rp-groups__empty">no case matches these filters</p>
  return (
    <div className="rp-table rp-list">
      <Table<Row> aria-label="cases" rowKey="id" sortBy={null} rows={rows} columns={columns} />
    </div>
  )
}

function TestGroup({
  group: g,
  suite,
  kernel,
  open,
  onToggle,
}: {
  group: Group
  suite: string
  kernel: string | null
  open: boolean
  onToggle: (open: boolean) => void
}) {
  const test = url.compare(suite, g.test)
  // Under its area's row, a test is named by its path below the area.
  const name = g.test.slice(g.operation.length + 1) || g.test
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
            href={test + caseSearch(kernel, r.n)}
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
          {open ? <Breakable path={name} /> : name}
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
