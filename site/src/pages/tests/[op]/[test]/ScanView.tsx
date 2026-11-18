import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Annot } from '@/components'
import type { Mark, ScanHandle } from './scan'
import type { Loaded } from './viewer'

type Status = 'loading' | 'ready' | 'missing' | 'unreadable' | 'undrawn'

const EMPTY = new Float32Array(0)
const NO_TRIANGLES = new Uint32Array(0)

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
}: {
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
  const overlay = useRef<HTMLDivElement>(null)
  const handle = useRef<ScanHandle | null>(null)
  const pick = useRef(onPick)
  pick.current = onPick
  const [status, setStatus] = useState<Status>('loading')
  const [loaded, setLoaded] = useState<Loaded | null>(null)
  const [gl, setGl] = useState(true)

  useEffect(() => {
    let live = true
    setLoaded(null)
    if (!glb) {
      setStatus(unpublished ? 'missing' : 'undrawn')
      return
    }
    setStatus('loading')
    void import('./viewer').then(v =>
      v.loadShape(glb).then(
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
      if (!live || !canvas.current || !overlay.current) return
      const scene = {
        positions: loaded?.shape.positions ?? EMPTY,
        normals: loaded?.shape.normals ?? EMPTY,
        triangles: loaded?.shape.triangles ?? NO_TRIANGLES,
        edges: loaded?.shape.edges ?? EMPTY,
        smooth: loaded?.shape.smooth ?? EMPTY,
        marks,
      }
      if (handle.current) handle.current.set(scene)
      else {
        handle.current = v.mountScan(canvas.current, overlay.current, scene, i => pick.current(i))
        setGl(handle.current.gl)
      }
    })
    return () => {
      live = false
    }
  }, [loaded, marks])

  useEffect(() => handle.current?.focus(focus), [focus, loaded, marks])

  useEffect(
    () => () => {
      handle.current?.destroy()
      handle.current = null
    },
    [],
  )

  // A result with no volume, such as the intersection of boxes that only touch, has no faces.
  const hollow = status === 'ready' && loaded?.triangles === 0
  // The browser cannot draw the shape; the points are HTML, so they are still shown.
  const noGl = status === 'ready' && !hollow && !gl
  const empty = ((status !== 'ready' && status !== 'loading') || hollow || noGl) && !marks.length
  const state = noGl
    ? 'shape needs WebGL2'
    : {
        loading: 'loading shape',
        ready: loaded ? `glb · ${loaded.triangles.toLocaleString('en')} triangles` : '',
        missing: 'shape not published yet',
        unreadable: 'shape file unreadable',
        undrawn: 'no kernel drew this case',
      }[status]

  return (
    <>
      <div className="bc-scan" data-status={status} data-empty={empty || undefined}>
        <canvas
          ref={canvas}
          className="bc-scan__canvas sc-focus"
          tabIndex={empty ? -1 : 0}
          aria-label={`${caption}: drag or use the arrow keys to turn`}
          onDoubleClick={() => handle.current?.reset()}
        />
        <div ref={overlay} className="bc-scan__overlay" />
        {empty && (
          <div className="bc-scan__empty sc-hatch">
            <Annot>
              {status === 'unreadable' || status === 'undrawn' || noGl
                ? state
                : hollow
                  ? 'shape · empty'
                  : 'shape · not published yet'}
            </Annot>
          </div>
        )}
        <div className="bc-scan__tl">
          <Annot>scan</Annot>
          <span className="bc-scan__case bc-machine">{caption}</span>
          {by && <span className="bc-scan__by">{by}</span>}
        </div>
        <div className="bc-scan__tr">
          <Annot>{state}</Annot>
        </div>
        {!empty && (
          <div className="bc-scan__bl">
            <Annot>drag to turn · double-click resets</Annot>
          </div>
        )}
        <i className="bc-scan__corner" />
        <i className="bc-scan__corner" />
        <i className="bc-scan__corner" />
        <i className="bc-scan__corner" />
      </div>
      <div className="bc-shape__foot">
        <div className="bc-shape__legend">
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
