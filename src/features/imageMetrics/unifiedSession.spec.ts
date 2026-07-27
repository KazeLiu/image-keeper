import { describe, expect, it, vi } from 'vitest'
import { createImageMetricsSession, type ImageMetricsDependencies } from './session'
import type { TestImageInfo, TestSsimResult } from '@/api/imageMetrics'

const image = (path: string): TestImageInfo => ({
  path,
  fileName: `${path}.png`,
  fileSize: 100,
  width: 100,
  height: 100,
  modifiedAtMs: 1,
  thumbnailDataUrl: `data:image/png;base64,${path}`
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => { resolve = resolvePromise })
  return { promise, resolve }
}

describe('unified image metrics session', () => {
  it('uses one automatic standard ssim channel', async () => {
    const computeSsim = vi.fn(async () => ({ score: 0.987654, durationMs: 5 }))
    const dependencies = {
      loadImage: vi.fn(async (path: string) => image(path)),
      computePhash: vi.fn(async (item: TestImageInfo) => ({
        phash: item.path === 'base' ? '0000000000000000' : '0000000000000001'
      })),
      computeSsim
    } as ImageMetricsDependencies
    const session = createImageMetricsSession(dependencies)

    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    await session.setBaseline('base')

    await vi.waitFor(() => expect(computeSsim).toHaveBeenCalledTimes(1))
    await vi.waitFor(() =>
      expect(session.items.value.find((item) => item.path === 'candidate')?.ssim.status).toBe('done')
    )
  })

  it('limits standard ssim to four concurrent calculations', async () => {
    const pending = Array.from({ length: 5 }, () => deferred<TestSsimResult>())
    let active = 0
    let maxActive = 0
    const computeSsim = vi.fn(() => {
      const current = pending[computeSsim.mock.calls.length - 1]
      active += 1
      maxActive = Math.max(maxActive, active)
      return current.promise.finally(() => { active -= 1 })
    })
    const dependencies = {
      loadImage: vi.fn(async (path: string) => image(path)),
      computePhash: vi.fn(async (item: TestImageInfo) => ({
        phash: item.path === 'base'
          ? '0000000000000000'
          : `000000000000000${item.path}`
      })),
      computeSsim
    } as ImageMetricsDependencies
    const session = createImageMetricsSession(dependencies)

    await session.addPaths(['base', '1', '2', '3', '4', '5'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    void session.setBaseline('base')

    await vi.waitFor(() => expect(computeSsim).toHaveBeenCalledTimes(4))
    expect(maxActive).toBe(4)
    pending[0].resolve({ score: 0.9, durationMs: 1 })
    await vi.waitFor(() => expect(computeSsim).toHaveBeenCalledTimes(5))
    for (const item of pending.slice(1)) item.resolve({ score: 0.9, durationMs: 1 })
  })

  it('does not enqueue the same queued pair twice when reselecting the baseline', async () => {
    const calls: Array<ReturnType<typeof deferred<TestSsimResult>>> = []
    const computeSsim = vi.fn(() => {
      const task = deferred<TestSsimResult>()
      calls.push(task)
      return task.promise
    })
    const dependencies = {
      loadImage: vi.fn(async (path: string) => image(path)),
      computePhash: vi.fn(async (item: TestImageInfo) => ({
        phash: item.path === 'base'
          ? '0000000000000000'
          : `000000000000000${item.path}`
      })),
      computeSsim
    } as ImageMetricsDependencies
    const session = createImageMetricsSession(dependencies)

    await session.addPaths(['base', '1', '2', '3', '4', '5'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    void session.setBaseline('base')
    await vi.waitFor(() => expect(computeSsim).toHaveBeenCalledTimes(4))

    void session.setBaseline('base')
    for (const task of calls) task.resolve({ score: 0.9, durationMs: 1 })
    await vi.waitFor(() => expect(computeSsim).toHaveBeenCalledTimes(5))
    calls[4].resolve({ score: 0.9, durationMs: 1 })
    await new Promise((resolve) => window.setTimeout(resolve, 0))

    expect(computeSsim).toHaveBeenCalledTimes(5)
  })
})
