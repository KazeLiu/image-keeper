import { invoke } from '@tauri-apps/api/core'

export interface TestImageInfo {
  path: string
  fileName: string
  fileSize: number
  width: number
  height: number
  modifiedAtMs: number
  thumbnailDataUrl: string
}

export interface TestImagePhashResult {
  phash: string
}

export interface TestLowPrecisionResult {
  similarity: number
  durationMs: number
}

export interface TestStandardSsimResult {
  score: number
  durationMs: number
}

export function loadTestImage(path: string): Promise<TestImageInfo> {
  return invoke<TestImageInfo>('load_test_image', { path })
}

export function computeTestPhash(image: TestImageInfo): Promise<TestImagePhashResult> {
  return invoke<TestImagePhashResult>('compute_test_phash', {
    path: image.path,
    fileSize: image.fileSize,
    modifiedAtMs: image.modifiedAtMs
  })
}

export function computeTestLowPrecision(
  baseline: TestImageInfo,
  candidate: TestImageInfo
): Promise<TestLowPrecisionResult> {
  return invoke<TestLowPrecisionResult>('compute_test_low_precision', {
    baselinePath: baseline.path,
    candidatePath: candidate.path
  })
}

export function computeTestStandardSsim(
  baseline: TestImageInfo,
  candidate: TestImageInfo
): Promise<TestStandardSsimResult> {
  return invoke<TestStandardSsimResult>('compute_test_standard_ssim', {
    baselinePath: baseline.path,
    candidatePath: candidate.path,
    baselineFileSize: baseline.fileSize,
    baselineModifiedAtMs: baseline.modifiedAtMs,
    candidateFileSize: candidate.fileSize,
    candidateModifiedAtMs: candidate.modifiedAtMs
  })
}
