import { describe, expect, it, vi } from 'vitest'
import { createImageMetricsSession, type ImageMetricsDependencies } from './session'
import type { TestImageInfo } from '@/api/imageMetrics'

const image = (path: string): TestImageInfo => ({
  path,
  fileName: `${path}.png`,
  fileSize: 100,
  width: 100,
  height: 100,
  modifiedAtMs: 1,
  thumbnailDataUrl: `data:image/png;base64,${path}`
})

function deps(overrides: Partial<ImageMetricsDependencies> = {}): ImageMetricsDependencies {
  return {
    loadImage: vi.fn(async (path) => image(path)),
    computePhash: vi.fn(async (item) => ({ phash: item.path })),
    computeLow: vi.fn(async () => ({ similarity: 0.9, durationMs: 3 })),
    computeHigh: vi.fn(async () => ({ score: 0.98, durationMs: 20 })),
    ...overrides
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

function lowStatus(session: ReturnType<typeof createImageMetricsSession>, path: string) {
  return session.items.value.find((item) => item.path === path)?.low.status
}

describe('image metrics session', () => {
  it('deduplicates imported canonical paths', async () => {
    const testDeps = deps({
      loadImage: vi.fn(async (path) => image(path.toLowerCase()))
    })
    const session = createImageMetricsSession(testDeps)

    await session.addPaths(['A', 'a'])

    expect(session.items.value).toHaveLength(1)
    expect(session.duplicateCount.value).toBe(1)
    session.clearDuplicateCount()
    expect(session.duplicateCount.value).toBe(0)
  })

  it('continues importing after one image fails', async () => {
    const testDeps = deps({
      loadImage: vi.fn(async (path) => {
        if (path === 'bad') throw new Error('无法解码')
        return image(path)
      })
    })
    const session = createImageMetricsSession(testDeps)

    await session.addPaths(['bad', 'good'])

    expect(session.items.value.map((item) => item.path)).toEqual(['good'])
    expect(session.importErrors.value[0]).toContain('bad')
  })

  it('loads up to four images at a time and keeps import order', async () => {
    const pending = new Map<string, (value: TestImageInfo) => void>()
    let activeImports = 0
    let maxActiveImports = 0
    const testDeps = deps({
      loadImage: vi.fn(async (path) => {
        activeImports += 1
        maxActiveImports = Math.max(maxActiveImports, activeImports)
        return await new Promise<TestImageInfo>((resolve) => {
          pending.set(path, (value) => {
            activeImports -= 1
            resolve(value)
          })
        })
      })
    })
    const session = createImageMetricsSession(testDeps)

    const importing = session.addPaths(['a', 'b', 'c', 'd', 'e'])

    expect(session.loadingCount.value).toBe(5)
    await vi.waitFor(() => expect(testDeps.loadImage).toHaveBeenCalledTimes(4))
    expect(maxActiveImports).toBe(4)

    pending.get('c')?.(image('c'))
    await vi.waitFor(() => expect(testDeps.loadImage).toHaveBeenCalledTimes(5))
    pending.get('e')?.(image('e'))
    pending.get('d')?.(image('d'))
    pending.get('b')?.(image('b'))
    pending.get('a')?.(image('a'))

    await importing

    expect(session.items.value.map((item) => item.path)).toEqual(['a', 'b', 'c', 'd', 'e'])
  })

  it('shows image cards immediately before decoding finishes', async () => {
    const pending = new Map<string, (value: TestImageInfo) => void>()
    const testDeps = deps({
      loadImage: vi.fn(async (path) => (
        await new Promise<TestImageInfo>((resolve) => {
          pending.set(path, resolve)
        })
      ))
    })
    const session = createImageMetricsSession(testDeps)

    const importing = session.addPaths(['a', 'b', 'c', 'd'])
    await vi.waitFor(() => expect(testDeps.loadImage).toHaveBeenCalledTimes(4))

    expect(session.items.value.map((item) => item.path)).toEqual(['a', 'b', 'c', 'd'])
    expect(session.items.value.every((item) => item.loadState === 'loading')).toBe(true)

    pending.get('a')?.(image('a'))
    pending.get('b')?.(image('b'))
    pending.get('c')?.(image('c'))
    pending.get('d')?.(image('d'))
    await importing
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.loadState === 'ready')).toBe(true)
    )
  })

  it('computes perceptual hashes after images are decoded', async () => {
    const testDeps = deps({
      computePhash: vi.fn(async (item) => ({
        phash: item.path === 'candidate' ? '0000000000000001' : '0000000000000000'
      }))
    })
    const session = createImageMetricsSession(testDeps)

    await session.addPaths(['base', 'candidate'])

    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    expect(testDeps.computePhash).toHaveBeenCalledTimes(2)
  })

  it('automatically computes low precision for every ready non-baseline image', async () => {
    const testDeps = deps({
      computePhash: vi.fn(async (item) => ({
        phash: item.path === 'base' ? '0000000000000000' : '0000000000000001'
      }))
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'a', 'b'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )

    await session.setBaseline('base')

    await vi.waitFor(() => expect(testDeps.computeLow).toHaveBeenCalledTimes(2))
    await vi.waitFor(() => expect(lowStatus(session, 'a')).toBe('done'))
    expect(session.items.value.find((item) => item.path === 'a')?.phashDistance).toBe(1)
  })

  it('starts low precision later when the baseline is selected before hashes finish', async () => {
    const phashes = new Map<string, ReturnType<typeof deferred<{ phash: string }>>>()
    const testDeps = deps({
      computePhash: vi.fn((item) => {
        const pending = deferred<{ phash: string }>()
        phashes.set(item.path, pending)
        return pending.promise
      })
    })
    const session = createImageMetricsSession(testDeps)

    await session.addPaths(['base', 'candidate'])
    await session.setBaseline('base')
    expect(testDeps.computeLow).not.toHaveBeenCalled()

    phashes.get('base')?.resolve({ phash: '0000000000000000' })
    await vi.waitFor(() =>
      expect(session.items.value.find((item) => item.path === 'base')?.phashState).toBe('ready')
    )
    expect(testDeps.computeLow).not.toHaveBeenCalled()

    phashes.get('candidate')?.resolve({ phash: '0000000000000003' })
    await vi.waitFor(() => expect(testDeps.computeLow).toHaveBeenCalledTimes(1))
    expect(session.items.value.find((item) => item.path === 'candidate')?.phashDistance).toBe(2)
  })

  it('shows perceptual hash distance even when low precision fails', async () => {
    const testDeps = deps({
      computePhash: vi.fn(async (item) => ({
        phash: item.path === 'base' ? '0000000000000000' : '0000000000000003'
      })),
      computeLow: vi.fn(async () => {
        throw new Error('低精度失败')
      })
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )

    await session.setBaseline('base')

    await vi.waitFor(() => expect(lowStatus(session, 'candidate')).toBe('error'))
    expect(session.items.value.find((item) => item.path === 'candidate')?.phashDistance).toBe(2)
  })

  it('retries only the requested failed low precision comparison', async () => {
    const testDeps = deps({
      computePhash: vi.fn(async (item) => ({
        phash: item.path === 'base' ? '0000000000000000' : '0000000000000001'
      })),
      computeLow: vi.fn()
        .mockRejectedValueOnce(new Error('临时失败'))
        .mockResolvedValueOnce({ similarity: 0.9, durationMs: 3 })
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    await session.setBaseline('base')
    await vi.waitFor(() => expect(lowStatus(session, 'candidate')).toBe('error'))

    await session.retryLowPrecision('candidate')

    await vi.waitFor(() => expect(lowStatus(session, 'candidate')).toBe('done'))
    expect(testDeps.computeLow).toHaveBeenCalledTimes(2)
  })

  it('does not start standard similarity until requested and reuses a cached unordered pair', async () => {
    const testDeps = deps()
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['a', 'b'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    await session.setBaseline('a')

    expect(testDeps.computeHigh).not.toHaveBeenCalled()
    await session.computeHighPrecision('b')
    await vi.waitFor(() => expect(lowStatus(session, 'b')).toBe('done'))
    await session.setBaseline('b')
    await session.computeHighPrecision('a')

    expect(testDeps.computeHigh).toHaveBeenCalledTimes(1)
  })

  it('limits standard similarity to two running tasks', async () => {
    const first = deferred<{ score: number; durationMs: number }>()
    const second = deferred<{ score: number; durationMs: number }>()
    const testDeps = deps({
      computeHigh: vi.fn()
        .mockReturnValueOnce(first.promise)
        .mockReturnValueOnce(second.promise)
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'a', 'b', 'c'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    await session.setBaseline('base')

    const firstRun = session.computeHighPrecision('a')
    const secondRun = session.computeHighPrecision('b')
    const thirdStarted = await session.computeHighPrecision('c')

    expect(thirdStarted).toBe(false)
    expect(testDeps.computeHigh).toHaveBeenCalledTimes(2)
    first.resolve({ score: 1, durationMs: 1 })
    second.resolve({ score: 0.99, durationMs: 1 })
    await firstRun
    await secondRun
  })

  it('keeps pending low precision work in hasContent after reset', async () => {
    const pendingLow = deferred<{ similarity: number; durationMs: number }>()
    const testDeps = deps({
      computeLow: vi.fn(() => pendingLow.promise)
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    const run = session.setBaseline('base')
    await vi.waitFor(() => expect(testDeps.computeLow).toHaveBeenCalledTimes(1))

    session.reset()

    expect(session.hasContent.value).toBe(true)
    expect(session.hasRunningTasks.value).toBe(true)
    pendingLow.resolve({ similarity: 1, durationMs: 1 })
    await run
    await vi.waitFor(() => expect(session.hasContent.value).toBe(false))
    expect(session.hasRunningTasks.value).toBe(false)
  })
})
