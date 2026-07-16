import { beforeEach, describe, expect, it, vi } from 'vitest'
import { computeTestLowPrecision, type TestImageInfo } from './imageMetrics'

const invoke = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

function image(path: string, phash: string): TestImageInfo {
  return {
    path,
    fileName: `${path}.png`,
    fileSize: 100,
    width: 100,
    height: 100,
    modifiedAtMs: 1,
    phash,
    thumbnailDataUrl: 'data:image/png;base64,test'
  }
}

describe('image metrics api', () => {
  beforeEach(() => vi.clearAllMocks())

  it('keeps pHash data out of the low precision image task', async () => {
    invoke.mockResolvedValue({ similarity: 0.9, durationMs: 2 })

    await computeTestLowPrecision(
      image('base', '0000000000000000'),
      image('candidate', '0000000000000001')
    )

    expect(invoke).toHaveBeenCalledWith('compute_test_low_precision', {
      baselinePath: 'base',
      candidatePath: 'candidate'
    })
  })
})
