import { useSyncExternalStore } from 'react'

// The site has no client router, so this is the one place a page's ?q= is read and written.
const CHANGED = 'bc:query'

function subscribe(onChange: () => void): () => void {
  window.addEventListener('popstate', onChange)
  window.addEventListener(CHANGED, onChange)
  return () => {
    window.removeEventListener('popstate', onChange)
    window.removeEventListener(CHANGED, onChange)
  }
}

const read = () => new URLSearchParams(window.location.search).get('q') ?? ''

// The page's ?q= as state: it follows the address bar, and setting it rewrites the address in
// place, so a reload or a shared link opens on the same search.
export function useUrlQuery(): [string, (q: string) => void] {
  const q = useSyncExternalStore(subscribe, read, () => '')
  const set = (next: string) => {
    const url = new URL(window.location.href)
    if (next) url.searchParams.set('q', next)
    else url.searchParams.delete('q')
    window.history.replaceState(window.history.state, '', url)
    window.dispatchEvent(new Event(CHANGED))
  }
  return [q, set]
}
