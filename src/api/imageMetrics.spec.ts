import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  computeTestDifferencePreview,
  computeTestPhash,
  computeTestSsim,
  type TestImageInfo
} from './imageMetrics'

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

  it('标准 ssim 任务携带两张图片的文件指纹', async () => {
    invoke.mockResolvedValue({ score: 0.9, durationMs: 2 })

    await computeTestSsim(
      image('base'),
      image('candidate')
    )

    expect(invoke).toHaveBeenCalledWith('compute_test_ssim', {
      baselinePath: 'base',
      candidatePath: 'candidate',
      baselineFileSize: 100,
      baselineModifiedAtMs: 1,
      candidateFileSize: 100,
      candidateModifiedAtMs: 1
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

  it('差异高亮任务携带两张图片指纹和灵敏度', async () => {
    invoke.mockResolvedValue({ regionCount: 0, changedPixelRatio: 0 })
    const baseline = { ...image('base.png'), fileSize: 10, modifiedAtMs: 11 }
    const candidate = { ...image('candidate.png'), fileSize: 20, modifiedAtMs: 21 }

    await computeTestDifferencePreview(baseline, candidate, 50)

    expect(invoke).toHaveBeenCalledWith('compute_test_difference_preview', {
      baselinePath: 'base.png',
      candidatePath: 'candidate.png',
      baselineFileSize: 10,
      baselineModifiedAtMs: 11,
      candidateFileSize: 20,
      candidateModifiedAtMs: 21,
      sensitivity: 50
    })
  })
})
