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

  it('automatically computes low precision for every non-baseline image', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ phashDistance: 2, similarity: 0.9, durationMs: 3 })),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['base', 'a', 'b'])

    await session.setBaseline('base')

    expect(deps.computeLow).toHaveBeenCalledTimes(2)
    expect(session.items.value.find((item) => item.path === 'a')?.low.status).toBe('done')
  })

  it('does not start standard ssim until one card requests it', async () => {
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn(async () => ({ phashDistance: 0, similarity: 1, durationMs: 1 })),
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
      computeLow: vi.fn(async () => ({ phashDistance: 0, similarity: 1, durationMs: 1 })),
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
    let resolveFirst!: (value: { phashDistance: number; similarity: number; durationMs: number }) => void
    const first = new Promise<{ phashDistance: number; similarity: number; durationMs: number }>((resolve) => {
      resolveFirst = resolve
    })
    const deps: ImageMetricsDependencies = {
      loadImage: vi.fn(async (path) => image(path)),
      computeLow: vi.fn()
        .mockReturnValueOnce(first)
        .mockResolvedValue({ phashDistance: 1, similarity: 0.8, durationMs: 2 }),
      computeHigh: vi.fn()
    }
    const session = createImageMetricsSession(deps)
    await session.addPaths(['a', 'b'])
    const oldRun = session.setBaseline('a')
    await session.setBaseline('b')
    resolveFirst({ phashDistance: 9, similarity: 0.1, durationMs: 9 })

    await oldRun

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
})
