// 多目录对比 Store
import { defineStore } from 'pinia'
import { reactive, ref, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  startMultiCompare,
  getComparisonStats,
  getComparisonResults,
  getComparisonGroups,
  getGroupSimilarityStatuses,
  getRunStatus,
  listComparisonRuns,
  deleteComparisonRun
} from '@/api/comparison'
import type {
  MultiCompareRequest,
  ComparisonStats,
  ComparisonResultRow,
  ComparisonGroup,
  GroupSimilarityStatus,
  GroupSimilarityStatusValue,
  MultiCompareProgressEvent,
  RunStatusResponse,
  ComparisonRunHistoryItem
} from '@/types'
import { RunStatus } from '@/types'
import { readStoredRecognitionThreshold } from '@/features/similarity'

/** 多目录选择模型 */
interface DirectoryRow {
  id: string
  path: string
  name: string
  alias: string
  compareWithin: boolean
}

interface DirectorySelectionModel {
  directories: DirectoryRow[]
}

interface ManualMergeSet {
  id: string
  sourceGroupIndices: number[]
}

const DEFAULT_GROUPING_DISTANCE = 10
const MIN_GROUPING_DISTANCE = 0
const MAX_GROUPING_DISTANCE = 24
const LAST_RUN_ID_KEY = 'imagekeeper:last-comparison-run-id'

