import { describe, expect, it, vi } from 'vitest'
import { createImageMetricsSession, type ImageMetricsDependencies } from './session'

const image = (path: string) => ({
  path,
  fileName: `${path}.png`,
  fileSize: 100,
  width: 100,
  height: 100,
  modifiedAtMs: 1,
  phash: path,
  thumbnailDataUrl: `data:image/png;base64,${path}`
})

describe('image metrics session', () => {
  it('deduplicates imported canonical paths', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path.toLowerCase())),
      computeLow: vi.fn(),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)

    await session.addPaths(['A', 'a'])

    expect(session.items.value).toHaveLength(1)
    expect(session.duplicateCount.value).toBe(1)
    session.clearDuplicateCount()
    expect(session.duplicateCount.value).toBe(0)
  })

  it('continues importing after one image fails', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => {
        if (path === 'bad') throw new Error('无法解码')
        return image(path)
      }),
      computeLow: vi.fn(),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)

    await session.addPaths(['bad', 'good'])

    expect(session.items.value.map((item) => item.path)).toEqual(['good'])
    expect(session.importErrors.value[0]).toContain('bad')
  })

  it('loads up to three images at a time and keeps import order', async () => {
    const pending = new Map<string, (value: ReturnType<typeof image>) => void>()
    let activeImports = 0
    let maxActiveImports = 0
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => {
        activeImports += 1
        maxActiveImports = Math.max(maxActiveImports, activeImports)
        return await new Promise((resolve) => {
          pending.set(path, (value) => {
            activeImports -= 1
            resolve(value)
          })
        })
      }),
      computeLow: vi.fn(),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)

    const importing = session.addPaths(['a', 'b', 'c', 'd'])

    expect(session.loadingCount.value).toBe(4)
    await vi.waitFor(() => expect(deps.loadImage).toHaveBeenCalledTimes(3))
    expect(maxActiveImports).toBe(3)

    pending.get('c')?.(image('c'))
    await vi.waitFor(() => expect(deps.loadImage).toHaveBeenCalledTimes(4))
    pending.get('d')?.(image('d'))
    pending.get('b')?.(image('b'))
    pending.get('a')?.(image('a'))

    await importing

    expect(session.items.value.map((item) => item.path)).toEqual(['a', 'b', 'c', 'd'])
  })

  it('automatically computes low precision for every non-baseline image', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ similarity: 0.9, durationMs: 3 })),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'a', 'b'])

    await session.setBaseline('base')

    expect(deps.computeLow).toHaveBeenCalledTimes(2)
    expect(session.items.value.find((item) => item.path === 'a')?.low.status).toBe('done')
  })

  it('ignores selecting the current baseline again', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ similarity: 0.9, durationMs: 3 })),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'candidate'])
    await session.setBaseline('base')

    await session.setBaseline('base')

    expect(deps.computeLow).toHaveBeenCalledTimes(1)
  })

  it('computes only newly imported images when restoring the current baseline', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ similarity: 0.9, durationMs: 3 })),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'existing'])
    await session.setBaseline('base')
    await session.addPaths(['new'])

    await session.setBaseline('base')

    expect(deps.computeLow).toHaveBeenCalledTimes(2)
    expect(session.items.value.find((item) => item.path === 'existing')?.low.status).toBe('done')
    expect(session.items.value.find((item) => item.path === 'new')?.low.status).toBe('done')
  })

  it('does not start a new baseline generation until the running low task finishes', async () => {
    let resolveFirst!: (value: { similarity: number; durationMs: number }) => void
    const first = new Promise<{ similarity: number; durationMs: number }>((resolve) => {
      resolveFirst = resolve
    })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn()
        .mockReturnValueOnce(first)
        .mockResolvedValue({ similarity: 0.8, durationMs: 2 }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['a', 'b'])
    const oldRun = session.setBaseline('a')
    await Promise.resolve()
    expect(deps.computeLow).toHaveBeenCalledTimes(1)

    const newRun = session.setBaseline('b')
    await Promise.resolve()
    expect(deps.computeLow).toHaveBeenCalledTimes(1)

    resolveFirst({ similarity: 0.9, durationMs: 1 })
    await oldRun
    await newRun
    expect(deps.computeLow).toHaveBeenCalledTimes(2)
  })

  it('skips a queued candidate removed during another low calculation', async () => {
    let resolveFirst!: (value: { similarity: number; durationMs: number }) => void
    const first = new Promise<{ similarity: number; durationMs: number }>((resolve) => {
      resolveFirst = resolve
    })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn().mockReturnValueOnce(first),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'a', 'b'])
    const run = session.setBaseline('base')

    session.remove('b')
    resolveFirst({ similarity: 0.9, durationMs: 1 })
    await run

    expect(deps.computeLow).toHaveBeenCalledTimes(1)
  })

  it('shows pHash distance immediately even when low precision later fails', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => {
        throw new Error('低精度失败')
      }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'candidate'])
    session.items.value[0].phash = '0000000000000000'
    session.items.value[1].phash = '0000000000000003'

    await session.setBaseline('base')

    const candidate = session.items.value.find((item) => item.path === 'candidate')
    expect(candidate?.phashDistance).toBe(2)
    expect(candidate?.low.status).toBe('error')
  })

  it('retries only the requested failed low precision comparison', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn()
        .mockRejectedValueOnce(new Error('临时失败'))
        .mockResolvedValueOnce({ similarity: 0.9, durationMs: 3 }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'candidate'])
    await session.setBaseline('base')

    await session.retryLowPrecision('candidate')

    expect(deps.computeLow).toHaveBeenCalledTimes(2)
    expect(session.items.value.find((item) => item.path === 'candidate')?.low.status).toBe('done')
  })

  it('does not enqueue the same low precision retry more than once', async () => {
    let resolveBlocking!: (value: { similarity: number; durationMs: number }) => void
    const blocking = new Promise<{ similarity: number; durationMs: number }>((resolve) => {
      resolveBlocking = resolve
    })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn()
        .mockRejectedValueOnce(new Error('首次失败'))
        .mockReturnValueOnce(blocking)
        .mockResolvedValue({ similarity: 0.9, durationMs: 3 }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'failed', 'blocking'])
    const initialRun = session.setBaseline('base')
    await vi.waitFor(() => expect(deps.computeLow).toHaveBeenCalledTimes(2))

    const firstRetry = session.retryLowPrecision('failed')
    const secondRetry = session.retryLowPrecision('failed')

    expect(session.items.value.find((item) => item.path === 'failed')?.low.status).toBe('queued')
    expect(await secondRetry).toBe(false)
    resolveBlocking({ similarity: 0.8, durationMs: 2 })
    await initialRun
    await firstRetry
    expect(deps.computeLow).toHaveBeenCalledTimes(3)
  })

  it('does not start standard ssim until one card requests it', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ similarity: 1, durationMs: 1 })),
      computeHigh: vi.fn(async () => ({ score: 0.98, durationMs: 20 }))
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'candidate'])
    await session.setBaseline('base')

    expect(deps.computeHigh).not.toHaveBeenCalled()
    await session.computeHighPrecision('candidate')

    expect(deps.computeHigh).toHaveBeenCalledTimes(1)
  })

  it('reuses a standard ssim result for the same unordered unchanged pair', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ similarity: 1, durationMs: 1 })),
      computeHigh: vi.fn(async () => ({ score: 0.98, durationMs: 20 }))
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['a', 'b'])
    await session.setBaseline('a')
    await session.computeHighPrecision('b')
    await session.setBaseline('b')

    await session.computeHighPrecision('a')

    expect(deps.computeHigh).toHaveBeenCalledTimes(1)
  })

  it('discards low precision results from an old baseline generation', async () => {
    let resolveFirst!: (value: { similarity: number; durationMs: number }) => void
    const first = new Promise<{ similarity: number; durationMs: number }>((resolve) => {
      resolveFirst = resolve
    })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn()
        .mockReturnValueOnce(first)
        .mockResolvedValue({ similarity: 0.8, durationMs: 2 }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['a', 'b'])
    const oldRun = session.setBaseline('a')
    await Promise.resolve()
    const newRun = session.setBaseline('b')
    resolveFirst({ similarity: 0.1, durationMs: 9 })

    await oldRun
    await newRun

    expect(session.baselinePath.value).toBe('b')
    expect(session.items.value.find((item) => item.path === 'b')?.low.status).toBe('baseline')
  })

  it('requires close confirmation only when the session has content', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)

    expect(session.hasContent.value).toBe(false)
    await session.addPaths(['a'])
    expect(session.hasContent.value).toBe(true)
    session.reset()
    expect(session.hasContent.value).toBe(false)
  })

  it('keeps pending low precision work in hasContent after reset', async () => {
    let resolveLow!: (value: { similarity: number; durationMs: number }) => void
    const pendingLow = new Promise<{ similarity: number; durationMs: number }>(
      (resolve) => { resolveLow = resolve }
    )
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(() => pendingLow),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'candidate'])
    const run = session.setBaseline('base')

    session.reset()

    expect(session.hasContent.value).toBe(true)
    expect(session.hasRunningTasks.value).toBe(true)
    resolveLow({ similarity: 1, durationMs: 1 })
    await run
    expect(session.hasContent.value).toBe(false)
    expect(session.hasRunningTasks.value).toBe(false)
  })

  it('does not allow reset to start another standard ssim while the old one is running', async () => {
    let resolveHigh!: (value: { score: number; durationMs: number }) => void
    const pendingHigh = new Promise<{ score: number; durationMs: number }>((resolve) => {
      resolveHigh = resolve
    })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ similarity: 1, durationMs: 1 })),
      computeHigh: vi.fn()
        .mockReturnValueOnce(pendingHigh)
        .mockResolvedValueOnce({ score: 0.9, durationMs: 2 })
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['old-base', 'old-candidate'])
    await session.setBaseline('old-base')
    const oldRun = session.computeHighPrecision('old-candidate')

    session.reset()
    await session.addPaths(['new-base', 'new-candidate'])
    await session.setBaseline('new-base')
    const started = await session.computeHighPrecision('new-candidate')

    expect(started).toBe(false)
    expect(deps.computeHigh).toHaveBeenCalledTimes(1)
    expect(session.hasContent.value).toBe(true)
    resolveHigh({ score: 1, durationMs: 1 })
    await oldRun
  })
})
