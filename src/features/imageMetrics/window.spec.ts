import { beforeEach, describe, expect, it, vi } from 'vitest'
import { openImageMetricsWindow } from './window'

const windowMocks = vi.hoisted(() => ({
  getByLabel: vi.fn(),
  construct: vi.fn(),
  once: vi.fn()
}))

vi.mock('@tauri-apps/api/webviewWindow', () => {
  function MockWebviewWindow(
    this: { label: string; once: typeof windowMocks.once },
    label: string,
    options: unknown
  ) {
    this.label = label
    this.once = windowMocks.once
    windowMocks.construct(label, options)
  }
  MockWebviewWindow.getByLabel = windowMocks.getByLabel
  return { WebviewWindow: MockWebviewWindow }
})

describe('image metrics window', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    windowMocks.once.mockImplementation(async (event: string, handler: (event: unknown) => void) => {
      if (event === 'tauri://created') queueMicrotask(() => handler({ payload: null }))
      return () => undefined
    })
  })

  it('creates the standalone window with the fixed route and size', async () => {
    windowMocks.getByLabel.mockResolvedValue(null)

    await openImageMetricsWindow()

    expect(windowMocks.construct).toHaveBeenCalledWith(
      'image-metrics-test',
      expect.objectContaining({
        url: '/image-metrics-test',
        width: 1180,
        height: 820,
        minWidth: 840,
        minHeight: 600
      })
    )
  })

  it('focuses the existing window instead of creating a duplicate', async () => {
    const existing = {
      show: vi.fn(async () => undefined),
      unminimize: vi.fn(async () => undefined),
      setFocus: vi.fn(async () => undefined)
    }
    windowMocks.getByLabel.mockResolvedValue(existing)

    const result = await openImageMetricsWindow()

    expect(result).toBe(existing)
    expect(existing.show).toHaveBeenCalledTimes(1)
    expect(existing.unminimize).toHaveBeenCalledTimes(1)
    expect(existing.setFocus).toHaveBeenCalledTimes(1)
    expect(windowMocks.construct).not.toHaveBeenCalled()
  })

  it('shares one pending creation across concurrent open requests', async () => {
    windowMocks.getByLabel.mockResolvedValue(null)

    const [first, second] = await Promise.all([
      openImageMetricsWindow(),
      openImageMetricsWindow()
    ])

    expect(first).toBe(second)
    expect(windowMocks.getByLabel).toHaveBeenCalledTimes(1)
    expect(windowMocks.construct).toHaveBeenCalledTimes(1)
  })

  it('rejects a native window creation error and allows a later retry', async () => {
    windowMocks.getByLabel.mockResolvedValue(null)
    windowMocks.once.mockImplementation(async (event: string, handler: (event: unknown) => void) => {
      if (event === 'tauri://error') {
        queueMicrotask(() => handler({ payload: '创建失败' }))
      }
      return () => undefined
    })

    await expect(openImageMetricsWindow()).rejects.toThrow('创建失败')

    windowMocks.once.mockImplementation(async (event: string, handler: (event: unknown) => void) => {
      if (event === 'tauri://created') queueMicrotask(() => handler({ payload: null }))
      return () => undefined
    })
    await openImageMetricsWindow()
    expect(windowMocks.construct).toHaveBeenCalledTimes(2)
  })
})
