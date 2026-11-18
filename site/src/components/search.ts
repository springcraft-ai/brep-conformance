import { useEffect, useRef } from 'react'

// The chrome's search box: one box, and each page searches what it shows. The results page filters
// its tests, operations its operations and the docs their sections, as the reader types; any other
// page, and any page without JS, submits it to the results page as ?q=.
export const SEARCH_ID = 'bc-search'

const PLACEHOLDER: Record<string, string> = {
  '/operations/': 'search operations, e.g. fillet',
  '/docs/': 'search the docs, e.g. tolerance',
}

export function searchPlaceholder(path: string): string {
  return PLACEHOLDER[path] ?? 'search tests, e.g. fillets or cylinders'
}

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

export function useChromeSearch(q: string, onChange: (q: string) => void) {
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
