import { useEffect, useRef, useState, useSyncExternalStore, type MouseEvent } from 'react'
import { Annot, GradeLabel, Table, type Column } from '@/components'
import type { Grade } from '@/data/results'
import { url } from '@/site/href'
import {
  ask,
  displayOf,
  fileName,
  learn,
  stateOf,
  stateOfResponse,
  storeVersion,
  subscribe,
} from './files'
import { stepLabel } from './format'
import type { CaseFiles, KernelView } from './index.data'

interface Row {
  i: number
  kernel: KernelView
  grade: Grade | null
  wrote: CaseFiles | null
}

// Reads the page's file store, drawing again when it learns of a file. False for the page's own
// HTML and the first render over it, which must agree; true after.
export function useFiles(): boolean {
  return useSyncExternalStore(subscribe, storeVersion, () => -1) >= 0
}

// One file a kernel wrote. It is a link unless the CDN says it does not hold the file, so it works
// before the page's script runs, and where the CDN could not be asked.
function FileLink({ href, label, kernel }: { href: string; label: string; kernel: string }) {
  const hydrated = useFiles()
  const [busy, setBusy] = useState(false)
  const [plain, setPlain] = useState(false)
  const stop = useRef<AbortController | null>(null)
  useEffect(() => () => stop.current?.abort(), [])
  const display = displayOf(href, hydrated)
  if (display === 'missing')
    return (
      <span className="bc-dl" data-state="missing" title="This file is not on the CDN yet">
        {label}
        <span className="bc-sr">, not published</span>
      </span>
    )
  // A browser ignores `download` for a file on another origin and opens it instead. Where the CDN
  // has answered the page's own request for the file, the page can fetch it and save it under its
  // name; anywhere else the link is left to the browser.
  const saves = stateOf(href) === 'ok' && !plain
  const save = async (e: MouseEvent) => {
    if (!saves || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return
    e.preventDefault()
    if (busy) return
    const ctl = new AbortController()
    stop.current = ctl
    setBusy(true)
    try {
      const res = await fetch(href, { signal: ctl.signal })
      const state = stateOfResponse(res)
      if (state === 'missing') return learn(href, 'missing')
      if (state !== 'ok') return setPlain(true)
      const at = URL.createObjectURL(await res.blob())
      const a = document.createElement('a')
      a.href = at
      a.download = fileName(href)
      a.click()
      setTimeout(() => URL.revokeObjectURL(at), 1000)
    } catch {
      if (!ctl.signal.aborted) setPlain(true)
    } finally {
      if (!ctl.signal.aborted) setBusy(false)
    }
  }
  return (
    <a
      className="bc-dl"
      data-state={busy ? 'busy' : display === 'checking' ? 'checking' : undefined}
      href={href}
      download={fileName(href)}
      target={saves ? undefined : '_blank'}
      rel={saves ? undefined : 'noopener'}
      aria-label={`download ${kernel}’s ${fileName(href)}`}
      aria-busy={busy || undefined}
      onClick={save}
    >
      {label}
    </a>
  )
}

export const filesOf = (f: CaseFiles): string[] => [...(f.glb ? [f.glb] : []), ...f.steps]

// What each kernel made of the case shown: its result, whose drawing the viewer holds, and the
// files it wrote.
export function CaseKernels({
  test,
  shownId,
  kernels,
  grades,
  files,
  drawn,
  onDraw,
}: {
  test: string
  shownId: string
  kernels: KernelView[]
  grades: (Grade | null)[]
  files: (CaseFiles | null)[]
  // The kernel whose drawing the viewer shows, as an index into `kernels`.
  drawn: number | null
  onDraw: (kernel: string) => void
}) {
  const hydrated = useFiles()
  const key = files.flatMap(f => (f ? filesOf(f) : [])).join('\n')
  // Asked for once on show; the store keeps each answer, so a case seen before is not asked again.
  useEffect(() => {
    for (const u of key ? key.split('\n') : []) void ask(u)
  }, [key])
  const gone = (u: string) => displayOf(u, hydrated) === 'missing'
  const q = new URLSearchParams({ test, q: shownId }).toString()
  const rows: Row[] = kernels.map((kernel, i) => ({
    i,
    kernel,
    grade: grades[i] ?? null,
    wrote: files[i] ?? null,
  }))
  const columns: Column<Row>[] = [
    {
      key: 'kernel',
      label: 'kernel',
      sortable: false,
      render: r => (
        <a
          href={`${url.report(r.kernel.suite, r.kernel.kernel, r.kernel.version)}?${q}#cases`}
          title={`this case in ${r.kernel.kernel}’s report`}
        >
          {r.kernel.kernel}
        </a>
      ),
    },
    {
      key: 'result',
      label: 'result',
      sortable: false,
      render: r => (
        <span className="bc-casekernels__result">
          {r.grade ? <GradeLabel grade={r.grade} /> : <Annot>not run</Annot>}
          {r.kernel.worst?.id === shownId && <Annot>its worst case</Annot>}
        </span>
      ),
    },
    {
      key: 'viewer',
      label: 'viewer',
      sortable: false,
      render: r =>
        !r.wrote?.glb ? (
          <Annot>no drawing</Annot>
        ) : gone(r.wrote.glb) ? (
          <Annot>not published</Annot>
        ) : r.i === drawn ? (
          <span className="bc-casekernels__shown">
            <Annot>shown</Annot>
          </span>
        ) : (
          <button
            type="button"
            className="bc-kernels__show sc-focus"
            aria-label={`show ${r.kernel.kernel}’s drawing`}
            onClick={() => onDraw(r.kernel.kernel)}
          >
            show
          </button>
        ),
    },
    {
      key: 'download',
      label: 'download',
      sortable: false,
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
            {filesOf(r.wrote).every(gone) && (
              <span aria-hidden="true">
                <Annot>not published</Annot>
              </span>
            )}
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
        selected={drawn}
      />
    </div>
  )
}
