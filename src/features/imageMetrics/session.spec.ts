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
    computeSsim: vi.fn(async () => ({ score: 0.9, durationMs: 3 })),
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

function ssimStatus(session: ReturnType<typeof createImageMetricsSession>, path: string) {
  return session.items.value.find((item) => item.path === path)?.ssim.status
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

  it('drops queued perceptual hash work after reset', async () => {
    const pending = Array.from({ length: 5 }, () => deferred<{ phash: string }>())
    const computePhash = vi.fn(() => pending[computePhash.mock.calls.length - 1].promise)
    const testDeps = deps({ computePhash })
    const session = createImageMetricsSession(testDeps)

    await session.addPaths(['a', 'b', 'c', 'd', 'e'])
    await vi.waitFor(() => expect(computePhash).toHaveBeenCalledTimes(4))
    session.reset()
    pending[0].resolve({ phash: '0000000000000000' })

    await new Promise((resolve) => setTimeout(resolve, 0))
    const callCountAfterReset = computePhash.mock.calls.length
    for (const task of pending.slice(1)) task.resolve({ phash: '0000000000000000' })
    await vi.waitFor(() => expect(session.hasRunningTasks.value).toBe(false))
    expect(callCountAfterReset).toBe(4)
  })

  it('automatically computes ssim for every ready non-baseline image', async () => {
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

    await vi.waitFor(() => expect(testDeps.computeSsim).toHaveBeenCalledTimes(2))
    await vi.waitFor(() => expect(ssimStatus(session, 'a')).toBe('done'))
    expect(session.items.value.find((item) => item.path === 'a')?.phashDistance).toBe(1)
  })

  it('starts ssim later when the baseline is selected before hashes finish', async () => {
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
    expect(testDeps.computeSsim).not.toHaveBeenCalled()

    phashes.get('base')?.resolve({ phash: '0000000000000000' })
    await vi.waitFor(() =>
      expect(session.items.value.find((item) => item.path === 'base')?.phashState).toBe('ready')
    )
    expect(testDeps.computeSsim).not.toHaveBeenCalled()

    phashes.get('candidate')?.resolve({ phash: '0000000000000003' })
    await vi.waitFor(() => expect(testDeps.computeSsim).toHaveBeenCalledTimes(1))
    expect(session.items.value.find((item) => item.path === 'candidate')?.phashDistance).toBe(2)
  })

  it('shows perceptual hash distance even when ssim fails', async () => {
    const testDeps = deps({
      computePhash: vi.fn(async (item) => ({
        phash: item.path === 'base' ? '0000000000000000' : '0000000000000003'
      })),
      computeSsim: vi.fn(async () => {
        throw new Error('SSIM 失败')
      })
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )

    await session.setBaseline('base')

    await vi.waitFor(() => expect(ssimStatus(session, 'candidate')).toBe('error'))
    expect(session.items.value.find((item) => item.path === 'candidate')?.phashDistance).toBe(2)
  })

  it('retries only the requested failed ssim comparison', async () => {
    const testDeps = deps({
      computePhash: vi.fn(async (item) => ({
        phash: item.path === 'base' ? '0000000000000000' : '0000000000000001'
      })),
      computeSsim: vi.fn()
        .mockRejectedValueOnce(new Error('临时失败'))
        .mockResolvedValueOnce({ score: 0.9, durationMs: 3 })
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    await session.setBaseline('base')
    await vi.waitFor(() => expect(ssimStatus(session, 'candidate')).toBe('error'))

    await session.retrySsim('candidate')

    await vi.waitFor(() => expect(ssimStatus(session, 'candidate')).toBe('done'))
    expect(testDeps.computeSsim).toHaveBeenCalledTimes(2)
  })

  it('keeps pending ssim work in hasContent after reset', async () => {
    const pendingSsim = deferred<{ score: number; durationMs: number }>()
    const testDeps = deps({
      computeSsim: vi.fn(() => pendingSsim.promise)
    })
    const session = createImageMetricsSession(testDeps)
    await session.addPaths(['base', 'candidate'])
    await vi.waitFor(() =>
      expect(session.items.value.every((item) => item.phashState === 'ready')).toBe(true)
    )
    const run = session.setBaseline('base')
    await vi.waitFor(() => expect(testDeps.computeSsim).toHaveBeenCalledTimes(1))

    session.reset()

    expect(session.hasContent.value).toBe(true)
    expect(session.hasRunningTasks.value).toBe(true)
    pendingSsim.resolve({ score: 1, durationMs: 1 })
    await run
    await vi.waitFor(() => expect(session.hasContent.value).toBe(false))
    expect(session.hasRunningTasks.value).toBe(false)
  })
})
