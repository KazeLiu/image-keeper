import { beforeEach, describe, expect, it, vi } from 'vitest'
import { computeTestLowPrecision, computeTestPhash, type TestImageInfo } from './imageMetrics'

const invoke = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

function image(path: string): TestImageInfo {
  return {
    path,
    fileName: `${path}.png`,
    fileSize: 100,
    width: 100,
    height: 100,
    modifiedAtMs: 1,
    thumbnailDataUrl: 'data:image/png;base64,test'
  }
}

describe('image metrics api', () => {
  beforeEach(() => vi.clearAllMocks())

  it('低精度图片任务不携带感知哈希数据', async () => {
    invoke.mockResolvedValue({ similarity: 0.9, durationMs: 2 })

    await computeTestLowPrecision(
      image('base'),
      image('candidate')
    )

    expect(invoke).toHaveBeenCalledWith('compute_test_low_precision', {
      baselinePath: 'base',
      candidatePath: 'candidate'
    })
  })

  it('单独请求感知哈希时携带文件指纹', async () => {
    invoke.mockResolvedValue({ phash: '0000000000000000' })

    await computeTestPhash(image('base'))

    expect(invoke).toHaveBeenCalledWith('compute_test_phash', {
      path: 'base',
      fileSize: 100,
      modifiedAtMs: 1
    })
  })
})
