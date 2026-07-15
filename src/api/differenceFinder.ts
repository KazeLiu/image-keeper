import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MatchClassification =
  | 'exact'
  | 'compressed_or_reencoded'
  | 'variant'
  | 'related_group'
  | 'weak_candidate'

export interface DifferenceReferenceInput {
  id: string
  path: string
}

export interface DifferenceSearchRequest {
  sessionId: string
  references: DifferenceReferenceInput[]
  targetRoots: string[]
  recursive: boolean
}

export interface ReferenceRelation {
  referenceId: string
  referencePath: string
  classification: MatchClassification
  phashDistance: number
  similarity?: number | null
}

export interface DifferenceMatchItem {
  filePath: string
  fileName: string
  sourceRoot: string
  relativePath: string
  fileSize: number
  modifiedAt: number
  width: number
  height: number
  format: string
  blake3Hash: string
  classification: MatchClassification
  bestReferenceId: string
  relations: ReferenceRelation[]
}

export interface SearchFileError {
  filePath: string
  message: string
}

export interface DifferenceSearchResponse {
  sessionId: string
  scannedFileCount: number
  validReferenceCount: number
  matches: DifferenceMatchItem[]
  errors: SearchFileError[]
}

export type DifferenceSearchPhase =
  | 'scanning'
  | 'extracting'
  | 'matching'
  | 'aggregating'
  | 'completed'

export interface DifferenceSearchProgress {
  sessionId: string
  phase: DifferenceSearchPhase
  processed: number
  total: number
  currentFile?: string | null
}

export interface FileFingerprint {
  blake3Hash: string
  fileSize: number
  modifiedAt: number
}

export type RenameRule =
  | { mode: 'simple'; template: string }
  | { mode: 'advanced'; oldPattern: string; newTemplate: string }
  | { mode: 'quick'; firstName: string }

export interface RenameInput {
  sourcePath: string
  referenceName: string
  groupIndex: number
  order: number
  expectedFingerprint: FileFingerprint
}

export interface RenameExecutionItem {
  sourcePath: string
  newName: string
  expectedFingerprint: FileFingerprint
}

export type FilePlanIssueKind =
  | 'invalid_name'
  | 'batch_duplicate'
  | 'target_exists'
  | 'same_content_exists'
  | 'rule_unmatched'
  | 'source_missing'
  | 'source_changed'

export interface FilePlanIssue {
  kind: FilePlanIssueKind
  message: string
  blocking: boolean
}

export interface RenamePreviewItem {
  sourcePath: string
  originalName: string
  proposedName: string
  targetPath: string
  issues: FilePlanIssue[]
  blocking: boolean
}

export interface OperationEntry {
  sourcePath: string
  targetPath: string
  status: 'succeeded' | 'skipped' | 'failed'
  message?: string | null
  targetFingerprint?: FileFingerprint | null
}

export interface OperationBatchResult {
  batchId: string
  kind: 'rename' | 'move' | 'copy' | 'undo'
  entries: OperationEntry[]
  succeeded: number
  skipped: number
  failed: number
  reversible: boolean
}

export interface TransferFilesRequest {
  files: TransferInput[]
  targetDirectory: string
  newFolderName?: string | null
}

export interface TransferInput {
  sourcePath: string
  expectedFingerprint: FileFingerprint
}

export interface TransferPreviewItem {
  sourcePath: string
  targetPath: string
  issues: FilePlanIssue[]
  conflict: boolean
}

export interface TransferPreview {
  destination: string
  items: TransferPreviewItem[]
  conflictCount: number
}

export function startDifferenceSearch(request: DifferenceSearchRequest) {
  return invoke<DifferenceSearchResponse>('start_difference_search', { request })
}

export function cancelDifferenceSearch(sessionId: string) {
  return invoke<void>('cancel_difference_search', { sessionId })
}

export function previewDifferenceRename(items: RenameInput[], rule: RenameRule) {
  return invoke<RenamePreviewItem[]>('preview_difference_rename', {
    request: { items, rule }
  })
}

export function executeDifferenceRename(items: RenameExecutionItem[]) {
  return invoke<OperationBatchResult>('execute_difference_rename', { request: { items } })
}

export function previewDifferenceExplicitRename(items: RenameExecutionItem[]) {
  return invoke<RenamePreviewItem[]>('preview_difference_explicit_rename', {
    request: { items }
  })
}

export function moveDifferenceFiles(request: TransferFilesRequest) {
  return invoke<OperationBatchResult>('move_difference_files', { request })
}

export function previewDifferenceTransfer(request: TransferFilesRequest) {
  return invoke<TransferPreview>('preview_difference_transfer', { request })
}

export function copyDifferenceFiles(request: TransferFilesRequest) {
  return invoke<OperationBatchResult>('copy_difference_files', { request })
}

export function undoDifferenceBatch(batchId: string) {
  return invoke<OperationBatchResult>('undo_difference_batch', { batchId })
}

export function listenDifferenceSearchProgress(
  callback: (progress: DifferenceSearchProgress) => void
): Promise<UnlistenFn> {
  return listen<DifferenceSearchProgress>('difference-search-progress', event => callback(event.payload))
}
