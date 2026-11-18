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
