import { beforeEach, describe, expect, it, vi } from 'vitest'
import { openImageMetricsWindow } from './window'

const windowMocks = vi.hoisted(() => ({
  getByLabel: vi.fn(),
  construct: vi.fn()
}))

vi.mock('@tauri-apps/api/webviewWindow', () => {
  function MockWebviewWindow(this: { label: string }, label: string, options: unknown) {
    this.label = label
    windowMocks.construct(label, options)
  }
  MockWebviewWindow.getByLabel = windowMocks.getByLabel
  return { WebviewWindow: MockWebviewWindow }
})

describe('image metrics window', () => {
  beforeEach(() => vi.clearAllMocks())

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
})
