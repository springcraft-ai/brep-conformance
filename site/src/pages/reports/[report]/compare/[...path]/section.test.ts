import { describe, expect, it } from 'vitest'
import { HOME, sectionPlane } from './scan'
import { sectionOf } from './section'
import type { Vec3 } from './vec'

const FRAME = { lo: [0, 0, 0] as Vec3, hi: [10, 8, 6] as Vec3 }
const kept = (plane: number[], p: Vec3) =>
  plane[0]! * p[0] + plane[1]! * p[1] + plane[2]! * p[2] >= plane[3]!

describe('sectionPlane', () => {
  it('cuts through the frame’s centre', () => {
    for (const axis of [0, 1, 2] as const) {
      const centre: Vec3 = [5, 4, 3]
      expect(kept(sectionPlane(axis, FRAME), centre)).toBe(true)
      const off = [...centre] as Vec3
      off[axis] += 1e-6
      const back = [...centre] as Vec3
      back[axis] -= 1e-6
      expect(kept(sectionPlane(axis, FRAME), off)).not.toBe(kept(sectionPlane(axis, FRAME), back))
    }
  })

  it('keeps the half away from where the view opens', () => {
    const cp = Math.cos(HOME.pitch)
    const toward: Vec3 = [cp * Math.cos(HOME.yaw), cp * Math.sin(HOME.yaw), Math.sin(HOME.pitch)]
    for (const axis of [0, 1, 2] as const) {
      const near: Vec3 = [5, 4, 3]
      near[axis] += Math.sign(toward[axis]) * 2
      expect(kept(sectionPlane(axis, FRAME), near)).toBe(false)
    }
  })
})

describe('sectionOf', () => {
  it('cuts a shell and leaves a test with nothing hidden whole', () => {
    expect(sectionOf('modify/shell')).toBe(1)
    expect(sectionOf('modify/shell-l-prism-open-at-its-step')).toBe(2)
    expect(sectionOf('booleans/overlap/cone-and-box')).toBeNull()
  })
})
