import { describe, expect, it } from 'vitest'
import { site } from '@/data/source'
import { shortCaseIds, stepLabel } from './format'
import { propsOf } from './index.data'

describe('propsOf', () => {
  const s = site()
  const pages = s.tests.map(t => propsOf(t, s.latest))

  it('gives each kernel its own files for a case, under its own report’s path', () => {
    let seen = 0
    for (const p of pages)
      for (const c of p.caseList)
        p.kernels.forEach((k, i) => {
          const f = c.files[i]
          if (!f) return
          seen++
          const at = `/${k.suite}/${k.kernel}/${k.version}/${p.test}/${c.id}\\.[0-9a-f]+`
          if (f.glb) expect(f.glb).toMatch(new RegExp(`${at}\\.glb$`))
          for (const step of f.steps)
            expect(step).toMatch(new RegExp(`${at}(\\.[a-z0-9-]+)?\\.step$`))
        })
    expect(seen).toBeGreaterThan(0)
  })

  it('keeps the STEP files of a kernel that drew nothing', () => {
    const stepOnly = pages
      .flatMap(p => p.caseList.flatMap(c => c.files))
      .filter(f => f && !f.glb && f.steps.length > 0)
    expect(stepOnly.length).toBeGreaterThan(0)
  })

  it('gives a case of two solids a STEP file for each', () => {
    const two = pages
      .flatMap(p => p.caseList.flatMap(c => c.files))
      .filter(f => f?.steps.length === 2)
    expect(two.length).toBeGreaterThan(0)
    expect(two[0]!.steps.map(stepLabel)).toEqual(['step · main', 'step · alt'])
  })

  it('gives a kernel no files for a case it does not support', () => {
    const none = pages.flatMap(p =>
      p.caseList.flatMap(c => c.files.filter((_, i) => c.grades[i] === 'unsupported')),
    )
    expect(none.length).toBeGreaterThan(0)
    expect(none.every(f => f === null)).toBe(true)
  })

  it('gives a kernel no worst case exactly when every case passed at 1e-7', () => {
    for (const p of pages)
      for (const k of p.kernels) expect(k.worst === null).toBe(k.grade === '1e-7')
  })

  it('lists every kernel with a report, in report order', () => {
    for (const p of pages) expect(p.kernels.map(k => k.kernel)).toEqual(s.kernels)
  })

  it('lists every case once, with a grade per kernel that agrees with the kernel’s count', () => {
    for (const p of pages) {
      expect(p.caseList.length).toBe(p.cases)
      expect(new Set(p.caseList.map(c => c.id)).size).toBe(p.cases)
      p.kernels.forEach((k, i) => {
        expect(p.caseList.filter(c => c.grades[i] === '1e-7').length).toBe(k.passing)
        if (k.worst) expect(p.caseList.find(c => c.id === k.worst!.id)?.grades[i]).toBe(k.grade)
      })
    }
  })

  it('tells a test’s cases apart by their short ids', () => {
    for (const p of pages) {
      const short = shortCaseIds(p.caseList.map(c => c.id))
      expect(new Set(short).size).toBe(short.length)
    }
  })

  it('opens every test’s path with its operation, as the page’s header link assumes', () => {
    for (const p of pages) expect(p.test.startsWith(`${p.operation}/`)).toBe(true)
  })

  it('quotes its area’s section of api.md on every page but a part’s', () => {
    for (const p of pages.filter(p => p.operation !== 'parts'))
      expect(p.definition?.signatures.length).toBeGreaterThan(0)
  })
})
