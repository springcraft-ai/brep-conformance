import { useEffect, useMemo, useRef, useState } from 'react'
import { useUrlSearch } from '@/components/search'
import { Segmented } from '@/ds'
import { Annot, fmt, GradeLabel, gradeLabel, GradeMark, PageHead, Table } from '@/components'
import { url } from '@/site/href'
import {
  caseFamilies,
  drawnBy,
  kernelOrder,
  madeNoResult,
  point,
  testName,
  trailOf,
  value,
} from './format'
import type { CaseOption, CheckView, KernelView, Props, TestProps } from './index.data'
import { LevelPage } from './LevelPage'
import type { Mark } from './scan'
import { CaseKernels } from './CaseKernels'
import { ScanView } from './ScanView'
import { sectionOf } from './section'
import './page.css'

export const hydrate = true

interface PointRow {
  i: number
  p: [number, number, number]
  got: boolean
  want: boolean
  clause: string
}

const placed = (inside: boolean) => (inside ? 'inside' : 'outside')

// The case a link names: its number in the list, from 1, or its id.
function caseAt(caseList: CaseOption[], v: string | null): CaseOption | null {
  if (v === null) return null
  return /^[1-9]\d*$/.test(v)
    ? (caseList[Number(v) - 1] ?? null)
    : (caseList.find(c => c.id === v) ?? null)
}

// A kernel as the leaderboard names it: its name, linking to its report, over its version.
function KernelName({ k }: { k: KernelView }) {
  return (
    <span className="tp-kernel">
      <a className="tp-kernel__name" href={url.kernel(k.suite, k.segment)}>
        {k.kernel}
      </a>
      <span className="tp-kernel__version">{fmt.version(k.version)}</span>
    </span>
  )
}

// The kernels whose worst case has points to show, in report order.
function withPoints(kernels: KernelView[]): KernelView[] {
  return kernels.filter(k => k.worst?.checks.some(c => c.points.length))
}

function ChecksOf({ k }: { k: KernelView }) {
  const worst = k.worst!
  return (
    <div className="bc-checks">
      <div className="bc-checks__head">
        <KernelName k={k} />
        <GradeLabel grade={k.grade} />
        {k.failure && <Annot>{k.failure}</Annot>}
        <span className="bc-checks__case bc-machine">{worst.id}</span>
      </div>
      <div className="bc-scroll">
        <Table<CheckView & { key: string }>
          rowKey="key"
          sortBy={null}
          rows={worst.checks.map((c, i) => ({ ...c, key: `${c.clause}-${i}` }))}
          columns={[
            {
              key: 'clause',
              label: 'clause',
              render: c => (
                <span className="bc-clause">
                  <span className="bc-machine">{c.clause}</span>
                  <span className="bc-clause__promise">{c.promise}</span>
                </span>
              ),
            },
            { key: 'measure', label: 'measure' },
            { key: 'got', label: 'got', align: 'right', render: c => value(c.got) },
            { key: 'want', label: 'want', align: 'right', render: c => value(c.want) },
            { key: 'error', label: 'error', align: 'right', render: c => value(c.error) },
            { key: 'bound', label: 'bound', align: 'right', render: c => value(c.bound) },
            { key: 'unit', label: 'unit' },
          ]}
        />
      </div>
    </div>
  )
}

// A kernel whose worst case put points on the wrong side, which its reason says: a failure of
// validity or of a measurement misplaces none. Only then is there a point the report could list.
const misplaces = (k: KernelView) => k.grade === 'fail' && /\bpoints?\b/i.test(k.why)

