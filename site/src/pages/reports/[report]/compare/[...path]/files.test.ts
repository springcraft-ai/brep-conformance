import { describe, expect, it } from 'vitest'
import { ask, displayOf, fileName, learn, probe, stateOf, storeVersion, subscribe } from './files'

const answering = (status: number) => {
  const made = { requests: 0, init: undefined as RequestInit | undefined }
  const get = (async (_url: string, init?: RequestInit) => {
    made.requests++
    made.init = init
    return { ok: status >= 200 && status < 300, status }
  }) as unknown as typeof fetch
  return { made, get }
}
const failing = (async () => {
  throw new TypeError('network')
}) as unknown as typeof fetch

describe('fileName', () => {
  it('is the last part of the file’s address', () => {
    expect(fileName('https://cdn.example/b/1.0.0/k/1/t/x/box-box-1f.abc.main.step')).toBe(
      'box-box-1f.abc.main.step',
    )
  })
})

describe('probe', () => {
  it('finds a file the CDN answers for', async () => {
    expect(await probe('https://cdn.example/a.glb', answering(200).get)).toBe('ok')
  })

  it('misses a file only where the CDN says it is absent', async () => {
    for (const status of [403, 404, 410])
      expect(await probe('https://cdn.example/a.glb', answering(status).get)).toBe('missing')
  })

  it('says nothing of a file from a rate limit, a server error or a failed request', async () => {
    for (const status of [429, 500, 503])
      expect(await probe('https://cdn.example/a.glb', answering(status).get)).toBe('unknown')
    expect(await probe('https://cdn.example/a.glb', failing)).toBe('unknown')
  })

  it('asks with HEAD, so no file is fetched to learn it is there', async () => {
    const { made, get } = answering(200)
    await probe('https://cdn.example/a.glb', get)
    expect(made.init?.method).toBe('HEAD')
  })
})

describe('ask', () => {
  it('asks for a file once, and answers from memory after', async () => {
    const { made, get } = answering(404)
    const url = 'https://cdn.example/ask/once.glb'
    expect(stateOf(url)).toBe('unknown')
    expect(await Promise.all([ask(url, get), ask(url, get)])).toEqual(['missing', 'missing'])
    expect(await ask(url, get)).toBe('missing')
    expect(made.requests).toBe(1)
    expect(stateOf(url)).toBe('missing')
  })

  it('does not ask again on the page for a file it could not settle', async () => {
    const url = 'https://cdn.example/ask/unsettled.glb'
    expect(await ask(url, failing)).toBe('unknown')
    const { made, get } = answering(200)
    expect(await ask(url, get)).toBe('unknown')
    expect(made.requests).toBe(0)
  })

  it('tells its readers when a file is answered, not when it starts being asked for', async () => {
    let told = 0
    const stop = subscribe(() => told++)
    const before = storeVersion()
    await ask('https://cdn.example/ask/told.glb', answering(200).get)
    stop()
    expect(told).toBe(1)
    expect(storeVersion()).toBe(before + 1)
  })
})

describe('displayOf', () => {
  it('draws a file never asked about as a link in the page’s HTML, and as checking after', () => {
    const url = 'https://cdn.example/display/new.glb'
    expect(displayOf(url, false)).toBe('link')
    expect(displayOf(url, true)).toBe('checking')
  })

  it('draws a file as checking while it is asked for, then by the answer', async () => {
    const found = 'https://cdn.example/display/found.glb'
    const pending = ask(found, answering(200).get)
    expect(displayOf(found, true)).toBe('checking')
    await pending
    expect(displayOf(found, true)).toBe('link')
    const gone = 'https://cdn.example/display/gone.glb'
    await ask(gone, answering(403).get)
    expect([displayOf(gone, true), displayOf(gone, false)]).toEqual(['missing', 'missing'])
  })

  it('keeps a file a link when asking about it failed', async () => {
    const url = 'https://cdn.example/display/failed.glb'
    await ask(url, failing)
    expect(displayOf(url, true)).toBe('link')
    const limited = 'https://cdn.example/display/limited.glb'
    await ask(limited, answering(429).get)
    expect(displayOf(limited, true)).toBe('link')
  })

  it('takes what a later request learnt of a file', () => {
    const url = 'https://cdn.example/display/learnt.glb'
    learn(url, 'missing')
    expect(displayOf(url, true)).toBe('missing')
  })
})
