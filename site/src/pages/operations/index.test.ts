import { describe, expect, it } from 'vitest'
import { site } from '@/data/source'
import { propsOf } from './index.data'

describe('the functions page', () => {
  const p = propsOf()
  const fns = p.groups.flatMap(g => g.functions)

  it('lists every function once, under its group', () => {
    expect(fns.length).toBe(p.total)
    expect(new Set(fns.map(f => f.id)).size).toBe(p.total)
  })

  it('counts a kernel’s offered functions as its column shows them', () => {
    p.kernels.forEach((k, i) => expect(fns.filter(f => f.cells[i] === true).length).toBe(k.offered))
  })

  it('marks a function a report lists as unsupported as not offered, or as its stand-in', () => {
    const s = site()
    s.latest.forEach((r, i) => {
      for (const f of fns) {
        const cell = f.cells[i]
        if (r.unsupported.includes(f.id)) expect(cell).toBe(r.standIns[f.id] ?? false)
        else expect(cell).toBe(true)
      }
    })
  })

  it('names the test areas that exercise a group', () => {
    expect(p.groups.find(g => g.group === 'Booleans')?.areas).toEqual(['booleans'])
    expect(p.groups.find(g => g.group === 'Blends')?.areas.sort()).toEqual(['chamfers', 'fillets'])
  })
})
