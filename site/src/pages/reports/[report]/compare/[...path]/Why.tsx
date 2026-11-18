import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { fmt } from '@/components'

export const FAILURE_GLOSS = {
  geometry: 'the shape is wrong',
  measurement: 'a measured value is off',
} as const

// A kernel's reason, a repeated message said once, cut to `lines` lines. Where the cut hides some of
// it, a button shows the rest; one line sits beside its button, more stand over it.
export function Why({ text, lines = 3 }: { text: string; lines?: number }) {
  const [all, setAll] = useState(false)
  const [cut, setCut] = useState(false)
  const box = useRef<HTMLSpanElement>(null)
  const short = fmt.condense(text)
  useEffect(() => {
    const el = box.current
    if (!el || all) return
    const check = () =>
      setCut(el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1)
    check()
    const watch = new ResizeObserver(check)
    watch.observe(el)
    return () => watch.disconnect()
  }, [short, all])
  return (
    <span
      className={`bc-why${lines === 1 ? ' bc-why--line' : ''}`}
      data-open={all || undefined}
      style={{ '--why-lines': lines } as CSSProperties}
    >
      <span className="bc-why__text" data-clamp={all ? undefined : ''} ref={box}>
        {short}
      </span>
      {(cut || all) && (
        <button type="button" className="bc-why__more sc-focus" onClick={() => setAll(!all)}>
          {all ? 'show less' : 'show more'}
        </button>
      )}
    </span>
  )
}
