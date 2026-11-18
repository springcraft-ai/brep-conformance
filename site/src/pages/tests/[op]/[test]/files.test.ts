import { describe, expect, it } from 'vitest'
import { ask, fileName, probe, stateOf } from './files'

describe('fileName', () => {
  it('is the last part of the file’s address', () => {
    expect(fileName('https://cdn.example/b/1.0.0/k/1/t/x/box-box-1f.abc.main.step')).toBe(
      'box-box-1f.abc.main.step',
    )
  })
})

describe('probe', () => {
  const answering = (ok: boolean) => (async () => ({ ok })) as unknown as typeof fetch

  it('finds a file the CDN answers for, and misses one it refuses', async () => {
    expect(await probe('https://cdn.example/a.glb', answering(true))).toBe('ok')
    expect(await probe('https://cdn.example/a.glb', answering(false))).toBe('missing')
  })

  it('asks with HEAD, so no file is fetched to learn it is there', async () => {
    let asked: RequestInit | undefined
    const get = (async (_url: string, init?: RequestInit) => {
      asked = init
      return { ok: true }
    }) as unknown as typeof fetch
    await probe('https://cdn.example/a.glb', get)
    expect(asked?.method).toBe('HEAD')
  })

  it('says nothing of a file when the request could not be made', async () => {
    const failing = (async () => {
      throw new TypeError('network')
    }) as unknown as typeof fetch
    expect(await probe('https://cdn.example/a.glb', failing)).toBe('unknown')
  })
})

describe('ask', () => {
  const counting = (ok: boolean) => {
    const made = { requests: 0 }
    const get = (async () => {
      made.requests++
      return { ok }
    }) as unknown as typeof fetch
    return { made, get }
  }

  it('asks for a file once, and answers from memory after', async () => {
    const { made, get } = counting(false)
    const url = 'https://cdn.example/ask/once.glb'
    expect(stateOf(url)).toBe('unknown')
    expect(await Promise.all([ask(url, get), ask(url, get)])).toEqual(['missing', 'missing'])
    expect(await ask(url, get)).toBe('missing')
    expect(made.requests).toBe(1)
    expect(stateOf(url)).toBe('missing')
  })

  it('asks again for a file whose request could not be made', async () => {
    const url = 'https://cdn.example/ask/again.glb'
    const failing = (async () => {
      throw new TypeError('network')
    }) as unknown as typeof fetch
    expect(await ask(url, failing)).toBe('unknown')
    const { made, get } = counting(true)
    expect(await ask(url, get)).toBe('ok')
    expect(made.requests).toBe(1)
  })
})
