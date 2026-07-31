// 多目录对比 API
import { invoke } from '@tauri-apps/api/core'
import type {
  MultiCompareRequest,
  ComparisonStats,
  ComparisonResultRow,
  ComparisonGroup,
  GroupSimilarityScore,
  GroupSimilarityStatus,
  ImageRecycleOutcome,
  RunStatusResponse,
  ComparisonRunHistoryItem
} from '@/types'

/** 开始多目录对比 */
export async function startMultiCompare(request: MultiCompareRequest): Promise<string> {
  return await invoke<string>('start_multi_compare', { request })
}

/** 获取对比统计 */
export async function getComparisonStats(runId: string): Promise<ComparisonStats> {
  return await invoke<ComparisonStats>('get_comparison_stats', { runId })
}

/** 获取分类结果列表 */
export async function getComparisonResults(runId: string): Promise<ComparisonResultRow[]> {
  return await invoke<ComparisonResultRow[]>('get_comparison_results', { runId })
}

/** 获取相似图片分组 */
export async function getComparisonGroups(
  runId: string,
  groupingDistance?: number
): Promise<ComparisonGroup[]> {
  return await invoke<ComparisonGroup[]>('get_comparison_groups', { runId, groupingDistance })
}

/** 获取当前组内两两相似度 */
export async function getGroupSimilarityScores(
  runId: string,
  imageIds: number[],
  requestId: string,
  groupingDistance: number,
  groupIndex: number
): Promise<GroupSimilarityScore[]> {
  return await invoke<GroupSimilarityScore[]>('get_group_similarity_scores', {
    runId,
    imageIds,
    requestId,
    groupingDistance,
    groupIndex
  })
}

/** 取消仍在排队或计算中的前台组内 SSIM 请求 */
export async function cancelGroupSimilarityRequest(requestId: string): Promise<void> {
  await invoke('cancel_group_similarity_request', { requestId })
}

/** 获取正式任务中全部自动分组的 SSIM 缓存与后台状态 */
export async function getGroupSimilarityStatuses(
  runId: string,
  groupingDistance: number
): Promise<GroupSimilarityStatus[]> {
  return await invoke<GroupSimilarityStatus[]>('get_group_similarity_statuses', {
    runId,
    groupingDistance
  })
}

/** 当前组完成后，低优先级补算正式任务中的其他分组 */
export async function startGroupSimilarityBackfill(
  runId: string,
  groupingDistance: number,
  afterGroupIndices: number[]
): Promise<void> {
  await invoke('start_group_similarity_backfill', {
    runId,
    groupingDistance,
    afterGroupIndices
  })
}

/** 按图片 ID 批量移动到回收站 */
export async function batchRecycleImages(
  runId: string,
  imageIds: number[]
): Promise<ImageRecycleOutcome[]> {
  return await invoke<ImageRecycleOutcome[]>('batch_recycle_images', { runId, imageIds })
}

/** 获取运行状态 */
export async function getRunStatus(runId: string): Promise<RunStatusResponse> {
  return await invoke<RunStatusResponse>('get_run_status', { runId })
}

/** 获取最近对比历史 */
export async function listComparisonRuns(limit = 20): Promise<ComparisonRunHistoryItem[]> {
  return await invoke<ComparisonRunHistoryItem[]>('list_comparison_runs', { limit })
}

/** 删除对比历史数据库记录，不删除图片文件 */
export async function deleteComparisonRun(runId: string): Promise<void> {
  await invoke('delete_comparison_run', { runId })
}