export const useComparisonStore = defineStore('comparison', () => {
  // 目录选择
  const directorySelection = reactive<DirectorySelectionModel>({
    directories: []
  })

  // 当前运行
  const currentRunId = ref<string>('')
  const currentPhase = ref<string>('')
  const isRunning = ref(false)
  const errorMessage = ref<string>('')
  const historyRuns = ref<ComparisonRunHistoryItem[]>([])
  const isLoadingHistory = ref(false)
  const hasInitializedHistory = ref(false)

  // 进度
  const progressModel = reactive({
    phase: '',
    totalFiles: 0,
    processedFiles: 0,
    currentFile: '',
    percentage: 0
  })

  // 统计结果
  const stats = ref<ComparisonStats | null>(null)
  const results = ref<ComparisonResultRow[]>([])
  const autoGroups = ref<ComparisonGroup[]>([])
  const selectedGroupIndex = ref<number | null>(null)
  const selectedMemberId = ref<number | null>(null)
  const selectedGroupIds = ref<number[]>([])
  const checkedImageIds = ref<number[]>([])
  const manualMergeSets = ref<ManualMergeSet[]>([])
  const groupingDistance = ref(DEFAULT_GROUPING_DISTANCE)
  const appliedGroupingDistance = ref(DEFAULT_GROUPING_DISTANCE)
  const isRefreshingGroups = ref(false)
  const groupingDataRevision = ref(0)
  const groupEditMode = ref(false)
  const originalRecognitionThreshold = ref(readStoredRecognitionThreshold(window.localStorage))
  const groupSimilarityStatuses = ref<Record<string, GroupSimilarityStatus>>({})

  let unlistenProgress: UnlistenFn | null = null
  let unlistenComplete: UnlistenFn | null = null
  let statusPollingTimer: ReturnType<typeof window.setInterval> | null = null
  let groupingRefreshTimer: ReturnType<typeof window.setTimeout> | null = null
  let unlistenGroupSimilarityStatus: UnlistenFn | null = null
  let groupSimilarityStatusListenerPromise: Promise<void> | null = null
  let historyLoadGeneration = 0
  let groupingRefreshGeneration = 0
  const bufferedProgress = new Map<string, MultiCompareProgressEvent>()
  const bufferedCompletions = new Set<string>()
  const groupSimilarityStatusRequests = new Map<string, Promise<void>>()

  // 计算属性：进度百分比
  const progressPercentage = computed(() => {
    if (progressModel.totalFiles === 0) return 0
    return Math.round((progressModel.processedFiles / progressModel.totalFiles) * 100)
  })

  // 计算属性：分类统计数组
  const categoryStats = computed(() => {
    if (!stats.value) return []

    return [
      {
        type: 'exact_duplicate',
        label: '完全重复',
        count: stats.value.exact_duplicate,
        color: '#f56c6c',
        description: '文件内容完全一样，通常可以只保留一张。这个判断主要看文件哈希。'
      },
      {
        type: 'likely_compressed',
        label: '疑似压缩',
        count: stats.value.likely_compressed,
        color: '#e6a23c',
        description: '图片内容高度相似，但这一张分辨率或文件体积更小，可能是被压缩、缩放或二次保存过的版本。'
      },
      {
        type: 'variant',
        label: '相似变体',
        count: stats.value.variant,
        color: '#409eff',
        description: '图片内容相近，但可能存在裁剪、调色、加字、构图变化等差异，不建议直接自动删除。'
      },
      {
        type: 'similar_keep',
        label: '相似但保留',
        count: stats.value.similar_keep,
        color: '#67c23a',
        description: '系统找到相似对象，但没有足够依据判断它是低质量版本，默认建议保留。'
      },
      {
        type: 'no_baseline_match',
        label: '无相似对象',
        count: stats.value.no_baseline_match,
        color: '#909399',
        description: '这张图没有在当前对比范围内找到足够相似的图片，暂时视为独立图片。'
      },
      {
        type: 'inconclusive',
        label: '需人工确认',
        count: stats.value.inconclusive,
        color: '#c0c4cc',
        description: '算法信息不够确定，可能相似但证据不足，需要你人工看图决定。'
      },
      {
        type: 'not_evaluated',
        label: '未评估',
        count: stats.value.not_evaluated,
        color: '#dcdfe6',
        description: '这部分图片还没有完成相似度评估，通常出现在任务中断、异常或后续流程尚未覆盖时。'
      },
      {
        type: 'error',
        label: '错误',
        count: stats.value.error,
        color: '#f56c6c',
        description: '分析这张图片时出错了，可能是文件损坏、格式不支持、读取失败或计算过程异常。'
      }
    ]
  })

  const groups = computed(() => buildDisplayGroups(autoGroups.value, manualMergeSets.value))

  const hasAnyCheckedSelection = computed(() =>
    selectedGroupIds.value.length > 0 || checkedImageIds.value.length > 0
  )

  // 计算属性：守恒验证
  const conservationCheck = computed(() => {
    if (!stats.value) return { valid: true, message: '' }

    const sum =
      stats.value.exact_duplicate +
      stats.value.likely_compressed +
      stats.value.variant +
      stats.value.similar_keep +
      stats.value.no_baseline_match +
      stats.value.inconclusive +
      stats.value.not_evaluated +
      stats.value.error

    const isValid = sum === stats.value.comparison_total
    const message = isValid
      ? '统计完整：所有参与分析的图片都已归类'
      : `统计异常：分类数量合计为 ${sum}，但参与分析图片为 ${stats.value.comparison_total}，可能有漏算或重复统计`

    return { valid: isValid, message }
  })

  // 添加目录
  function addDirectory(path: string) {
    if (directorySelection.directories.some((item) => item.path === path)) {
      throw new Error('该目录已添加')
    }

    directorySelection.directories.push({
      id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      path,
      name: getFolderName(path),
      alias: '',
      compareWithin: false
    })
    reassignAliases()
  }

  // 批量添加目录
  function addDirectories(paths: string[]) {
    let addedCount = 0
    let skippedCount = 0

    for (const path of paths) {
      if (directorySelection.directories.some((item) => item.path === path)) {
        skippedCount += 1
        continue
      }

      directorySelection.directories.push({
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        path,
        name: getFolderName(path),
        alias: '',
        compareWithin: false
      })
      addedCount += 1
    }

    reassignAliases()
    return { addedCount, skippedCount }
  }

  // 移除目录
  function removeDirectory(index: number) {
    directorySelection.directories.splice(index, 1)
    reassignAliases()
  }

  // 清空选择
  function clearSelection() {
    directorySelection.directories = []
  }

  // 开始对比
  async function startComparison() {
    if (!canStartComparison.value) {
      throw new Error('请至少添加 2 个目录，或勾选一个目录的“内部同时对比”')
    }

    const [baselineDirectory, ...comparisonDirectories] = directorySelection.directories
    const comparisonPaths = comparisonDirectories.map((p) => p.path)

    if (baselineDirectory.compareWithin && !comparisonPaths.includes(baselineDirectory.path)) {
      comparisonPaths.unshift(baselineDirectory.path)
    }

    const request: MultiCompareRequest = {
      baseline_path: baselineDirectory.path,
      comparison_paths: comparisonPaths,
      directory_options: directorySelection.directories.map((item) => ({
        path: item.path,
        compare_within: item.compareWithin
      }))
    }

    cleanupRuntime()
    resetProgress()

    isRunning.value = true
    errorMessage.value = ''
    currentPhase.value = 'preflight'

    await setupRuntimeListeners()

    try {
      const runId = await startMultiCompare(request)
      currentRunId.value = runId
      rememberRunId(runId)

      const earlyProgress = bufferedProgress.get(runId)
      if (earlyProgress) {
        applyProgress(earlyProgress)
        bufferedProgress.delete(runId)
      }

      startStatusPolling()

      if (bufferedCompletions.has(runId)) {
        bufferedCompletions.delete(runId)
        await reconcileRunStatus(runId)
      }
    } catch (error) {
      cleanupRuntime()
      isRunning.value = false
      currentPhase.value = ''
      currentRunId.value = ''
      throw error
    }
  }

  // 刷新统计
  async function refreshStats() {
    if (!currentRunId.value) return
    stats.value = await getComparisonStats(currentRunId.value)
  }

  // 刷新分类结果
  async function refreshResults() {
    if (!currentRunId.value) return
    results.value = await getComparisonResults(currentRunId.value)
  }

  async function refreshGroups() {
    if (!currentRunId.value) return
    const runId = currentRunId.value
    const distance = appliedGroupingDistance.value
    const refreshGeneration = ++groupingRefreshGeneration
    isRefreshingGroups.value = true
    try {
      const nextGroups = await getComparisonGroups(runId, distance)
      if (
        refreshGeneration !== groupingRefreshGeneration ||
        runId !== currentRunId.value ||
        distance !== appliedGroupingDistance.value
      ) return

      autoGroups.value = nextGroups
      groupingDataRevision.value += 1
      ensureSelectedGroup()
      void refreshGroupSimilarityStatuses()
    } finally {
      if (refreshGeneration === groupingRefreshGeneration) {
        isRefreshingGroups.value = false
      }
    }
  }

  async function refreshAnalysisData() {
    if (!currentRunId.value) return
    const runId = currentRunId.value
    const distance = appliedGroupingDistance.value

    const [nextStats, nextResults, nextGroups] = await Promise.all([
      getComparisonStats(runId),
      getComparisonResults(runId),
      getComparisonGroups(runId, distance)
    ])
    if (runId !== currentRunId.value || distance !== appliedGroupingDistance.value) return

    stats.value = nextStats
    results.value = nextResults
    autoGroups.value = nextGroups
    groupingDataRevision.value += 1
    ensureSelectedGroup()
    void refreshGroupSimilarityStatuses()
  }

  async function refreshHistory() {
    isLoadingHistory.value = true
    try {
      historyRuns.value = await listComparisonRuns(20)
    } finally {
      isLoadingHistory.value = false
    }
  }

  async function initializeHistory() {
    if (hasInitializedHistory.value) return
    hasInitializedHistory.value = true

    await refreshHistory()
    if (currentRunId.value || historyRuns.value.length === 0) return

    const rememberedRunId = readRememberedRunId()
    const rememberedRun = rememberedRunId
      ? historyRuns.value.find((run) => run.run_id === rememberedRunId)
      : null
    const recentSuccessfulRun =
      historyRuns.value.find((run) => isSuccessfulTerminalStatus(run.status) && run.result_count > 0) ||
      historyRuns.value[0]

    const runToLoad = rememberedRun || recentSuccessfulRun
    if (runToLoad) {
      await loadHistoryRun(runToLoad.run_id)
    }
  }

  async function loadHistoryRun(runId: string) {
    cleanupRuntime()
    resetProgress()
    const loadGeneration = historyLoadGeneration

    isLoadingHistory.value = true
    errorMessage.value = ''
    currentRunId.value = runId
    rememberRunId(runId)

    try {
      const snapshot = await getRunStatus(runId)
      if (
        loadGeneration !== historyLoadGeneration ||
        runId !== currentRunId.value ||
        snapshot.run_id !== runId
      ) return

      applyRunStatus(snapshot)

      if (isTerminalStatus(snapshot.status)) {
        isRunning.value = false
        await applyTerminalSnapshot(snapshot)
        return
      }

      isRunning.value = true
      await setupRuntimeListeners()
      startStatusPolling()
    } catch (error) {
      if (loadGeneration !== historyLoadGeneration || runId !== currentRunId.value) return
      currentRunId.value = ''
      currentPhase.value = ''
      errorMessage.value = '加载历史记录失败'
      throw error
    } finally {
      if (loadGeneration === historyLoadGeneration) {
        isLoadingHistory.value = false
      }
    }
  }

  async function deleteHistoryRun(runId: string) {
    await deleteComparisonRun(runId)
    historyRuns.value = historyRuns.value.filter((run) => run.run_id !== runId)

    if (currentRunId.value === runId) {
      cleanupRuntime()
      resetProgress()
      currentRunId.value = ''
      currentPhase.value = ''
      isRunning.value = false
      errorMessage.value = ''
    }

    if (readRememberedRunId() === runId) {
      window.localStorage.removeItem(LAST_RUN_ID_KEY)
    }
  }

  function clearCurrentRunView() {
    cleanupRuntime()
    resetProgress()
    currentRunId.value = ''
    currentPhase.value = ''
    isRunning.value = false
    errorMessage.value = ''
  }

  function resetProgress() {
    historyLoadGeneration += 1
    groupingRefreshGeneration += 1
    isLoadingHistory.value = false
    isRefreshingGroups.value = false
    progressModel.phase = ''
    progressModel.totalFiles = 0
    progressModel.processedFiles = 0
    progressModel.currentFile = ''
    progressModel.percentage = 0
    stats.value = null
    results.value = []
    autoGroups.value = []
    selectedGroupIndex.value = null
    selectedMemberId.value = null
    selectedGroupIds.value = []
    checkedImageIds.value = []
    manualMergeSets.value = []
    groupingDistance.value = DEFAULT_GROUPING_DISTANCE
    appliedGroupingDistance.value = DEFAULT_GROUPING_DISTANCE
    groupingDataRevision.value = 0
    groupEditMode.value = false
    groupSimilarityStatuses.value = {}
    groupSimilarityStatusRequests.clear()
    clearGroupingRefreshTimer()
  }

  function ensureSelectedGroup() {
    if (groups.value.length === 0) {
      selectedGroupIndex.value = null
      selectedMemberId.value = null
      return
    }

    const currentGroup =
      selectedGroupIndex.value === null
        ? null
        : groups.value.find((group) => group.group_index === selectedGroupIndex.value)

    const group = currentGroup || groups.value[0]
    selectedGroupIndex.value = group.group_index

    if (!group.members.some((member) => member.image_id === selectedMemberId.value)) {
      selectedMemberId.value = group.members[0]?.image_id || null
    }
  }

  function reassignAliases() {
    directorySelection.directories.forEach((item, index) => {
      item.alias = String.fromCharCode(65 + index)
      item.name = getFolderName(item.path)
    })
  }

  function getFolderName(path: string): string {
    const normalizedPath = path.replace(/[\\/]+$/, '')
    const parts = normalizedPath.split(/[\\/]/).filter(Boolean)
    return parts[parts.length - 1] || normalizedPath || path
  }

  const hasInternalCompareDirectory = computed(() =>
    directorySelection.directories.some((item) => item.compareWithin)
  )

  const isSingleInternalComparison = computed(() =>
    directorySelection.directories.length === 1 && directorySelection.directories[0]?.compareWithin
  )

  const canStartComparison = computed(() =>
    directorySelection.directories.length >= 2 || hasInternalCompareDirectory.value
  )

  const selectedGroup = computed(() => {
    if (selectedGroupIndex.value === null) return groups.value[0] || null
    return groups.value.find((group) => group.group_index === selectedGroupIndex.value) || null
  })

  const selectedMember = computed(() => {
    const group = selectedGroup.value
    if (!group) return null

    return (
      group.members.find((member) => member.image_id === selectedMemberId.value) ||
      group.members[0] ||
      null
    )
  })

  function selectGroupMember(groupIndex: number, memberId: number) {
    selectedGroupIndex.value = groupIndex
    selectedMemberId.value = memberId
  }

  function selectGroup(groupIndex: number) {
    const group = groups.value.find((item) => item.group_index === groupIndex)
    if (!group) return

    selectedGroupIndex.value = group.group_index
    selectedMemberId.value = group.members[0]?.image_id || null
  }

  function groupSimilarityStatusKey(imageIds: number[]) {
    return [...imageIds].sort((left, right) => left - right).join(',')
  }

  function applyGroupSimilarityStatus(status: GroupSimilarityStatus) {
    if (
      status.run_id !== currentRunId.value
      || status.grouping_distance !== appliedGroupingDistance.value
    ) return

    const key = groupSimilarityStatusKey(status.image_ids)
    groupSimilarityStatuses.value = {
      ...groupSimilarityStatuses.value,
      [key]: status
    }
  }

  async function ensureGroupSimilarityStatusListener() {
    if (unlistenGroupSimilarityStatus) return
    if (!groupSimilarityStatusListenerPromise) {
      groupSimilarityStatusListenerPromise = listen<GroupSimilarityStatus>(
        'group-similarity-status',
        (event) => applyGroupSimilarityStatus(event.payload)
      )
        .then((unlisten) => {
          unlistenGroupSimilarityStatus = unlisten
        })
        .catch((error) => {
          console.warn('监听分组 SSIM 状态失败:', error)
        })
        .finally(() => {
          groupSimilarityStatusListenerPromise = null
        })
    }
    await groupSimilarityStatusListenerPromise
  }

  async function refreshGroupSimilarityStatuses() {
    const runId = currentRunId.value
    const distance = appliedGroupingDistance.value
    if (!runId) {
      groupSimilarityStatuses.value = {}
      return
    }

    // 同一个 (run, 宽松度) 已有在途请求时直接复用，避免重复触发后端的整批指纹扫描。
    const requestKey = `${runId}:${distance}`
    const inFlightRequest = groupSimilarityStatusRequests.get(requestKey)
    if (inFlightRequest) return inFlightRequest

    const request = (async () => {
      try {
        await ensureGroupSimilarityStatusListener()
        const statuses = await getGroupSimilarityStatuses(runId, distance)
        if (runId !== currentRunId.value || distance !== appliedGroupingDistance.value) return

        const nextStatuses: Record<string, GroupSimilarityStatus> = {}
        for (const status of statuses) {
          const key = groupSimilarityStatusKey(status.image_ids)
          const currentStatus = groupSimilarityStatuses.value[key]
          nextStatuses[key] = currentStatus?.status === 'running' && status.status === 'pending'
            ? currentStatus
            : status
        }
        groupSimilarityStatuses.value = nextStatuses
      } catch (error) {
        console.warn('刷新分组 SSIM 状态失败:', error)
      } finally {
        groupSimilarityStatusRequests.delete(requestKey)
      }
    })()

    groupSimilarityStatusRequests.set(requestKey, request)
    return request
  }

  function getGroupSimilarityStatus(group: ComparisonGroup): GroupSimilarityStatus {
    const imageIds = group.members.map((member) => member.image_id)
    const cachedStatus = groupSimilarityStatuses.value[groupSimilarityStatusKey(imageIds)]
    if (cachedStatus) return cachedStatus

    const onlyOneImage = imageIds.length < 2
    return {
      run_id: currentRunId.value,
      grouping_distance: appliedGroupingDistance.value,
      group_index: group.group_index,
      image_ids: [...imageIds].sort((left, right) => left - right),
      status: onlyOneImage ? 'completed' : 'pending',
      message: onlyOneImage
        ? '本组只有一张图片，无需进行组内 SSIM 比对'
        : '尚未比对，正在等待后台 SSIM 计算'
    }
  }

  function markGroupSimilarityStatus(
    group: ComparisonGroup,
    status: GroupSimilarityStatusValue,
    message: string
  ) {
    applyGroupSimilarityStatus({
      run_id: currentRunId.value,
      grouping_distance: appliedGroupingDistance.value,
      group_index: group.group_index,
      image_ids: group.members.map((member) => member.image_id),
      status,
      message
    })
  }

  function clearCheckedSelections() {
    selectedGroupIds.value = []
    checkedImageIds.value = []
  }

  function clearManualGroupingState() {
    manualMergeSets.value = []
    selectedGroupIds.value = []
  }

  function setGroupingDistance(nextDistance: number) {
    const clampedDistance = clampGroupingDistance(nextDistance)
    if (groupingDistance.value === clampedDistance) return

    groupingDistance.value = clampedDistance
    clearCheckedSelections()
    clearManualGroupingState()
    scheduleGroupingRefresh()
  }

  function scheduleGroupingRefresh() {
    clearGroupingRefreshTimer()
    isRefreshingGroups.value = true

    groupingRefreshTimer = window.setTimeout(() => {
      appliedGroupingDistance.value = groupingDistance.value
      void refreshGroups()
    }, 360)
  }

  function clearGroupingRefreshTimer() {
    if (groupingRefreshTimer !== null) {
      window.clearTimeout(groupingRefreshTimer)
      groupingRefreshTimer = null
    }
  }

  function mergeSelectedGroups() {
    const selectedGroups = groups.value.filter((group) =>
      selectedGroupIds.value.includes(group.group_index)
    )
    if (selectedGroups.length < 2) return null

    const sourceGroupIndices = Array.from(
      new Set(selectedGroups.flatMap((group) => getSourceGroupIndices(group)))
    ).sort((left, right) => left - right)
    const manualGroupId = `manual-${Date.now()}-${Math.random().toString(16).slice(2)}`

    manualMergeSets.value.push({
      id: manualGroupId,
      sourceGroupIndices
    })
    selectedGroupIds.value = []

    const mergedGroup = groups.value.find((group) => group.manual_group_id === manualGroupId)
    if (mergedGroup) {
      selectedGroupIndex.value = mergedGroup.group_index
      selectedMemberId.value = mergedGroup.members[0]?.image_id || null
    }

    return mergedGroup || null
  }

  function buildDisplayGroups(
    sourceGroups: ComparisonGroup[],
    mergeSets: ManualMergeSet[]
  ): ComparisonGroup[] {
    if (mergeSets.length === 0) {
      return sourceGroups.map((group) => ({
        ...group,
        manual_merged: false,
        source_group_indices: [group.group_index]
      }))
    }

    const mergedSourceIndices = new Set<number>()
    const mergedGroups = mergeSets
      .map((mergeSet) => {
        const groupsToMerge = sourceGroups.filter((group) =>
          mergeSet.sourceGroupIndices.includes(group.group_index)
        )
        if (groupsToMerge.length === 0) return null

        for (const group of groupsToMerge) mergedSourceIndices.add(group.group_index)
        return createManualMergedGroup(groupsToMerge, mergeSet)
      })
      .filter((group): group is ComparisonGroup => group !== null)

    const remainingGroups = sourceGroups
      .filter((group) => !mergedSourceIndices.has(group.group_index))
      .map((group) => ({
        ...group,
        manual_merged: false,
        source_group_indices: [group.group_index]
      }))

    return [...mergedGroups, ...remainingGroups]
      .sort((left, right) => {
        const leftFirst = getSourceGroupIndices(left)[0] || left.group_index
        const rightFirst = getSourceGroupIndices(right)[0] || right.group_index
        return leftFirst - rightFirst
      })
      .map((group, index) => ({
        ...group,
        group_index: index + 1
      }))
  }

  function createManualMergedGroup(
    groupsToMerge: ComparisonGroup[],
    mergeSet: ManualMergeSet
  ): ComparisonGroup {
    const memberById = new Map<number, ComparisonGroup['members'][number]>()
    for (const group of groupsToMerge) {
      for (const member of group.members) {
        if (!memberById.has(member.image_id)) memberById.set(member.image_id, member)
      }
    }

    const members = Array.from(memberById.values()).sort((left, right) =>
      left.ssim_cluster_key
        .localeCompare(right.ssim_cluster_key)
        || left.role.localeCompare(right.role)
        || left.relative_path.localeCompare(right.relative_path)
    )
    const representative = chooseRepresentativeMember(members)

    return {
      group_index: Math.min(...mergeSet.sourceGroupIndices),
      representative_image_id: representative?.image_id || members[0]?.image_id || 0,
      representative_file_name: fileNameFromPath(representative?.relative_path || members[0]?.relative_path || ''),
      member_count: members.length,
      has_low_quality_suggestion: members.some((member) => member.is_low_quality_suggestion),
      members,
      manual_merged: true,
      manual_group_id: mergeSet.id,
      source_group_indices: mergeSet.sourceGroupIndices
    }
  }

  function chooseRepresentativeMember(members: ComparisonGroup['members']) {
    return [...members].sort((left, right) => {
      const leftPixels = left.width * left.height
      const rightPixels = right.width * right.height
      return (
        rightPixels - leftPixels ||
        right.file_size - left.file_size ||
        left.relative_path.localeCompare(right.relative_path)
      )
    })[0] || null
  }

  function getSourceGroupIndices(group: ComparisonGroup) {
    return group.source_group_indices || [group.group_index]
  }

  function clampGroupingDistance(distance: number) {
    return Math.min(MAX_GROUPING_DISTANCE, Math.max(MIN_GROUPING_DISTANCE, Math.round(distance)))
  }

  function fileNameFromPath(path: string) {
    const parts = path.split(/[/\\]/).filter(Boolean)
    return parts[parts.length - 1] || path
  }

  async function setupRuntimeListeners() {
    cleanupEventListeners()
    bufferedProgress.clear()
    bufferedCompletions.clear()

    unlistenProgress = await listen<MultiCompareProgressEvent>('scan-progress', (event) => {
      const progress = event.payload

      if (!currentRunId.value) {
        bufferedProgress.set(progress.run_id, progress)
        return
      }

      applyProgress(progress)
    })

    unlistenComplete = await listen<string>('comparison-complete', (event) => {
      const runId = event.payload

      if (!currentRunId.value) {
        bufferedCompletions.add(runId)
        return
      }

      if (runId === currentRunId.value) {
        void reconcileRunStatus(runId)
      }
    })
  }

  function applyProgress(progress: MultiCompareProgressEvent) {
    if (progress.run_id !== currentRunId.value) return

    progressModel.phase = progress.phase
    progressModel.totalFiles = progress.total_files
    progressModel.processedFiles = progress.processed_files
    progressModel.currentFile = progress.current_file || ''
    progressModel.percentage = progressPercentage.value
    currentPhase.value = progress.phase
  }

  function startStatusPolling() {
    cleanupPolling()
    statusPollingTimer = window.setInterval(() => {
      void reconcileRunStatus()
    }, 1000)
    void reconcileRunStatus()
  }

  async function reconcileRunStatus(runId = currentRunId.value) {
    if (!runId || runId !== currentRunId.value) return

    try {
      const snapshot = await getRunStatus(runId)
      if (snapshot.run_id !== currentRunId.value) return

      applyRunStatus(snapshot)

      if (isTerminalStatus(snapshot.status)) {
        await finishRun(snapshot)
      }
    } catch (error) {
      console.warn('同步对比运行状态失败:', error)
    }
  }

  function applyRunStatus(snapshot: RunStatusResponse) {
    const statusPhaseMap: Partial<Record<RunStatus, string>> = {
      [RunStatus.Pending]: 'preflight',
      [RunStatus.Preflight]: 'preflight',
      [RunStatus.Indexing]: 'indexing',
      [RunStatus.Matching]: 'matching',
      [RunStatus.Scoring]: 'scoring',
      [RunStatus.Resolving]: 'resolving',
      [RunStatus.ReviewPending]: 'complete',
      [RunStatus.AnalysisComplete]: 'complete',
      [RunStatus.ActionComplete]: 'complete',
      [RunStatus.CompletedWithErrors]: 'complete',
      [RunStatus.Failed]: 'failed',
      [RunStatus.Canceled]: 'canceled',
      [RunStatus.Paused]: 'paused'
    }

    currentPhase.value = statusPhaseMap[snapshot.status] || snapshot.status
  }

  async function finishRun(snapshot: RunStatusResponse) {
    cleanupRuntime()
    isRunning.value = false
    await applyTerminalSnapshot(snapshot)
    await refreshHistory()
  }

  async function applyTerminalSnapshot(snapshot: RunStatusResponse) {
    if (isSuccessfulTerminalStatus(snapshot.status)) {
      progressModel.phase = 'complete'
      currentPhase.value = 'complete'
      if (progressModel.totalFiles > 0) {
        progressModel.processedFiles = progressModel.totalFiles
        progressModel.percentage = 100
      }

      await refreshAnalysisData()
      return
    }

    errorMessage.value =
      snapshot.status === RunStatus.Canceled ? '对比任务已取消' : '对比任务执行失败'
  }

  function isSuccessfulTerminalStatus(status: RunStatus) {
    return [
      RunStatus.ReviewPending,
      RunStatus.AnalysisComplete,
      RunStatus.ActionComplete,
      RunStatus.CompletedWithErrors
    ].includes(status)
  }

  function isTerminalStatus(status: RunStatus) {
    return [
      RunStatus.ReviewPending,
      RunStatus.AnalysisComplete,
      RunStatus.ActionComplete,
      RunStatus.CompletedWithErrors,
      RunStatus.Canceled,
      RunStatus.Failed
    ].includes(status)
  }

  function cleanupRuntime() {
    cleanupPolling()
    cleanupEventListeners()
    clearGroupingRefreshTimer()
  }

  function cleanupPolling() {
    if (statusPollingTimer !== null) {
      window.clearInterval(statusPollingTimer)
      statusPollingTimer = null
    }
  }

  function cleanupEventListeners() {
    if (unlistenProgress) {
      unlistenProgress()
      unlistenProgress = null
    }

    if (unlistenComplete) {
      unlistenComplete()
      unlistenComplete = null
    }
  }

  function rememberRunId(runId: string) {
    window.localStorage.setItem(LAST_RUN_ID_KEY, runId)
  }

  function readRememberedRunId() {
    return window.localStorage.getItem(LAST_RUN_ID_KEY)
  }

  // 获取阶段名称（中文）
  function getPhaseName(phase: string): string {
    const phaseMap: Record<string, string> = {
      pending: '准备中',
      preflight: '预检查',
      indexing: '索引构建',
      matching: '匹配计算',
      candidate_search: '候选筛选',
      scoring: '评分分析',
      resolving: '结果解析',
      complete: '完成',
      paused: '已暂停',
      canceled: '已取消',
      failed: '失败'
    }
    return phaseMap[phase] || phase
  }

  return {
    // 状态
    directorySelection,
    currentRunId,
    currentPhase,
    isRunning,
    errorMessage,
    historyRuns,
    isLoadingHistory,
    progressModel,
    stats,
    results,
    groups,
    selectedGroupIndex,
    selectedMemberId,
    selectedGroupIds,
    checkedImageIds,
    groupingDistance,
    appliedGroupingDistance,
    isRefreshingGroups,
    groupingDataRevision,
    groupEditMode,
    originalRecognitionThreshold,
    groupSimilarityStatuses,

    // 计算属性
    progressPercentage,
    categoryStats,
    conservationCheck,
    hasAnyCheckedSelection,
    hasInternalCompareDirectory,
    isSingleInternalComparison,
    canStartComparison,
    selectedGroup,
    selectedMember,

    // 方法
    addDirectory,
    addDirectories,
    removeDirectory,
    clearSelection,
    startComparison,
    refreshStats,
    refreshResults,
    refreshGroups,
    refreshAnalysisData,
    refreshHistory,
    initializeHistory,
    loadHistoryRun,
    deleteHistoryRun,
    clearCurrentRunView,
    selectGroup,
    selectGroupMember,
    getGroupSimilarityStatus,
    markGroupSimilarityStatus,
    clearCheckedSelections,
    clearManualGroupingState,
    setGroupingDistance,
    mergeSelectedGroups,
    getPhaseName
  }
})
