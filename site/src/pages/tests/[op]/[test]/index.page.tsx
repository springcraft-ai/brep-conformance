import { useEffect, useMemo, useRef, useState } from 'react'
import { Segmented } from '@/ds'
import {
  Annot,
  fmt,
  GradeLabel,
  gradeLabel,
  GradeLegend,
  GradeMark,
  PageHead,
  Section,
  Table,
} from '@/components'
import { GRADES, type Grade } from '@/data/results'
import { url } from '@/site/href'
import { drawnBy, kernelOrder, point, shortCaseIds, testName, value } from './format'
import type { CaseOption, CheckView, KernelView, Props } from './index.data'
import type { Mark } from './scan'
import { CaseKernels, filesOf, useFiles } from './CaseKernels'
import { ask, displayOf } from './files'
import { ScanView } from './ScanView'
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

// Where the browser keeps the kernel the reader chose to see.
const CHOSEN_KEY = 'brep-conformance:drawn-by'

// The grades under which a kernel made no result: a wrong result ('fail') is still a solid to see.
const NO_RESULT = new Set<Grade>(['refused', 'crash', 'timeout', 'unsupported'])
const noResult = (g: Grade | null) => (g && NO_RESULT.has(g) ? g : null)

// A kernel as the leaderboard names it: its name, linking to its report, over its version.
function KernelName({ k }: { k: KernelView }) {
  return (
    <span className="tp-kernel">
      <a className="tp-kernel__name" href={url.report(k.suite, k.kernel, k.version)}>
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

function Definition({ definition, operation, kind, kindText }: Props) {
  return (
    <details className="bc-section bc-defs">
      <summary className="bc-section__label">
        <h2 className="bc-annot bc-section__title">definition</h2>
        <Annot>api.md · {definition?.heading ?? operation}</Annot>
      </summary>
      <div className="bc-def">
        {definition ? (
          <>
            {definition.signatures.map(s => (
              <code key={s} className="bc-def__sig bc-machine">
                {s}
              </code>
            ))}
            {definition.paragraphs.map((p, i) => (
              <p key={i} className="bc-body">
                {p.map((r, j) =>
                  typeof r === 'string' ? (
                    r
                  ) : (
                    <code key={j} className="bc-machine">
                      {r.code}
                    </code>
                  ),
                )}
              </p>
            ))}
          </>
        ) : (
          <p className="bc-body">
            {operation} tests combine many functions; api.md defines each of them.
          </p>
        )}
        <div className="bc-def__kind">
          <Annot>kind</Annot>
          <span className="bc-def__kindname bc-machine">{kind}</span>
          <p className="bc-body">{kindText}</p>
        </div>
      </div>
    </details>
  )
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

const FAILURE_GLOSS = {
  geometry: 'the shape is wrong',
  measurement: 'a measured value is off',
} as const

const LONG_WHY = 220

// A kernel whose worst case put points on the wrong side, which its reason says: a failure of
// validity or of a measurement misplaces none. Only then is there a point the report could list.
const misplaces = (k: KernelView) => k.grade === 'fail' && /\bpoints?\b/i.test(k.why)

// A kernel's reason, a repeated message said once; a long one shows three lines until asked.
function Why({ text }: { text: string }) {
  const [all, setAll] = useState(false)
  const short = fmt.condense(text)
  const long = short.length > LONG_WHY
  return (
    <span className="bc-why">
      <span className="bc-why__text" data-clamp={long && !all ? '' : undefined}>
        {short}
      </span>
      {long && (
        <button type="button" className="bc-why__more sc-focus" onClick={() => setAll(!all)}>
          {all ? 'show less' : 'show all'}
        </button>
      )}
    </span>
  )
}

const warm = (c: CaseOption) => c.files.forEach(f => f && filesOf(f).forEach(u => void ask(u)))

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
    const short = shortCaseIds(caseList.map(c => c.id))
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
              <tr
                key={c.id}
                aria-selected={c.id === shownId}
                // Asked for on the press before the click, so the case's files are known sooner.
                // Not on hover or focus: passing over a long list would ask for every case in it.
                onPointerDown={() => warm(c)}
              >
                <th scope="row">
                  <button
                    type="button"
                    className="sc-focus bc-machine"
                    title={c.id}
                    onClick={() => onPick(c)}
                  >
                    <span className="bc-caselist__n">{label.get(c.id)!.n}</span>
                    {label.get(c.id)!.short}
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

export default function TestPage(props: Props) {
  const { test, operation, kind, cases, kernels, caseList } = props
  const showable = withPoints(kernels)
  const [selected, setSelected] = useState<string | null>(
    (showable[0] ?? kernels.find(k => k.worst))?.kernel ?? null,
  )
  const [focus, setFocus] = useState<number | null>(null)
  const [picked, setPicked] = useState<CaseOption | null>(null)
  const [drawer, setDrawer] = useState<string | null>(null)
  // The kernel the reader last chose to see, on any test page, kept in this browser.
  const [chosen, setChosen] = useState<string | null>(null)
  useEffect(() => {
    try {
      setChosen(localStorage.getItem(CHOSEN_KEY))
    } catch {
      // Storage refused: the choice lasts only for this page.
    }
  }, [])
  const draw = (k: string) => {
    setDrawer(k)
    setChosen(k)
    try {
      localStorage.setItem(CHOSEN_KEY, k)
    } catch {
      // As above.
    }
  }
  const order = useMemo(
    () =>
      kernelOrder(
        caseList.map(c => c.grades),
        kernels.map(k => k.rank),
      ),
    [caseList, kernels],
  )

  // A link may name the kernel and the case to show: #kernel=<id>&case=<id>.
  useEffect(() => {
    const read = () => {
      const p = new URLSearchParams(location.hash.slice(1))
      const k = p.get('kernel')
      if (k && kernels.some(x => x.kernel === k && x.worst)) setSelected(k)
      if (!p.has('kernel') && !p.has('case')) return
      setDrawer(k && kernels.some(x => x.kernel === k) ? k : null)
      setPicked(caseList.find(x => x.id === p.get('case')) ?? null)
      setFocus(null)
    }
    read()
    window.addEventListener('hashchange', read)
    return () => window.removeEventListener('hashchange', read)
  }, [kernels, caseList])

  const current = kernels.find(k => k.kernel === selected && k.worst) ?? null
  const shownId = picked?.id ?? current?.worst?.id ?? caseList[0]?.id ?? ''
  const shownCase = caseList.find(c => c.id === shownId) ?? null
  // A kernel that crashed on a case, or does not offer it, drew nothing: another's drawing shows.
  // Nor has a kernel whose drawing the CDN does not hold anything to show.
  const hydrated = useFiles()
  const published = shownCase?.files.map(f =>
    f?.glb && displayOf(f.glb, hydrated) === 'missing' ? { ...f, glb: null } : f,
  )
  const asked = kernels.findIndex(k => k.kernel === drawer)
  const drawn = !shownCase
    ? null
    : asked >= 0 && published![asked]?.glb
      ? asked
      : drawnBy(
          published!,
          shownCase.grades,
          order,
          kernels.findIndex(k => k.kernel === chosen),
        )
  const files = drawn === null ? null : shownCase!.files[drawn]!
  const wanted = asked
  // Drawn by some kernel, but by none the CDN holds yet.
  const unpublished = drawn === null && !!shownCase?.files.some(f => f?.glb)
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
  const choose = (k: string) => {
    setSelected(k)
    setDrawer(null)
    setPicked(null)
    setFocus(null)
  }
  const pickCase = (c: CaseOption) => {
    setPicked(c)
    setDrawer(null)
    setFocus(null)
  }
  const failed = kernels.filter(k => k.worst)
  const present = GRADES.filter(g => caseList.some(c => c.grades.includes(g)))
  const checked = failed.filter(k => k.worst!.checks.length)

  return (
    <div className="fn-page tp">
      <PageHead title={testName(test)} />
      <div className="fn-tools">
        <p className="fn-lede">
          <a href={`${url.home()}#${operation}`} title={`All ${operation} tests`}>
            {operation}
          </a>{' '}
          · {kind} test · {cases} {cases === 1 ? 'case' : 'cases'}
        </p>
      </div>
      <div className="bc-kernels" role="table" aria-label="Each kernel on this test">
        <div className="bc-kernels__row bc-kernels__head" role="row">
          <span role="columnheader">kernel</span>
          <span role="columnheader">result</span>
          <span role="columnheader">cases at 1e-7</span>
          <span role="columnheader">why, on its worst case</span>
        </div>
        {kernels.map(k => {
          const on = onWorst && current?.kernel === k.kernel
          return (
            <div
              key={k.kernel}
              className="bc-kernels__row"
              role="row"
              aria-current={on || undefined}
            >
              <span role="rowheader">
                <KernelName k={k} />
              </span>
              <span className="bc-kernels__result" role="cell">
                <GradeLabel grade={k.grade} />
                {k.failure && <span className="bc-kernels__gloss">{FAILURE_GLOSS[k.failure]}</span>}
              </span>
              <span className="bc-machine bc-kernels__n" role="cell">
                {k.passing}/{k.total}
              </span>
              <span className="bc-kernels__why" role="cell">
                {k.why ? <Why text={k.why} /> : '–'}
                {k.worst &&
                  (on ? (
                    <span className="bc-annot">showing its worst case</span>
                  ) : (
                    <button
                      type="button"
                      className="bc-kernels__show sc-focus"
                      onClick={() => choose(k.kernel)}
                    >
                      show its worst case
                    </button>
                  ))}
              </span>
            </div>
          )
        })}
      </div>

      <Section
        label="cases"
        aside={`${cases} ${cases === 1 ? 'case' : 'cases'} × ${kernels.length} kernels`}
      >
        <div className="bc-cases">
          <div className="bc-cases__list">
            <CaseList caseList={caseList} kernels={kernels} shownId={shownId} onPick={pickCase} />
            <GradeLegend grades={present} marks="table" />
          </div>
          <div className="bc-cases__shape">
            {shownCase && (
              <div className="bc-thiscase">
                <span className="bc-thiscase__id">
                  <Annot>case</Annot>
                  <span className="bc-machine">{shownId}</span>
                </span>
                <CaseKernels
                  test={test}
                  shownId={shownId}
                  kernels={kernels}
                  grades={shownCase.grades}
                  files={shownCase.files}
                  drawn={drawn}
                  onDraw={draw}
                />
              </div>
            )}
            <ScanView
              glb={files?.glb ?? null}
              unpublished={unpublished}
              missing={noResult(drawn === null ? null : shownCase!.grades[drawn]!)}
              caption={shownId}
              marks={marks}
              focus={focus}
              onPick={setFocus}
              by={
                drawn !== null && (
                  <>
                    {wanted >= 0 && wanted !== drawn && (
                      <span className="bc-scan__instead">
                        {kernels[wanted]!.kernel} has no drawing of this case ·{' '}
                      </span>
                    )}
                    drawn by <b>{kernels[drawn]!.kernel}</b>
                  </>
                )
              }
            />
            {current && rows.length > 0 && (
              <div className="bc-points">
                <p className="bc-body">
                  Sample points {current.kernel} placed on the wrong side of the boundary in case{' '}
                  <span className="bc-machine">{current.worst!.id}</span>. Select one to mark it.
                </p>
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
            )}
            {current && onWorst && rows.length === 0 && misplaces(current) && (
              <p className="bc-annot bc-points__none">
                no points drawn: the report does not list where {current.kernel} misplaced them
              </p>
            )}
          </div>
        </div>
      </Section>

      {checked.length > 0 && (
        <Section label="checks" aside="contract clauses, worst case per kernel">
          {checked.map(k => (
            <ChecksOf key={k.kernel} k={k} />
          ))}
        </Section>
      )}

      <Definition {...props} />
    </div>
  )
}
