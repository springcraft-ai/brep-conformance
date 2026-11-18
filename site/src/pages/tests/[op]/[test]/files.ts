// What is known of a published file: nothing until the CDN has answered for it.
export type FileState = 'unknown' | 'ok' | 'missing'

// The name a saved file takes: the last part of its address.
export function fileName(url: string): string {
  return url.slice(url.lastIndexOf('/') + 1)
}

// Asks whether a published file is there. A request that could not be made says nothing either
// way, so its link stays as it was.
export async function probe(
  url: string,
  get: typeof fetch = fetch,
  signal?: AbortSignal,
): Promise<FileState> {
  try {
    return (await get(url, { method: 'HEAD', signal })).ok ? 'ok' : 'missing'
  } catch {
    return 'unknown'
  }
}

const known = new Map<string, FileState>()
const asking = new Map<string, Promise<FileState>>()

// What the page has learnt of a file so far.
export function stateOf(url: string): FileState {
  return known.get(url) ?? 'unknown'
}

// Asks for a file once and remembers the answer, so a case seen before is not asked for again. A
// request that could not be made is not remembered, and is made again when next asked for.
export function ask(url: string, get: typeof fetch = fetch): Promise<FileState> {
  const answered = known.get(url)
  if (answered) return Promise.resolve(answered)
  let pending = asking.get(url)
  if (!pending) {
    pending = probe(url, get).then(state => {
      asking.delete(url)
      if (state !== 'unknown') known.set(url, state)
      return state
    })
    asking.set(url, pending)
  }
  return pending
}
