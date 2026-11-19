import { DENSITIES, type Density } from './zebra'
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { Checkbox, Menu, Segmented } from '@/ds'
import { Annot } from '@/components'
import { zoomKeyOf } from './format'
import type { Mark, ScanHandle } from './scan'
import type { Loaded } from './viewer'
import {
  firstView,
  ghostsOf,
  keysOf,
  operandsLabel,
  shownOverlay,
  subjectOf,
  toneKeyOf,
  tonesOf,
} from './views'

type Status = 'loading' | 'ready' | 'missing' | 'unreadable' | 'undrawn'

const EMPTY = new Float32Array(0)
const NO_TRIANGLES = new Uint32Array(0)

// What a ghost of each tone is: a boolean's operands, or the solid a step or move started from.
// A kernel's drawing of a case in Scan mode, with a kernel's misplaced points over it. The drawing
// code loads after hydration; where no kernel drew the case, or the file is not published yet, the
// points are drawn on their own.
export function ScanView({
  glb,
  caption,
  marks,
  focus,
  onPick,
  files,
  unpublished = false,
  by,
  missing = null,
}: {
  // Why the drawing kernel has no result to show, as its grade, where it made none.
  missing?: string | null
  glb: string | null
  // No `glb` because every kernel's drawing is missing from the CDN, not because none drew it.
  unpublished?: boolean
  // Whose drawing this is, shown under the case's name.
  by?: ReactNode
  caption: string
  marks: Mark[]
  focus: number | null
  onPick: (i: number | null) => void
  files?: ReactNode
}) {
  const canvas = useRef<HTMLCanvasElement>(null)
  const surface = useRef<HTMLCanvasElement>(null)
  const handle = useRef<ScanHandle | null>(null)
  const [noGpu, setNoGpu] = useState(false)
  // The zoom key as the reader's keyboard names it, known only once the page runs.
  const [zoomKey, setZoomKey] = useState('ctrl')
  useEffect(() => {
    const nav = navigator as Navigator & { userAgentData?: { platform?: string } }
    setZoomKey(zoomKeyOf(nav.userAgentData?.platform, nav.platform))
  }, [])
  const pick = useRef(onPick)
  pick.current = onPick
  const [status, setStatus] = useState<Status>('loading')
  const [loaded, setLoaded] = useState<Loaded | null>(null)
  // The view shown, by name, so moving to another case or kernel keeps it where it can.
  const [viewName, setViewName] = useState<string | null>(null)
  const [operands, setOperands] = useState(true)
  const [zebra, setZebra] = useState<Density | null>(null)
  const [points, setPoints] = useState(true)
  const [box, setBox] = useState(true)
  const o = loaded?.overlay
  const hasPoints = !!(
    o?.points.length ||
    o?.closest.length ||
    o?.sides.length ||
    o?.normals.length
  )
  const hasBox = !!o?.box
  const overlay = useMemo(
    () => loaded && shownOverlay(loaded.overlay, { points, box }),
    [loaded, points, box],
  )
  const keys = useMemo(() => (loaded ? keysOf(loaded.overlay, points) : []), [loaded, points])

  const views = loaded?.views ?? []
  const shown = useMemo(() => {
    if (!views.length) return -1
    const i = viewName === null ? -1 : views.findIndex(v => v.name === viewName)
    // A drawing opens on its first view with a shape: a partial one may have shapes in no other.
    return i < 0 ? firstView(views) : i
  }, [views, viewName, missing])
  const view = shown < 0 ? null : views[shown]!
  const subject = useMemo(() => (view ? subjectOf(view) : null), [view])
  const tones = useMemo(() => (view ? tonesOf(view) : null), [view])
  const toneKey = view ? toneKeyOf(view) : null
  // A view a kernel that made no result left empty, where a ghost alone would read as the result:
  // show neither. A kernel that passed made every view, so its empty view is an empty result.
  const noResult = !!missing && !!view && !subject?.triangles.length
  const ghosts = useMemo(
    () => (view && operands && !noResult ? ghostsOf(views, view) : []),
    [views, view, operands, noResult],
  )
  const toggle = view && !noResult ? operandsLabel(views, view) : null
  // The controls under the frame stay as the last drawing set them while the next one loads, so
  // what follows the frame does not move up and back on every case.
  // A partial drawing's empty views cannot be chosen: there is nothing in them to see.
  const row = {
    views: views.map(v => ({
      name: v.name,
      empty: !!missing && !subjectOf(v).triangles.length,
    })),
    name: view?.name ?? '',
    toggle,
    hasPoints,
    hasBox,
  }
  const kept = useRef(row)
  if (status !== 'loading') kept.current = row
  const shownRow = kept.current

  useEffect(() => {
    let live = true
    setLoaded(null)
    if (!glb) {
      setStatus(unpublished ? 'missing' : 'undrawn')
      return
    }
    setStatus('loading')
    void import('./viewer').then(v =>
      v.loadViews(glb).then(
        l => {
          if (!live) return
          setLoaded(l)
          setStatus('ready')
        },
        (e: unknown) => {
          if (!live) return
          const unreadable = e instanceof Error && e.message.startsWith('glb:')
          setStatus(unreadable ? 'unreadable' : 'missing')
        },
      ),
    )
    return () => {
      live = false
    }
  }, [glb, unpublished])

  useEffect(() => {
    let live = true
    void import('./viewer').then(v => {
      if (!live || !canvas.current || !surface.current || noGpu) return
      let frame = loaded?.frame
      if (frame && marks.length) {
        const lo = [...frame.lo] as typeof frame.lo
        const hi = [...frame.hi] as typeof frame.hi
        for (const m of marks)
          for (let k = 0; k < 3; k++) {
            lo[k] = Math.min(lo[k]!, m.p[k]!)
            hi[k] = Math.max(hi[k]!, m.p[k]!)
          }
        frame = { lo, hi }
      }
      const none = { positions: EMPTY, normals: EMPTY, triangles: NO_TRIANGLES, edges: EMPTY }
      const scene = {
        ...(tones?.main ?? none),
        alt: tones?.alt ?? none,
        ghosts,
        marks,
        overlay: overlay ?? undefined,
        frame,
        zebra: zebra ? DENSITIES[zebra] : 0,
      }
      if (handle.current) return handle.current.set(scene)
      try {
        handle.current = v.mountScan(canvas.current, surface.current, scene, i => pick.current(i))
      } catch {
        setNoGpu(true)
      }
    })
    return () => {
      live = false
    }
  }, [loaded, tones, ghosts, marks, overlay, noGpu, zebra])

  useEffect(() => handle.current?.focus(focus), [focus, loaded, marks])

  const triangles = subject ? subject.triangles.length / 3 : 0

  useEffect(
    () => () => {
      handle.current?.destroy()
      handle.current = null
    },
    [],
  )

  // A result with no volume, such as the intersection of boxes that only touch, has no faces.
  const hollow = status === 'ready' && triangles === 0
  const ghosted = ghosts.some(g => g.triangles.length)
  const empty =
    noGpu || (((status !== 'ready' && status !== 'loading') || hollow) && !marks.length && !ghosted)
  const state = {
    loading: 'loading shape',
    ready: loaded ? (hollow ? (noResult ? `no result · ${missing}` : 'result · empty') : '') : '',
    missing: 'shape not published yet',
    unreadable: 'shape file unreadable',
    undrawn: 'no kernel drew this case',
  }[status]

  return (
    <>
      <div className="bc-scan" data-status={status} data-empty={empty || undefined}>
        <canvas ref={surface} className="bc-scan__canvas bc-scan__surface" aria-hidden="true" />
        <canvas
          ref={canvas}
          className="bc-scan__canvas sc-focus"
          tabIndex={empty ? -1 : 0}
          aria-label={`${caption}: drag or use the arrow keys to turn`}
          onDoubleClick={() => handle.current?.reset()}
        />
        {empty && (
          <div className="bc-scan__empty sc-hatch">
            <Annot>
              {noGpu
                ? 'shape · this browser has no WebGL2'
                : status === 'unreadable' || status === 'undrawn'
                  ? state
                  : noResult
                    ? `no result · ${missing}`
                    : hollow
                      ? 'shape · empty'
                      : 'shape · not published yet'}
            </Annot>
          </div>
        )}
        <div className="bc-scan__tl">{by && <span className="bc-scan__by">{by}</span>}</div>
        {state && (
          <div className="bc-scan__tr">
            <Annot>{state}</Annot>
          </div>
        )}
        {!empty && (
          <div className="bc-scan__bl">
            <Annot>
              drag to turn · right-drag to pan · pinch or {zoomKey}-scroll to zoom · double-click
              resets
            </Annot>
          </div>
        )}
        <i className="bc-scan__corner" />
        <i className="bc-scan__corner" />
        <i className="bc-scan__corner" />
        <i className="bc-scan__corner" />
      </div>
      {shownRow.views.length > 0 && (
        <div className="bc-views">
          {shownRow.views.length > 1 && (
            <Segmented
              label="view"
              options={shownRow.views.map(v => ({ id: v.name, label: v.name, disabled: v.empty }))}
              value={shownRow.name}
              onChange={id => setViewName(id)}
            />
          )}
          {shownRow.toggle && (
            <Checkbox
              tone="void"
              checked={operands}
              onChange={setOperands}
              label={shownRow.toggle}
            />
          )}
          {shownRow.hasPoints && (
            <Checkbox tone="void" checked={points} onChange={setPoints} label="show points" />
          )}
          {shownRow.hasBox && (
            <Checkbox tone="void" checked={box} onChange={setBox} label="show box" />
          )}
          <span className="bc-views__zebra">
            <Menu
              label="zebra stripes"
              trigger={
                <span
                  className={
                    zebra ? 'bc-views__zebra-now bc-views__zebra-now--on' : 'bc-views__zebra-now'
                  }
                >
                  zebra
                </span>
              }
              align="right"
              width={96}
              tone="void"
              items={['off', ...Object.keys(DENSITIES)].map(id => ({
                id,
                label: id,
                intent: (zebra ?? 'off') === id ? ('armed' as const) : undefined,
              }))}
              onSelect={id => setZebra(id === 'off' ? null : (id as Density))}
            />
          </span>
        </div>
      )}
      <p className="bc-shape__note" title={view?.note}>
        {view?.note ?? ' '}
      </p>
      <div className="bc-shape__foot">
        <div className="bc-shape__legend">
          {toneKey && (
            <span>
              <i className="bc-key bc-key--alt" /> {toneKey}
            </span>
          )}
          {keys.map(k => (
            <span key={k.label}>
              <i className={`bc-key bc-key--${k.mark}`} /> {k.label}
            </span>
          ))}
          {marks.some(m => m.held) && (
            <span>
              <i className="bc-key bc-key--held" /> placed inside, belongs outside
            </span>
          )}
          {marks.some(m => !m.held) && (
            <span>
              <i className="bc-key bc-key--missed" /> placed outside, belongs inside
            </span>
          )}
        </div>
        {status !== 'missing' && status !== 'unreadable' && files}
      </div>
    </>
  )
}
