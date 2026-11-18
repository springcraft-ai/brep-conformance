import { describe, expect, it } from 'vitest'
import { fileName } from './files'

describe('fileName', () => {
  it('is the last part of the file’s address', () => {
    expect(fileName('https://cdn.example/b/1.0.0/k/1/t/x/box-box-1f.abc.main.step')).toBe(
      'box-box-1f.abc.main.step',
    )
  })
})