// Every case of the test against every kernel, one mark each. A row shows that case's shape.
function CaseList({
  caseList,
  kernels,
  shownId,
  onPick,
}: {
  caseList: CaseOption[]
  kernels: KernelView[]
  shownId: string
  onPick: (c: CaseOption) => void
}) {
  const [only, setOnly] = useState<'all' | 'short'>('all')
  const scroll = useRef<HTMLDivElement>(null)

  // Brings the case shown into the list's own view, without moving the page.
  useEffect(() => {
    const box = scroll.current
    const row = box?.querySelector<HTMLElement>('tr[aria-selected="true"]')
    if (!box || !row) return
    const head = box.querySelector('thead')?.clientHeight ?? 0
    const top = row.offsetTop - head
    if (top < box.scrollTop || top + row.clientHeight > box.scrollTop + box.clientHeight - head)
      box.scrollTop = Math.max(0, top - (box.clientHeight - head) / 2)
  }, [shownId])
  const label = useMemo(() => {
    const short = caseFamilies(caseList.map(c => c.id))
    const width = String(caseList.length).length
    return new Map(
      caseList.map((c, i) => [c.id, { n: String(i + 1).padStart(width, '0'), short: short[i]! }]),
    )
  }, [caseList])
  const short = caseList.filter(c => c.grades.some(g => g !== null && g !== '1e-7'))
  const rows = only === 'short' && short.length ? short : caseList
  return (
    <div className="bc-caselist">
      <Segmented
        label="cases"
        options={[
          { id: 'all', label: `all ${caseList.length}` },
          {
            id: 'short',
            label: `any short ${short.length}`,
            disabled: short.length === 0 || short.length === caseList.length,
          },
        ]}
        value={rows === short ? 'short' : 'all'}
        onChange={id => setOnly(id as 'all' | 'short')}
      />
      <div className="bc-caselist__scroll" ref={scroll}>
        <table>
          <thead>
            <tr>
              <th scope="col">case</th>
              {kernels.map(k => (
                <th key={k.kernel} scope="col" className="bc-caselist__kernel">
                  <span>{k.kernel}</span>
                </th>
              ))}
              <th aria-hidden="true" className="bc-caselist__lean" />
            </tr>
          </thead>
          <tbody>
            {rows.map(c => (
              <tr key={c.id} aria-selected={c.id === shownId}>
                <th scope="row">
                  <button
                    type="button"
                    className="sc-focus bc-machine"
                    title={c.id}
                    onClick={() => onPick(c)}
                  >
                    <span className="bc-caselist__n">{label.get(c.id)!.n}</span>
                    {label.get(c.id)!.short && ` ${label.get(c.id)!.short}`}
                  </button>
                </th>
                {c.grades.map((g, i) => (
                  <td key={kernels[i]!.kernel}>
                    {g ? (
                      <GradeMark grade={g} title={`${kernels[i]!.kernel}: ${gradeLabel(g)}`} />
                    ) : (
                      <span title={`${kernels[i]!.kernel}: not run`}>–</span>
                    )}
                  </td>
                ))}
                <td aria-hidden="true" />
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

export default function ComparePage(props: Props) {
  return props.page === 'level' ? <LevelPage {...props} /> : <TestPage {...props} />
}

function TestPage(props: TestProps) {
  const { suite, test, kernels, caseList } = props
  const showable = withPoints(kernels)
  // The view is named by the address alone: ?kernel=<id>&case=<number>, or the case's id. Every
  // control here changes the address, and the page draws what it names.
  const [search, setSearch] = useUrlSearch()
  const asked = kernels.findIndex(k => k.kernel === search.get('kernel'))
  const picked = caseAt(caseList, search.get('case'))
  const [focus, setFocus] = useState<number | null>(null)
  useEffect(() => setFocus(null), [search.toString()])
  const order = useMemo(
    () =>
      kernelOrder(
        caseList.map(c => c.grades),
        kernels.map(k => k.rank),
      ),
    [caseList, kernels],
  )

  // Whose misplaced points show: the kernel the address names, where it failed a case; else the
  // first kernel with points to show.
  const current =
    (asked >= 0 && kernels[asked]!.worst ? kernels[asked]! : null) ??
    showable[0] ??
    kernels.find(k => k.worst) ??
    null
  const shownId = picked?.id ?? current?.worst?.id ?? caseList[0]?.id ?? ''
  const shownCase = caseList.find(c => c.id === shownId) ?? null
  const numberOf = (id: string | undefined) => String(caseList.findIndex(c => c.id === id) + 1)
  // The address's kernel where it has a result to draw; else the best kernel that has one, and a
  // warning where the address named a kernel. One address always shows one drawing.
  const drawn = shownCase ? drawnBy(shownCase.files, shownCase.grades, order, asked) : null
  const files = drawn === null ? null : shownCase!.files[drawn]!
  // A drawing of a kernel that made no result holds only what it made before it stopped.
  const partial = drawn === null ? null : shownCase!.grades[drawn]!
  const partialGrade = partial && madeNoResult(partial) ? partial : null
  const wanted = asked
  // A kernel's misplaced points belong to its worst case, so they show only over that shape.
  const onWorst = current?.worst?.id === shownId
  const rows: PointRow[] = useMemo(
    () =>
      ((onWorst && current?.worst?.checks) || [])
        .flatMap(c => c.points.map(q => ({ ...q, clause: c.clause, i: 0 })))
        .map((r, i) => ({ ...r, i })),
    [current, onWorst],
  )
  const marks: Mark[] = useMemo(() => rows.map(r => ({ p: r.p, held: r.got })), [rows])
  const draw = (k: string) =>
    setSearch([
      ['kernel', k],
      ['case', numberOf(shownId)],
    ])
  const pickCase = (c: CaseOption) =>
    setSearch([
      ['kernel', search.get('kernel')],
      ['case', numberOf(c.id)],
    ])
  const failed = kernels.filter(k => k.worst)
  const checked = failed.filter(k => k.worst!.checks.length)

  return (
    <div className="fn-page tp">
      <PageHead trail={trailOf(suite, test)} title={testName(test)} />
      <section className="bc-section" aria-label="cases">
        <div className="bc-cases">
          <div className="bc-cases__shape">
            <ScanView
              glb={files?.glb ?? null}
              caption={shownId}
              marks={marks}
              focus={focus}
              onPick={setFocus}
              missing={partialGrade}
              section={sectionOf(test)}
              // A warning, only where the drawing is not the whole result of the kernel selected:
              // another kernel's drawing stands in, or the drawing is partial.
              by={
                drawn === null ? null : wanted >= 0 && wanted !== drawn ? (
                  <>
                    {kernels[wanted]!.kernel} has no drawing · showing {kernels[drawn]!.kernel}’s
                  </>
                ) : partialGrade ? (
                  <>
                    {kernels[drawn]!.kernel} {partialGrade} · showing only what it made before it
                    stopped
                  </>
                ) : null
              }
            />
            {current && rows.length > 0 && (
              <div className="bc-points">
                <p className="bc-body">
                  Sample points {current.kernel} placed on the wrong side of the boundary in case{' '}
                  <span className="bc-machine">{current.worst!.id}</span>. Select one to mark it.
                </p>
                <div className="bc-points__scroll">
                  <Table<PointRow>
                    rowKey="i"
                    sortBy={null}
                    rows={rows}
                    selected={focus}
                    onRow={r => setFocus(r.i === focus ? null : r.i)}
                    columns={[
                      { key: 'i', label: '#', align: 'right', render: r => r.i + 1 },
                      { key: 'p', label: 'point · mm', render: r => point(r.p) },
                      { key: 'clause', label: 'clause' },
                      { key: 'got', label: 'kernel', render: r => placed(r.got) },
                      { key: 'want', label: 'shape', render: r => placed(r.want) },
                    ]}
                  />
                </div>
              </div>
            )}
            {current && onWorst && rows.length === 0 && misplaces(current) && (
              <p className="bc-annot bc-points__none">
                no points drawn: the report does not list where {current.kernel} misplaced them
              </p>
            )}
            {shownCase && (
              <div className="bc-thiscase">
                <CaseKernels
                  shownId={shownId}
                  kernels={kernels}
                  grades={shownCase.grades}
                  files={shownCase.files}
                  whys={shownCase.whys}
                  failures={shownCase.failures}
                  selected={asked >= 0 ? asked : drawn}
                  onDraw={draw}
                />
              </div>
            )}
          </div>
          <div className="bc-cases__list">
            <div className="bc-cases__table">
              <CaseList caseList={caseList} kernels={kernels} shownId={shownId} onPick={pickCase} />
            </div>
          </div>
        </div>
      </section>

      {checked.length > 0 && (
        <details className="bc-section bc-fold">
          <summary className="bc-section__label">
            <h2 className="bc-annot bc-section__title">checks</h2>
            <Annot>contract clauses, worst case per kernel</Annot>
          </summary>
          {checked.map(k => (
            <ChecksOf key={k.kernel} k={k} />
          ))}
        </details>
      )}
    </div>
  )
}
