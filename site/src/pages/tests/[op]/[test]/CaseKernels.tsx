import { useEffect, useState, type MouseEvent } from 'react'
import { Annot, GradeLabel, Table, type Column } from '@/components'
import type { Grade } from '@/data/results'
import { url } from '@/site/href'
import { fileName, probe, type FileState } from './files'
import { stepLabel } from './format'
import type { CaseFiles, KernelView } from './index.data'

interface Row {
  i: number
  kernel: KernelView
  grade: Grade | null
  wrote: CaseFiles | null
}

// Asks the CDN for each file once it is on show, so a link to a file that is not there never
// takes the reader to the CDN's own error.
function useFileStates(urls: string[]): Record<string, FileState> {
  const [states, setStates] = useState<Record<string, FileState>>({})
  const key = urls.join('\n')
  useEffect(() => {
    const stop = new AbortController()
    setStates({})
    for (const u of key ? key.split('\n') : [])
      void probe(u, fetch, stop.signal).then(state => {
        if (!stop.signal.aborted) setStates(known => ({ ...known, [u]: state }))
      })
    return () => stop.abort()
  }, [key])
  return states
}

// A browser ignores `download` for a file on another origin and opens it instead, so the page
// fetches the file and saves it under its own name.
function FileLink({ href, label, state }: { href: string; label: string; state: FileState }) {
  const [failed, setFailed] = useState(false)
  if (state === 'missing' || failed)
    return (
      <span className="bc-dl bc-dl--missing" title="This file is not on the CDN yet">
        {label} <Annot>not published</Annot>
      </span>
    )
  const save = async (e: MouseEvent) => {
    e.preventDefault()
    try {
      const res = await fetch(href)
      if (!res.ok) return setFailed(true)
      const at = URL.createObjectURL(await res.blob())
      const a = document.createElement('a')
      a.href = at
      a.download = fileName(href)
      a.click()
      setTimeout(() => URL.revokeObjectURL(at), 1000)
    } catch {
      window.location.assign(href)
    }
  }
  return (
    <a className="bc-dl" href={href} download={fileName(href)} onClick={save}>
      {label}
    </a>
  )
}

const filesOf = (f: CaseFiles): string[] => [...(f.glb ? [f.glb] : []), ...f.steps]

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
  const states = useFileStates(files.flatMap(f => (f ? filesOf(f) : [])))
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
          href={`${url.report(r.kernel.suite, r.kernel.kernel, r.kernel.version)}?test=${encodeURIComponent(test)}&q=${encodeURIComponent(shownId)}#cases`}
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
        ) : states[r.wrote.glb] === 'missing' ? (
          <Annot>not published</Annot>
        ) : r.i === drawn ? (
          <Annot>shown</Annot>
        ) : (
          <button
            type="button"
            className="bc-kernels__show sc-focus"
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
        ) : filesOf(r.wrote).every(u => states[u] === 'missing') ? (
          <Annot>not published</Annot>
        ) : (
          <span className="bc-casekernels__files">
            {r.wrote.glb && (
              <FileLink
                key={r.wrote.glb}
                href={r.wrote.glb}
                label="glb"
                state={states[r.wrote.glb] ?? 'unknown'}
              />
            )}
            {r.wrote.steps.map(step => (
              <FileLink
                key={step}
                href={step}
                label={stepLabel(step)}
                state={states[step] ?? 'unknown'}
              />
            ))}
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
      />
    </div>
  )
}
