import { useEffect, useRef } from 'react'
import { Checkbox } from '@/ds'
import { COMPARE_MAX, COMPARE_MIN } from './model'

// The kernels the grid shows, as a dropdown of checkboxes: one per kernel, in rank order. It keeps
// between COMPARE_MIN and COMPARE_MAX ticked, the limits the leaderboard's compare ticks keep, so a
// box that would cross a limit is disabled. COMPARE_MIN is 0, so every kernel may be unticked. A
// click outside it or Escape closes it.
export function KernelPicker({
  kernels,
  shown,
  onToggle,
}: {
  kernels: string[]
  shown: string[]
  onToggle: (kernel: string) => void
}) {
  const root = useRef<HTMLDetailsElement>(null)
  useEffect(() => {
    const outside = (e: PointerEvent) => {
      const d = root.current
      if (d?.open && !d.contains(e.target as Node)) d.open = false
    }
    const escape = (e: KeyboardEvent) => {
      const d = root.current
      if (e.key !== 'Escape' || !d?.open) return
      d.open = false
      d.querySelector('summary')?.focus()
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape)
    }
  }, [])

  const atMost = Math.min(COMPARE_MAX, kernels.length)
  const atLeast = Math.min(COMPARE_MIN, kernels.length)
  return (
    <details ref={root} className="hm-kpick">
      <summary
        className="hm-kpick__trigger bc-annot sc-focus"
        aria-label="Choose the kernels shown"
      >
        <span>kernels</span>
        <span className="hm-kpick__count">
          {shown.length}/{kernels.length}
        </span>
        <span className="hm-kpick__caret" aria-hidden="true">
          ▾
        </span>
      </summary>
      <div className="hm-kpick__panel" role="group" aria-label="Kernels shown">
        {kernels.map(k => {
          const on = shown.includes(k)
          return (
            <Checkbox
              key={k}
              tone="void"
              label={k}
              checked={on}
              disabled={on ? shown.length <= atLeast : shown.length >= atMost}
              onChange={() => onToggle(k)}
            />
          )
        })}
        <p className="bc-annot hm-kpick__note">
          {atLeast === 0 ? `up to ${atMost} at once` : `${atLeast} to ${atMost} at once`}
        </p>
      </div>
    </details>
  )
}
