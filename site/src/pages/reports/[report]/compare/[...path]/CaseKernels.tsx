import { useEffect, useRef, useState, type MouseEvent } from 'react'
import { Annot, fmt, GradeLabel, Table, type Column } from '@/components'
import type { Failure, Grade } from '@/data/results'
import { fileName } from './files'
import { stepLabel } from './format'
import type { CaseFiles, KernelView } from './index.data'
import { FAILURE_GLOSS, Why } from './Why'

interface Row {
  i: number
  kernel: KernelView
  grade: Grade | null
  wrote: CaseFiles | null
  why: string
  failure: Failure | null
}

// One file a kernel wrote, a link that works before the page's script runs. A browser ignores
// `download` for a file on another origin and opens it, so a click fetches the file and saves it
// under its own name; where the fetch fails, the file opens in a new tab.
function FileLink({ href, label, kernel }: { href: string; label: string; kernel: string }) {
  const [busy, setBusy] = useState(false)
  const stop = useRef<AbortController | null>(null)
  useEffect(() => () => stop.current?.abort(), [])
  const save = async (e: MouseEvent) => {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return
    e.preventDefault()
    if (busy) return
    const ctl = new AbortController()
    stop.current = ctl
    setBusy(true)
    try {
      const res = await fetch(href, { signal: ctl.signal })
      if (!res.ok) throw new Error(`${res.status}`)
      const at = URL.createObjectURL(await res.blob())
      const link = document.createElement('a')
      link.href = at
      link.download = fileName(href)
      link.click()
      setTimeout(() => URL.revokeObjectURL(at), 1000)
    } catch {
      if (!ctl.signal.aborted) window.open(href, '_blank', 'noopener')
    } finally {
      if (!ctl.signal.aborted) setBusy(false)
    }
  }
  return (
    <a
      className="bc-dl"
      data-state={busy ? 'busy' : undefined}
      href={href}
      download={fileName(href)}
      aria-label={`download ${kernel}’s ${fileName(href)}`}
      aria-busy={busy || undefined}
      onClick={save}
    >
      {label}
    </a>
  )
}

export const filesOf = (f: CaseFiles): string[] => [...(f.glb ? [f.glb] : []), ...f.steps]

// What each kernel made of the case shown: the files it wrote, and its result with why it failed. A kernel's name shows
// its drawing in the viewer, and the row of the kernel selected is marked.
export function CaseKernels({
  shownId,
  kernels,
  grades,
  files,
  whys,
  failures,
  selected,
  onDraw,
}: {
  shownId: string
  kernels: KernelView[]
  grades: (Grade | null)[]
  files: (CaseFiles | null)[]
  // Why each kernel got its grade on this case, '' where it says nothing, and how it failed.
  whys: string[]
  failures: (Failure | null)[]
  // The kernel selected, as an index into `kernels`: the one the address names, else the one drawn.
  selected: number | null
  onDraw: (kernel: string) => void
}) {
  const rows: Row[] = kernels.map((kernel, i) => ({
    i,
    kernel,
    grade: grades[i] ?? null,
    wrote: files[i] ?? null,
    why: whys[i] ?? '',
    failure: failures[i] ?? null,
  }))
  const columns: Column<Row>[] = [
    {
      key: 'kernel',
      label: 'kernel',
      sortable: false,
      align: 'left',
      width: 220,
      render: r => {
        const has = !!r.wrote?.glb
        return (
          <span className="bc-casekernels__kernel">
            {r.i === selected ? (
              <span className="bc-casekernels__current" aria-current="true">
                {r.kernel.kernel}
              </span>
            ) : (
              <button
                type="button"
                className="bc-casekernels__name sc-focus"
                data-none={has ? undefined : ''}
                title={
                  has ? `show ${r.kernel.kernel}’s drawing` : `${r.kernel.kernel} has no drawing`
                }
                onClick={() => onDraw(r.kernel.kernel)}
              >
                {r.kernel.kernel}
              </button>
            )}
            <span className="bc-version">{fmt.version(r.kernel.version)}</span>
          </span>
        )
      },
    },
    {
      key: 'download',
      label: 'download',
      sortable: false,
      align: 'left',
      width: 150,
      render: r =>
        !r.wrote ? (
          <Annot>no files</Annot>
        ) : (
          <span className="bc-casekernels__files">
            {r.wrote.glb && (
              <FileLink key={r.wrote.glb} href={r.wrote.glb} label="glb" kernel={r.kernel.kernel} />
            )}
            {r.wrote.steps.map(step => (
              <FileLink key={step} href={step} label={stepLabel(step)} kernel={r.kernel.kernel} />
            ))}
          </span>
        ),
    },
    {
      key: 'result',
      label: 'result',
      sortable: false,
      align: 'left',
      render: r => (
        <span className="bc-casekernels__status">
          <span className="bc-casekernels__result">
            {r.grade ? <GradeLabel grade={r.grade} /> : <Annot>not run</Annot>}
            {r.failure && <span className="bc-kernels__gloss">{FAILURE_GLOSS[r.failure]}</span>}
          </span>
          {r.why && <Why text={r.why} lines={1} />}
        </span>
      ),
    },
  ]
  return (
    <div className="bc-casekernels">
      <Table<Row>
        aria-label={`each kernel on case ${shownId}`}
        rowKey="i"
        sortBy={null}
        rows={rows}
        columns={columns}
        selected={selected}
      />
    </div>
  )
}
