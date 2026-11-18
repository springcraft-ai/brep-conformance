import { useEffect, useState, type ReactNode } from 'react'
import { fmt, groupLabel, markGroups } from '@/components'
import { GRADES, type Grade } from '@/data/results'
import { drawn, type Counts } from './model'

// What a band means, in a few words. Keyed by the first grade of each group that shares a mark.
const MEANING: Partial<Record<Grade, string>> = {
  '1e-7': 'every check held at 1e-7 mm',
  '1e-6': 'held at 1e-6 mm, not 1e-7',
  '1e-5': 'held at 1e-5 mm, not 1e-7',
  '1e-3': 'held at 1e-3 mm, not tighter',
  unsupported: 'an operation the kernel does not offer',
  refused: 'declined, or out of time',
  fail: 'a wrong answer, or a crash',
}

// The tip sits below and right of the pointer, as a cursor tip does, kept inside the window.
export const tipAt = (x: number, y: number) => ({
  left: Math.max(16, Math.min(x + 14, window.innerWidth - 300)),
  top: y + 18,
})

interface Tip {
  group: Grade[]
  x: number
  y: number
}

// The band nearest the pointer across the bar, so a gap between bands or a band thinner than the
// pointer still picks one. The others dim.
function pick(wrap: HTMLElement, x: number): Grade[] | null {
  const segs = [...wrap.querySelectorAll<HTMLElement>('.hm-bar__seg')]
  let hot: HTMLElement | null = null
  let best = Infinity
  for (const s of segs) {
    const r = s.getBoundingClientRect()
    const d = x < r.left ? r.left - x : x > r.right ? x - r.right : 0
    if (d < best) {
      best = d
      hot = s
    }
  }
  const grade = hot?.dataset.grade as Grade | undefined
  const group = grade ? (markGroups().find(g => g.includes(grade)) ?? [grade]) : null
  wrap.querySelector('.hm-bar')?.classList.toggle('is-picking', group !== null)
  for (const s of segs) s.classList.toggle('is-hot', !!group?.includes(s.dataset.grade as Grade))
  return group
}

function unpick(wrap: HTMLElement) {
  wrap.querySelector('.hm-bar')?.classList.remove('is-picking')
  for (const s of wrap.querySelectorAll('.hm-bar__seg')) s.classList.remove('is-hot')
}

// A small tip at the pointer for the one band it is on: its cases, their share, what the band
// means, and that a click lists them. Fixed to the window, since the leaderboard scrolls sideways
// and would clip it. Each band's link label carries the same counts, so the tip is
// for the eye only.
export function BarCard({ counts, children }: { counts: Counts; children: ReactNode }) {
  const [tip, setTip] = useState<Tip | null>(null)
  counts = drawn(counts)
  const total = counts.reduce((a, b) => a + b, 0)

  useEffect(() => {
    if (!tip) return
    const close = () => setTip(null)
    window.addEventListener('scroll', close, { capture: true, passive: true })
    window.addEventListener('resize', close)
    return () => {
      window.removeEventListener('scroll', close, { capture: true })
      window.removeEventListener('resize', close)
    }
  }, [tip])

  const show = (wrap: HTMLElement, x: number, y: number) => {
    const group = pick(wrap, x)
    if (!group) return setTip(null)
    setTip({ group, x, y })
  }

  const n = tip ? tip.group.reduce((sum, g) => sum + counts[GRADES.indexOf(g)]!, 0) : 0
  return (
    <span
      className="hm-barcard"
      onMouseMove={e => show(e.currentTarget, e.clientX, e.clientY)}
      onMouseLeave={e => {
        setTip(null)
        unpick(e.currentTarget)
      }}
      onFocus={e => {
        const r = (e.target as HTMLElement).getBoundingClientRect()
        show(e.currentTarget, r.left + r.width / 2, r.bottom - 10)
      }}
      onBlur={e => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          setTip(null)
          unpick(e.currentTarget)
        }
      }}
    >
      {children}
      {tip && (
        <span className="hm-tip" aria-hidden="true" style={tipAt(tip.x, tip.y)}>
          {groupLabel(tip.group)}
          <small>
            {n.toLocaleString('en-US')} of {total.toLocaleString('en-US')} cases (
            {fmt.pct(n / total)}): {MEANING[tip.group[0]!]}. Click to list them.
          </small>
        </span>
      )}
    </span>
  )
}
