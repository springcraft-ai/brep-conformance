import { useEffect, useRef } from 'react'

// A page's search box narrows what the page shows as the reader types: the results page its tests,
// operations its operations, the docs their sections and a report its cases.
export const SEARCH_ID = 'bc-search'

// Takes the box over on a page that searches in place without React: `apply` runs on each change,
// and Enter runs `onEnter` instead of leaving the page. A ?q= the page was opened with is applied.
export function takeSearch(apply: (q: string) => void, onEnter?: () => void) {
  const input = document.getElementById(SEARCH_ID) as HTMLInputElement | null
  if (!input) return
  input.addEventListener('input', () => apply(input.value))
  input.form?.addEventListener('submit', e => {
    e.preventDefault()
    onEnter?.()
  })
  const q = queryFromUrl()
  if (q) input.value = q
  apply(input.value)
}

export function useSearchBox(q: string, onChange: (q: string) => void) {
  const latest = useRef(onChange)
  latest.current = onChange

  useEffect(() => {
    const input = document.getElementById(SEARCH_ID) as HTMLInputElement | null
    if (!input) return
    const onInput = () => latest.current(input.value)
    const onSubmit = (e: Event) => e.preventDefault()
    input.addEventListener('input', onInput)
    input.form?.addEventListener('submit', onSubmit)
    return () => {
      input.removeEventListener('input', onInput)
      input.form?.removeEventListener('submit', onSubmit)
    }
  }, [])

  useEffect(() => {
    const input = document.getElementById(SEARCH_ID) as HTMLInputElement | null
    if (input && input.value !== q) input.value = q
  }, [q])
}

export function queryFromUrl(): string {
  return new URLSearchParams(window.location.search).get('q') ?? ''
}
