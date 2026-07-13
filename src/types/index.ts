// TypeScript 类型定义

/** 图片元数据 */
export interface Image {
  id: number
  filePath: string
  relativePath: string
  fileSize: number
  fileModifiedAt: number
  width: number
  height: number
  format: string
  aspectRatio: number
  blake3Hash?: string
  hashComputedAt?: number
  scanId: number
  scannedAt: number
}

/** 扫描任务 */
export interface Scan {
  id: number
  rootPath: string
  status: ScanStatus
  totalFiles: number
  scannedFiles: number
  hashComputed: number
  lastScannedPath?: string
  createdAt: number
  startedAt?: number
  completedAt?: number
}

/** 扫描状态 */
export enum ScanStatus {
  Pending = 'pending',
  Running = 'running',
  Paused = 'paused',
  Completed = 'completed',
  Cancelled = 'cancelled'
}

/** 完全重复文件 */
export interface Duplicate {
  id: number
  hashGroup: string
  originalImageId: number
  duplicateImageId: number
  status: DeleteStatus
  markedAt: number
}

/** 相似图片配对 */
export interface SimilarPair {
  id: number
  largerImageId: number
  smallerImageId: number
  ssimScore?: number
  sizeRatio: number
  resolutionRatio: number
  isCompressedVersion: boolean
  ssimThreshold?: number
  status: DeleteStatus
  markedAt: number
  computedAt?: number
}

/** 删除状态 */
export enum DeleteStatus {
  Pending = 'pending',
  Recycled = 'recycled',
  Deleted = 'deleted',
  Kept = 'kept',
  Skipped = 'skipped'
}

/** 回收站记录 */
export interface RecycleBinEntry {
  id: number
  originalPath: string
  recycledPath: string
  deleteReason: DeleteReason
  relatedImageId?: number
  duplicateId?: number
  similarPairId?: number
  fileSize: number
  width: number
  height: number
  blake3Hash?: string
  ssimScore?: number
  recycledAt: number
}

/** 删除原因 */
export enum DeleteReason {
  ExactDuplicate = 'exact_duplicate',
  LowerResolution = 'lower_resolution'
}

/** 扫描进度事件 */
export interface ScanProgressEvent {
  scanId: number
  totalFiles: number
  scannedFiles: number
  currentFile: string
  estimatedTimeRemaining?: number
}

/** 哈希进度事件 */
export interface HashProgressEvent {
  scanId: number
  totalFiles: number
  hashedFiles: number
  currentFile: string
}

/** 用户设置 */
export interface Settings {
  ssimThreshold: number
  duplicateKeepStrategy: 'shortest_path' | 'earliest_time' | 'preferred_dir'
  preferredDirectory: string
  autoRecycleDuplicates: boolean
  autoRecycleCompressed: boolean
}
