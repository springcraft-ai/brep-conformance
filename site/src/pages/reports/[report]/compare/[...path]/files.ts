// What the CDN has said of a published file: nothing yet, or nothing that settles it.
export type FileState = 'unknown' | 'ok' | 'missing'

// How a file is drawn: a link, a link still being asked about, or a file the CDN does not hold.
export type FileDisplay = 'link' | 'checking' | 'missing'

// The name a saved file takes: the last part of its address.
export function fileName(url: string): string {
  return url.slice(url.lastIndexOf('/') + 1)
}

// The CDN answers 403 for a key it does not hold, having no listing to tell 404 by.
const ABSENT = [403, 404, 410]

// What a response says of a file. Only an answer that the file is absent counts as missing: a
// rate limit or a server error says nothing of the file.
export function stateOfResponse(res: { ok: boolean; status: number }): FileState {
  return res.ok ? 'ok' : ABSENT.includes(res.status) ? 'missing' : 'unknown'
}

// Asks whether a published file is there. A request that could not be made says nothing either
// way, so its link stays as it was.
export async function probe(url: string, get: typeof fetch = fetch): Promise<FileState> {
  try {
    return stateOfResponse(await get(url, { method: 'HEAD' }))
  } catch {
    return 'unknown'
  }
}

const known = new Map<string, FileState>()
const asking = new Map<string, Promise<FileState>>()
// Asked for and not settled: not asked for again on this page, so a reader offline, or a CDN
// that will not answer, is not asked over and over.
const unsettled = new Set<string>()
const listeners = new Set<() => void>()
let version = 0

function changed(): void {
  version++
  for (const tell of listeners) tell()
}

export function subscribe(tell: () => void): () => void {
  listeners.add(tell)
  return () => listeners.delete(tell)
}

// Rises with every change to what is known, for a reader of the store to notice by.
export function storeVersion(): number {
  return version
}

// What the page has learnt of a file so far.
export function stateOf(url: string): FileState {
  return known.get(url) ?? 'unknown'
}

// Records what a later request showed of a file, such as a download that found it gone.
export function learn(url: string, state: FileState): void {
  if (state === 'unknown' || known.get(url) === state) return
  known.set(url, state)
  changed()
}

// Asks for a file once and remembers the answer, so a case seen before is not asked for again.
export function ask(url: string, get: typeof fetch = fetch): Promise<FileState> {
  const answered = known.get(url)
  if (answered) return Promise.resolve(answered)
  if (unsettled.has(url)) return Promise.resolve('unknown')
  let pending = asking.get(url)
  if (!pending) {
    pending = probe(url, get).then(state => {
      asking.delete(url)
      if (state === 'unknown') unsettled.add(url)
      else known.set(url, state)
      changed()
      return state
    })
    asking.set(url, pending)
  }
  return pending
}

// Only a file the CDN says it does not hold stops being a link: one not asked about, as in the
// page's own HTML, and one whose request failed, both stay links. `hydrated` is false for that
// HTML and the first render over it, which must agree.
export function displayOf(url: string, hydrated: boolean): FileDisplay {
  const state = known.get(url)
  if (state === 'missing') return 'missing'
  if (state === 'ok' || unsettled.has(url) || !hydrated) return 'link'
  return 'checking'
}
