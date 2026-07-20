import { describe, expect, it, vi } from 'vitest'
import type { TestDifferencePreviewResult, TestImageInfo } from '@/api/imageMetrics'
import { createDifferencePreview } from './differencePreview'

function image(path: string): TestImageInfo {
  return {
    path,
    fileName: `${path}.png`,
    fileSize: 100,
    width: 100,
    height: 100,
    modifiedAtMs: 1,
    thumbnailDataUrl: `data:image/png;base64,${path}`
  }
}

function result(regionCount: number): TestDifferencePreviewResult {
  return {
    baselineDataUrl: 'data:image/png;base64,base',
    candidateDataUrl: 'data:image/png;base64,candidate',
    highlightDataUrl: 'data:image/png;base64,highlight',
    width: 100,
    height: 100,
    changedPixelRatio: regionCount / 100,
    regionCount
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => { resolve = resolvePromise })
  return { promise, resolve }
}

describe('difference preview state', () => {
  it('opens a pair with the default sensitivity', async () => {
    const load = vi.fn(async () => result(1))
    const preview = createDifferencePreview(load)

    await preview.open(image('base'), image('candidate'))

    expect(load).toHaveBeenCalledWith(expect.objectContaining({ path: 'base' }), expect.objectContaining({ path: 'candidate' }), 50)
    expect(preview.visible.value).toBe(true)
    expect(preview.result.value?.regionCount).toBe(1)
  })

  it('refreshes the current pair after sensitivity changes', async () => {
    const load = vi.fn(async () => result(2))
    const preview = createDifferencePreview(load)
    await preview.open(image('base'), image('candidate'))

    preview.sensitivity.value = 80
    await preview.refresh()

    expect(load).toHaveBeenLastCalledWith(expect.anything(), expect.anything(), 80)
  })

  it('ignores an older request that finishes after the latest result', async () => {
    const first = deferred<TestDifferencePreviewResult>()
    const second = deferred<TestDifferencePreviewResult>()
    const load = vi.fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const preview = createDifferencePreview(load)

    const opening = preview.open(image('base'), image('candidate'))
    preview.sensitivity.value = 90
    const refreshing = preview.refresh()
    second.resolve(result(2))
    await refreshing
    first.resolve(result(1))
    await opening

    expect(preview.result.value?.regionCount).toBe(2)
  })
})
