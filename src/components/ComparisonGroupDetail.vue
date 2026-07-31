<template>
  <div class="group-detail">
    <el-card v-if="group" shadow="never" class="detail-card">
      <template #header>
        <div class="detail-header">
          <div class="detail-title-block">
            <div class="header-title">第 {{ group.group_index }} 组</div>
            <div class="threshold-summary">
              原图拆分阈值 {{ formatSsimThreshold(originalRecognitionThreshold) }}
            </div>
          </div>
          <div class="detail-actions">
            <el-button
              :icon="CopyDocument"
              plain
              size="small"
              data-test="copy-checked-names"
              :disabled="isLoadingCrossCheck || checkedDeleteCandidates.length === 0"
              @click="copyCheckedDeleteFileNames"
            >
              复制已选文件名
            </el-button>

            <el-popover
              v-model:visible="thresholdPopoverVisible"
              trigger="click"
              placement="bottom-end"
              :width="440"
            >
              <template #reference>
                <el-button :icon="Setting" plain size="small">
                  判断阈值
                </el-button>
              </template>

              <div class="quality-control">
                <div class="quality-control-card">
                  <div class="quality-heading">
                    <span>原图拆分阈值</span>
                    <span class="quality-value">{{ formatSsimThreshold(originalRecognitionThreshold) }}</span>
                  </div>
                  <el-slider
                    class="quality-slider"
                    :model-value="originalRecognitionSliderValue"
                    :min="0"
                    :max="140"
                    :step="1"
                    :marks="originalRecognitionMarks"
                    :format-tooltip="formatRecognitionSliderTooltip"
                    @input="handleOriginalRecognitionInput"
                  />
                  <div class="quality-help">
                    系统先筛选总像素至少达到组内最大图 90%、且画面比例接近的图片，再检查任务记录的标准 SSIM。达到当前阈值时会单独列为原图。调低会拆出更多原图，调高会减少原图数量；组内代表图和系统参考图始终保留为原图。
                  </div>
                </div>

                <div class="quality-help">
                  此阈值只改变原图和缩略图的归属，不改变标准 SSIM 算法、已计算数值或复选框状态。组内 SSIM 固定使用完整较小分辨率，最多 4 路并行计算。
                </div>
              </div>
            </el-popover>

            <el-button
              plain
              size="small"
              :type="hasManualAssignmentChanges ? 'warning' : ''"
              :class="{ 'recalculate-needed': hasManualAssignmentChanges }"
              :loading="isLoadingCrossCheck"
              @click="recalculateImageAssignment"
            >
              重新计算图片归属
            </el-button>

            <el-button
              type="danger"
              plain
              size="small"
              :disabled="isLoadingCrossCheck || isRecycling || store.checkedImageIds.length === 0"
              :loading="isRecycling"
              @click="confirmRecycleSelected"
            >
              删除所选
            </el-button>
          </div>
        </div>
      </template>

      <div v-if="hasManualAssignmentChanges" class="assignment-notice">
        已手动调整原图/缩略图，建议重新计算组内图片归属。
      </div>

      <div v-if="isLoadingCrossCheck" class="cross-check-loading">
        <el-icon class="cross-check-spinner"><Loading /></el-icon>
        <div class="cross-check-loading-title">{{ crossCheckProgressTitle }}</div>
        <div v-if="crossCheckProgressText" class="cross-check-current">
          {{ crossCheckProgressText }}
        </div>
        <el-progress
          v-if="crossCheckProgress && crossCheckProgressTotal > 0"
          class="cross-check-progress"
          :percentage="crossCheckProgressPercent"
          :stroke-width="8"
          :show-text="false"
        />
        <div v-if="crossCheckProgress" class="cross-check-meta">
          {{ crossCheckProgressMeta }}
          <span v-if="crossCheckProgress.phase === 'caching' && crossCheckProgress.image_cache_hits > 0">
            · 图片缓存命中 {{ crossCheckProgress.image_cache_hits }}
          </span>
          <span v-if="crossCheckProgress.skipped_pairs > 0">
            · 已跳过 {{ crossCheckProgress.skipped_pairs }} 个无意义组合
          </span>
          <span v-if="crossCheckProgress.phase !== 'caching' && crossCheckProgress.cache_hits > 0">
            · 缓存命中 {{ crossCheckProgress.cache_hits }}
          </span>
        </div>
      </div>

      <div
        v-if="!isLoadingCrossCheck && hiddenOriginalRowCount > 0"
        class="detail-filter-bar"
      >
        <span>已隐藏 {{ hiddenOriginalRowCount }} 张暂无缩略图的原图</span>
        <el-switch
          v-model="showEmptyOriginalRows"
          size="small"
          active-text="显示无候选原图"
        />
      </div>

      <el-empty
        v-if="!isLoadingCrossCheck && visibleOriginalRows.length === 0"
        class="no-candidate-empty"
        description="本组暂无缩略图"
        :image-size="72"
      />

      <el-table
        v-else-if="!isLoadingCrossCheck"
        :data="visibleOriginalRows"
        row-key="id"
        height="100%"
        class="detail-table"
        :expand-row-keys="defaultExpandedOriginalRowKeys"
        :row-class-name="getOriginalRowClassName"
        @row-click="handleOriginalRowClick"
      >
        <el-table-column type="expand" width="58">
          <template #header>
            <el-tooltip content="全选本组全部缩略图" placement="top">
              <el-checkbox
                data-test="select-all-delete"
                :model-value="allDeleteCandidatesChecked"
                :indeterminate="someDeleteCandidatesChecked"
                :disabled="thumbnailCandidates.length === 0"
                aria-label="全选本组全部缩略图"
                @change="handleSelectAllDeleteChange"
                @click.stop
              />
            </el-tooltip>
          </template>
          <template #default="{ row }">
            <div class="thumbnail-panel">
              <el-empty
                v-if="row.candidates.length === 0"
                description="这张原图下暂无缩略图"
                :image-size="52"
              />
              <el-table
                v-else
                :data="row.candidates"
                row-key="id"
                size="small"
                class="thumbnail-table"
                :row-class-name="getCandidateRowClassName"
                @row-click="handleCandidateRowClick"
              >
                <el-table-column label="删除" width="58" align="center">
                  <template #default="{ row: candidate }">
                    <el-checkbox
                      :model-value="isCheckedForDelete(candidate.member.image_id)"
                      @change="handleCandidateDeleteChange(candidate, $event)"
                      @click.stop
                    >
                      <span class="sr-only">选择删除</span>
                    </el-checkbox>
                  </template>
                </el-table-column>

                <el-table-column label="图片" min-width="180">
                  <template #default="{ row: candidate }">
                    <div class="image-cell">
                      <el-tooltip
                        placement="right-start"
                        :show-after="200"
                        popper-class="detail-image-preview-tooltip"
                      >
                        <template #content>
                          <div class="detail-image-preview-tooltip-content">
                            <img
                              class="detail-image-preview-large"
                              :src="getImageUrl(candidate.member.file_path)"
                              :alt="candidate.member.relative_path"
                            />
                          </div>
                        </template>
                        <img
                          class="thumb small"
                          :class="{ active: candidate.member.image_id === store.selectedMemberId }"
                          :src="getImageUrl(candidate.member.file_path)"
                          :alt="candidate.member.relative_path"
                          loading="lazy"
                          @click.stop="openImageViewer(candidate.member.image_id)"
                        />
                      </el-tooltip>
                      <el-tooltip :content="getFileName(candidate.member)" placement="right-start">
                        <el-button
                          class="file-name-button"
                          type="primary"
                          link
                          @click.stop="copyFileName(candidate.member)"
                        >
                          {{ getFileName(candidate.member) }}
                        </el-button>
                      </el-tooltip>
                    </div>
                  </template>
                </el-table-column>

                <el-table-column label="判断" width="150" align="center">
                  <template #default>
                    <el-tag type="info" size="small" effect="light">缩略图</el-tag>
                  </template>
                </el-table-column>

                <el-table-column label="分辨率 / 大小" width="162" align="center">
                  <template #default="{ row: candidate }">
                    <span class="metric-inline">{{ formatImageMetrics(candidate.member) }}</span>
                  </template>
                </el-table-column>

                <el-table-column width="118" align="center">
                  <template #header>
                    <el-tooltip :content="candidateSsimColumnHelp" placement="top">
                      <span
                        class="column-header-help"
                        tabindex="0"
                        aria-label="与原图 SSIM 说明"
                      >
                        与原图 SSIM
                        <el-icon><QuestionFilled /></el-icon>
                      </span>
                    </el-tooltip>
                  </template>
                  <template #default="{ row: candidate }">
                    <span>{{ formatSimilarity(candidate.similarity) }}</span>
                  </template>
                </el-table-column>

                <el-table-column label="图片操作" width="104" align="center">
                  <template #default="{ row: candidate }">
                    <el-button
                      class="row-action-button"
                      type="primary"
                      plain
                      size="small"
                      @click.stop="markAsOriginal(candidate.member.image_id)"
                    >
                      设为原图
                    </el-button>
                  </template>
                </el-table-column>
                <el-table-column label="路径操作" width="136" align="center">
                  <template #default="{ row: candidate }">
                    <div class="path-actions">
                      <el-button
                        class="compact-action-button"
                        type="primary"
                        link
                        title="复制当前路径"
                        @click.stop="copyFolderPath(candidate.member)"
                      >
                        复制
                      </el-button>
                      <el-button
                        class="compact-action-button"
                        type="primary"
                        link
                        title="使用资源管理器打开"
                        @click.stop="openFolder(candidate.member)"
                      >
                        打开
                      </el-button>
                      <el-tooltip
                        :content="candidate.member.file_path"
                        placement="top-end"
                        :show-after="160"
                      >
                        <el-button
                          class="compact-action-button"
                          type="primary"
                          link
                          data-test="view-full-path"
                          @click.stop="showFullPath(candidate.member)"
                        >
                          查看
                        </el-button>
                      </el-tooltip>
                    </div>
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="图片" min-width="190">
          <template #default="{ row }">
            <div class="image-cell">
              <el-tooltip
                placement="right-start"
                :show-after="200"
                popper-class="detail-image-preview-tooltip"
              >
                <template #content>
                  <div class="detail-image-preview-tooltip-content">
                    <img
                      class="detail-image-preview-large"
                      :src="getImageUrl(row.member.file_path)"
                      :alt="row.member.relative_path"
                    />
                  </div>
                </template>
                <img
                  class="thumb"
                  :class="{ active: row.member.image_id === store.selectedMemberId }"
                  :src="getImageUrl(row.member.file_path)"
                  :alt="row.member.relative_path"
                  loading="lazy"
                  @click.stop="openImageViewer(row.member.image_id)"
                />
              </el-tooltip>
              <el-tooltip :content="getFileName(row.member)" placement="right-start">
                <el-button
                  class="file-name-button"
                  type="primary"
                  link
                  @click.stop="copyFileName(row.member)"
                >
                  {{ getFileName(row.member) }}
                </el-button>
              </el-tooltip>
            </div>
          </template>
        </el-table-column>

        <el-table-column width="150" align="center">
          <template #header>
            <el-tooltip :content="originalBasisColumnHelp" placement="top">
              <span
                class="column-header-help"
                tabindex="0"
                aria-label="原图依据说明"
              >
                原图依据
                <el-icon><QuestionFilled /></el-icon>
              </span>
            </el-tooltip>
          </template>
          <template #default="{ row }">
            <el-tooltip :content="row.reason" placement="right-start">
              <el-tag :type="row.source === 'manual' ? 'primary' : 'success'" size="small" effect="light">
                {{ row.source === 'manual' ? '手动原图' : '识别为原图' }}
              </el-tag>
            </el-tooltip>
          </template>
        </el-table-column>

        <el-table-column label="分辨率 / 大小" width="162" align="center">
          <template #default="{ row }">
            <span class="metric-inline">{{ formatImageMetrics(row.member) }}</span>
          </template>
        </el-table-column>

        <el-table-column label="缩略图" width="96" align="center">
          <template #default="{ row }">
            <span>{{ row.candidates.length }}</span>
          </template>
        </el-table-column>

        <el-table-column label="图片操作" width="110" align="center">
          <template #default="{ row }">
            <el-button
              class="row-action-button"
              type="warning"
              plain
              size="small"
              @click.stop="markAsThumbnail(row.member.image_id)"
            >
              设为缩略图
            </el-button>
          </template>
        </el-table-column>
        <el-table-column label="路径操作" width="136" align="center">
          <template #default="{ row }">
            <div class="path-actions">
              <el-button
                class="compact-action-button"
                type="primary"
                link
                title="复制当前路径"
                @click.stop="copyFolderPath(row.member)"
              >
                复制
              </el-button>
              <el-button
                class="compact-action-button"
                type="primary"
                link
                title="使用资源管理器打开"
                @click.stop="openFolder(row.member)"
              >
                打开
              </el-button>
              <el-tooltip
                :content="row.member.file_path"
                placement="top-end"
                :show-after="160"
              >
                <el-button
                  class="compact-action-button"
                  type="primary"
                  link
                  data-test="view-full-path"
                  @click.stop="showFullPath(row.member)"
                >
                  查看
                </el-button>
              </el-tooltip>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <div v-else class="empty-state">
      <el-empty description="请选择一个分组" />
    </div>

    <el-dialog
      v-model="viewerVisible"
      title="图片组预览"
      width="80%"
      align-center
      class="image-viewer-dialog"
      @closed="resetImageViewerZoom"
    >
      <div v-if="viewerMember" class="viewer">
        <div class="viewer-toolbar">
          <el-button @click="showPreviousImage" :disabled="viewerIndex <= 0">上一张</el-button>
          <span>{{ viewerIndex + 1 }} / {{ group?.members.length || 0 }}</span>
          <span class="viewer-zoom-label">{{ Math.round(viewerZoom * 100) }}%</span>
          <el-button @click="resetImageViewerZoom">重置</el-button>
          <el-button @click="showNextImage" :disabled="!group || viewerIndex >= group.members.length - 1">
            下一张
          </el-button>
        </div>
        <div class="viewer-stage" @wheel.prevent="handleViewerWheel">
          <img
            class="viewer-image"
            :style="{ transform: `scale(${viewerZoom})` }"
            :src="getImageUrl(viewerMember.file_path)"
            :alt="viewerMember.relative_path"
            draggable="false"
          />
        </div>
        <div class="viewer-caption">{{ viewerMember.relative_path }}</div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { CopyDocument, Loading, QuestionFilled, Setting } from '@element-plus/icons-vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  batchRecycleImages,
  cancelGroupSimilarityRequest,
  getGroupSimilarityScores,
  startGroupSimilarityBackfill
} from '@/api/comparison'
import { useComparisonStore } from '@/stores/comparisonStore'
import type {
  ComparisonGroup,
  ComparisonGroupMember,
  GroupSimilarityProgress,
  GroupSimilarityScore
} from '@/types'
import {
  formatSsim,
  ORIGINAL_RECOGNITION_THRESHOLD_KEY,
  precisionSliderValueToThreshold,
  precisionThresholdToSliderValue,
  readStoredRecognitionThreshold
} from '@/features/similarity'
import { getAutomaticOriginalImageIds } from '@/features/groupThumbnails'

const store = useComparisonStore()
const candidateSsimColumnHelp = '候选图与当前所在行原图的标准 SSIM，用于确认它被归到哪张原图下。'
const originalBasisColumnHelp = '说明图片为何被列为原图。自动识别会综合分辨率、画面比例和原图拆分阈值；手动设置优先。'
type OriginalRowSource = 'auto' | 'manual' | 'fallback'

interface OriginalRow {
  id: string
  member: ComparisonGroupMember
  source: OriginalRowSource
  reason: string
  candidates: ThumbnailCandidate[]
}

interface ThumbnailCandidate {
  id: string
  member: ComparisonGroupMember
  reference: ComparisonGroupMember
  similarity?: number | null
}

const viewerVisible = ref(false)
const viewerIndex = ref(0)
const viewerZoom = ref(1)
const thresholdPopoverVisible = ref(false)
const isRecycling = ref(false)
const originalRecognitionThreshold = ref(readStoredRecognitionThreshold(window.localStorage))
const manualOriginalIds = ref<number[]>([])
const manualThumbnailIds = ref<number[]>([])
const showEmptyOriginalRows = ref(false)
const groupSimilarityScores = ref<GroupSimilarityScore[]>([])
const crossCheckProgress = ref<GroupSimilarityProgress | null>(null)
const isLoadingCrossCheck = ref(false)
const hasManualAssignmentChanges = ref(false)
let crossCheckRequestId = 0
let activeCrossCheckRequestKey = ''
let activeCrossCheckGroup: ComparisonGroup | null = null
let activeCrossCheckRunId = ''
let activeCrossCheckDistance = 0
let lastLoadedGroupKey = ''
let unlistenGroupSimilarityProgress: UnlistenFn | null = null
let groupSimilarityProgressListenerPromise: Promise<void> | null = null

const group = computed(() => store.selectedGroup)
const originalRecognitionMarks = {
  0: '0.95',
  30: '0.98',
  40: '0.99',
  140: '1.00'
}

const originalRecognitionSliderValue = computed(() =>
  precisionThresholdToSliderValue(originalRecognitionThreshold.value)
)

const groupKey = computed(() =>
  [
    store.currentRunId,
    store.groupingDataRevision,
    group.value?.group_index || '',
    group.value?.source_group_indices?.join(',') || '',
    group.value?.members.map((member) => member.image_id).join(',') || ''
  ].join(':')
)

const originalRows = computed(() => buildOriginalRows(group.value?.members || []))
const visibleOriginalRows = computed(() =>
  showEmptyOriginalRows.value
    ? originalRows.value
    : originalRows.value.filter((row) => row.candidates.length > 0)
)
const hiddenOriginalRowCount = computed(() =>
  originalRows.value.filter((row) => row.candidates.length === 0).length
)
const defaultExpandedOriginalRowKeys = computed(() =>
  visibleOriginalRows.value
    .filter((row) => row.candidates.length > 0)
    .map((row) => row.id)
)
const thumbnailCandidates = computed(() => originalRows.value.flatMap((row) => row.candidates))
const checkedDeleteCandidates = computed(() =>
  thumbnailCandidates.value.filter((candidate) =>
    store.checkedImageIds.includes(candidate.member.image_id)
  )
)
const allDeleteCandidatesChecked = computed(() =>
  thumbnailCandidates.value.length > 0
  && thumbnailCandidates.value.every((candidate) =>
    store.checkedImageIds.includes(candidate.member.image_id)
  )
)
const someDeleteCandidatesChecked = computed(() =>
  thumbnailCandidates.value.some((candidate) =>
    store.checkedImageIds.includes(candidate.member.image_id)
  ) && !allDeleteCandidatesChecked.value
)

const crossCheckProgressTotal = computed(() => {
  if (!crossCheckProgress.value) return 0
  if (crossCheckProgress.value.phase === 'caching') return crossCheckProgress.value.total_images
  return crossCheckProgress.value.total_pairs
})

const crossCheckProgressDone = computed(() => {
  if (!crossCheckProgress.value) return 0
  if (crossCheckProgress.value.phase === 'caching') return crossCheckProgress.value.processed_images
  return crossCheckProgress.value.processed_pairs
})

const crossCheckProgressPercent = computed(() => {
  if (!crossCheckProgress.value || crossCheckProgressTotal.value <= 0) return 0
  return Math.min(
    100,
    Math.round((crossCheckProgressDone.value / crossCheckProgressTotal.value) * 100)
  )
})

const crossCheckProgressTitle = computed(() => {
  const phase = crossCheckProgress.value?.phase
  if (phase === 'caching') return '正在建立图片缓存...'
  if (phase === 'comparing') return '正在交叉验证组内图片归属...'
  if (phase === 'completed') return '正在整理结果...'
  return '正在准备组内比对...'
})

const crossCheckProgressText = computed(() => {
  const progress = crossCheckProgress.value
  if (!progress) return ''
  if (progress.phase === 'caching') {
    const imageName = progress.current_image_file_name
    return imageName ? `正在处理图片：${imageName}` : '正在准备图片缓存...'
  }
  const leftName = progress.current_left_file_name
  const rightName = progress.current_right_file_name
  if (!leftName || !rightName) return '正在准备组内比对...'
  return `正在比对：${leftName} ↔ ${rightName}`
})

const crossCheckProgressMeta = computed(() => {
  const progress = crossCheckProgress.value
  if (!progress) return ''
  if (progress.phase === 'caching') {
    return `图片缓存 ${progress.processed_images} / ${progress.total_images}`
  }
  return `比对进度 ${progress.processed_pairs} / ${progress.total_pairs}`
})

const similarityScoreMap = computed(() => {
  const scoreMap = new Map<string, number>()
  for (const score of groupSimilarityScores.value) {
    if (typeof score.ssim_score !== 'number') continue
    scoreMap.set(getSimilarityKey(score.left_image_id, score.right_image_id), score.ssim_score)
  }
  return scoreMap
})

const viewerMember = computed(() => {
  const members = group.value?.members || []
  return members[viewerIndex.value] || null
})

watch(
  [groupKey, () => store.isRefreshingGroups],
  ([nextGroupKey, isRefreshing]) => {
    if (isRefreshing) {
      crossCheckRequestId += 1
      cancelActiveCrossCheckRequest()
      isLoadingCrossCheck.value = false
      return
    }
    if (nextGroupKey === lastLoadedGroupKey) return
    lastLoadedGroupKey = nextGroupKey
    viewerIndex.value = 0
    manualOriginalIds.value = []
    manualThumbnailIds.value = []
    showEmptyOriginalRows.value = false
    hasManualAssignmentChanges.value = false
    void loadGroupCrossCheckScores()
  },
  { immediate: true }
)

watch(
  () => thumbnailCandidates.value.map((candidate) => candidate.member.image_id).sort((a, b) => a - b).join(','),
  () => {
    const currentMemberIds = new Set(group.value?.members.map((member) => member.image_id) || [])
    const currentThumbnailIds = new Set(
      thumbnailCandidates.value.map((candidate) => candidate.member.image_id)
    )
    store.checkedImageIds = store.checkedImageIds.filter(
      (imageId) => !currentMemberIds.has(imageId) || currentThumbnailIds.has(imageId)
    )
  },
  { immediate: true }
)

onBeforeUnmount(() => {
  crossCheckRequestId += 1
  cancelActiveCrossCheckRequest()
  if (unlistenGroupSimilarityProgress) {
    unlistenGroupSimilarityProgress()
    unlistenGroupSimilarityProgress = null
  }
})

async function ensureGroupSimilarityProgressListener() {
  if (unlistenGroupSimilarityProgress) return
  if (!groupSimilarityProgressListenerPromise) {
    groupSimilarityProgressListenerPromise = listen<GroupSimilarityProgress>(
      'group-similarity-progress',
      (event) => {
        const progress = event.payload
        if (progress.request_id !== activeCrossCheckRequestKey) return
        crossCheckProgress.value = progress
      }
    )
      .then((unlisten) => {
        unlistenGroupSimilarityProgress = unlisten
      })
      .catch((error) => {
        console.warn('监听组内交叉验证进度失败:', error)
      })
      .finally(() => {
        groupSimilarityProgressListenerPromise = null
      })
  }
  await groupSimilarityProgressListenerPromise
}

function handleOriginalRowClick(row: OriginalRow) {
  selectMember(row.member)
}

function handleCandidateRowClick(candidate: ThumbnailCandidate) {
  selectMember(candidate.member)
}

function selectMember(member: ComparisonGroupMember) {
  if (!group.value) return
  store.selectGroupMember(group.value.group_index, member.image_id)
}

function getOriginalRowClassName({ row }: { row: OriginalRow }) {
  const classes: string[] = ['original-row']
  if (row.member.image_id === store.selectedMemberId) classes.push('active-member-row')
  return classes.join(' ')
}

function getCandidateRowClassName({ row }: { row: ThumbnailCandidate }) {
  const classes: string[] = ['candidate-row']
  if (row.member.image_id === store.selectedMemberId) classes.push('active-member-row')
  return classes.join(' ')
}

function getImageUrl(path: string) {
  return convertFileSrc(path)
}

function getFileName(member: ComparisonGroupMember) {
  return fileNameFromPath(member.relative_path || member.file_path)
}

function fileNameFromPath(path: string) {
  const normalizedPath = path.replace(/[\\/]+$/, '')
  const parts = normalizedPath.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] || normalizedPath || path
}

function getFolderPath(member: ComparisonGroupMember) {
  const normalizedPath = member.file_path.replace(/[\\/]+$/, '')
  const lastSlashIndex = Math.max(normalizedPath.lastIndexOf('/'), normalizedPath.lastIndexOf('\\'))
  if (lastSlashIndex <= 0) return normalizedPath
  return normalizedPath.slice(0, lastSlashIndex)
}

async function copyText(text: string, successMessage: string) {
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success(successMessage)
  } catch (error) {
    console.error('复制失败:', error)
    ElMessage.error('复制失败')
  }
}

function copyFileName(member: ComparisonGroupMember) {
  void copyText(getFileName(member), '已复制文件名')
}

function copyFolderPath(member: ComparisonGroupMember) {
  void copyText(getFolderPath(member), '已复制文件夹路径')
}

function copyCheckedDeleteFileNames() {
  const fileNames = checkedDeleteCandidates.value.map((candidate) => getFileName(candidate.member))
  if (fileNames.length === 0) return
  void copyText(fileNames.join(','), `已复制 ${fileNames.length} 个文件名`)
}

async function openFolder(member: ComparisonGroupMember) {
  try {
    await invoke('open_folder', { path: getFolderPath(member) })
  } catch (error: any) {
    ElMessage.error(error || '打开文件夹失败')
  }
}

function showFullPath(member: ComparisonGroupMember) {
  void ElMessageBox.alert(member.file_path, '完整路径', {
    confirmButtonText: '关闭'
  })
}

function formatSimilarity(value?: number | null) {
  if (value === undefined || value === null) return '—'
  return formatSsim(value)
}

function formatSsimThreshold(value: number) {
  return value.toFixed(4)
}

function formatRecognitionSliderTooltip(value: number) {
  return formatSsimThreshold(precisionSliderValueToThreshold(value))
}

function handleOriginalRecognitionInput(value: number | number[]) {
  const nextValue = Array.isArray(value) ? value[0] : value
  originalRecognitionThreshold.value = precisionSliderValueToThreshold(nextValue)
  rememberOriginalRecognitionThreshold(originalRecognitionThreshold.value)
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

function formatImageMetrics(member: ComparisonGroupMember) {
  return `${member.width}×${member.height} · ${formatFileSize(member.file_size)}`
}

async function confirmRecycleSelected() {
  if (!store.currentRunId) {
    ElMessage.error('当前没有可操作的任务')
    return
  }

  const imageIds = [...store.checkedImageIds]
  if (imageIds.length === 0) return

  try {
    await ElMessageBox.confirm(
      `将 ${imageIds.length} 张图片移入 Windows 系统回收站，之后可在 Windows 回收站中恢复或永久删除。是否继续？`,
      '删除所选图片',
      {
        confirmButtonText: '移动到回收站',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
  } catch (error) {
    if (error === 'cancel' || error === 'close') return
    throw error
  }

  isRecycling.value = true
  try {
    const outcomes = await batchRecycleImages(store.currentRunId, imageIds)
    const successImageIds = outcomes
      .filter((outcome) => outcome.success)
      .map((outcome) => outcome.image_id)
    const failedOutcomes = outcomes.filter((outcome) => !outcome.success)

    if (successImageIds.length > 0) {
      store.checkedImageIds = store.checkedImageIds.filter((imageId) => !successImageIds.includes(imageId))
      await store.refreshAnalysisData()
    }

    if (failedOutcomes.length > 0) {
      const firstError = failedOutcomes[0]?.error_message || '部分图片移动失败'
      ElMessage.warning(`已移入系统回收站 ${successImageIds.length} 张，失败 ${failedOutcomes.length} 张：${firstError}`)
      return
    }

    ElMessage.success(`已移入 Windows 系统回收站 ${successImageIds.length} 张图片`)
  } catch (error: any) {
    ElMessage.error(error?.message || '移动到回收站失败')
  } finally {
    isRecycling.value = false
  }
}

function rememberOriginalRecognitionThreshold(value: number) {
  window.localStorage.setItem(ORIGINAL_RECOGNITION_THRESHOLD_KEY, value.toFixed(4))
}

async function loadGroupCrossCheckScores() {
  cancelActiveCrossCheckRequest()
  const requestId = ++crossCheckRequestId
  const currentGroup = group.value
  groupSimilarityScores.value = []
  crossCheckProgress.value = null

  if (!currentGroup || !store.currentRunId) {
    isLoadingCrossCheck.value = false
    return
  }
  if (currentGroup.members.length < 2) {
    isLoadingCrossCheck.value = false
    store.markGroupSimilarityStatus(
      currentGroup,
      'completed',
      '本组只有一张图片，无需进行组内 SSIM 比对'
    )
    scheduleGroupSimilarityBackfill(currentGroup)
    return
  }

  const requestKey = `group-${requestId}-${Date.now()}`
  const runId = store.currentRunId
  const groupingDistance = store.appliedGroupingDistance
  activeCrossCheckRequestKey = requestKey
  activeCrossCheckGroup = currentGroup
  activeCrossCheckRunId = runId
  activeCrossCheckDistance = groupingDistance
  isLoadingCrossCheck.value = true
  store.markGroupSimilarityStatus(currentGroup, 'running', '正在优先比对当前分组 SSIM')
  try {
    await ensureGroupSimilarityProgressListener()
    if (requestId !== crossCheckRequestId) return
    const scores = await getGroupSimilarityScores(
      runId,
      currentGroup.members.map((member) => member.image_id),
      requestKey,
      groupingDistance,
      currentGroup.group_index
    )
    if (requestId === crossCheckRequestId) {
      groupSimilarityScores.value = scores
      const completed = scores.every(
        (score) => typeof score.ssim_score === 'number' && !score.error_message
      )
      store.markGroupSimilarityStatus(
        currentGroup,
        completed ? 'completed' : 'pending',
        completed
          ? '组内 SSIM 已比对完成并缓存'
          : '组内 SSIM 比对未全部完成，正在等待重试'
      )
      scheduleGroupSimilarityBackfill(currentGroup)
    }
  } catch (error) {
    if (requestId === crossCheckRequestId) {
      store.markGroupSimilarityStatus(
        currentGroup,
        'pending',
        '组内 SSIM 比对失败，正在等待重试'
      )
      console.warn('组内交叉相似度计算失败:', error)
      ElMessage.warning('组内交叉验证失败，暂时使用已有参考关系')
    }
  } finally {
    if (requestId === crossCheckRequestId) {
      activeCrossCheckRequestKey = ''
      activeCrossCheckGroup = null
      activeCrossCheckRunId = ''
      activeCrossCheckDistance = 0
      isLoadingCrossCheck.value = false
    }
  }
}

function cancelActiveCrossCheckRequest() {
  const requestKey = activeCrossCheckRequestKey
  if (!requestKey) return

  const requestGroup = activeCrossCheckGroup
  const requestRunId = activeCrossCheckRunId
  const requestDistance = activeCrossCheckDistance
  activeCrossCheckRequestKey = ''
  activeCrossCheckGroup = null
  activeCrossCheckRunId = ''
  activeCrossCheckDistance = 0
  if (
    requestGroup &&
    requestRunId === store.currentRunId &&
    requestDistance === store.appliedGroupingDistance
  ) {
    store.markGroupSimilarityStatus(
      requestGroup,
      'pending',
      '已切换分组，等待后台 SSIM 计算'
    )
  }
  void cancelGroupSimilarityRequest(requestKey).catch((error) => {
    console.warn('取消旧的组内相似度计算失败:', error)
  })
}

function scheduleGroupSimilarityBackfill(currentGroup: ComparisonGroup) {
  if (!store.currentRunId) return
  const sourceGroupIndices = currentGroup.source_group_indices?.length
    ? currentGroup.source_group_indices
    : [currentGroup.group_index]
  void startGroupSimilarityBackfill(
    store.currentRunId,
    store.appliedGroupingDistance,
    sourceGroupIndices
  ).catch((error) => {
    console.warn('启动后台组内相似度预计算失败:', error)
  })
}

async function recalculateImageAssignment() {
  await loadGroupCrossCheckScores()
  hasManualAssignmentChanges.value = false
}

function markAsOriginal(imageId: number) {
  manualThumbnailIds.value = manualThumbnailIds.value.filter((id) => id !== imageId)
  if (!manualOriginalIds.value.includes(imageId)) {
    manualOriginalIds.value = [...manualOriginalIds.value, imageId]
  }
  hasManualAssignmentChanges.value = true
  removeCheckedImage(imageId)
}

function markAsThumbnail(imageId: number) {
  manualOriginalIds.value = manualOriginalIds.value.filter((id) => id !== imageId)
  if (!manualThumbnailIds.value.includes(imageId)) {
    manualThumbnailIds.value = [...manualThumbnailIds.value, imageId]
  }
  hasManualAssignmentChanges.value = true
}

function isCheckedForDelete(imageId: number) {
  return store.checkedImageIds.includes(imageId)
}

function handleCandidateDeleteChange(candidate: ThumbnailCandidate, checked: string | number | boolean) {
  if (checked) {
    if (!store.checkedImageIds.includes(candidate.member.image_id)) {
      store.checkedImageIds = [...store.checkedImageIds, candidate.member.image_id]
    }
    return
  }

  removeCheckedImage(candidate.member.image_id)
}

function handleSelectAllDeleteChange(checked: string | number | boolean) {
  const currentCandidateIds = new Set(
    thumbnailCandidates.value.map((candidate) => candidate.member.image_id)
  )
  const checkedOutsideCurrentGroup = store.checkedImageIds.filter(
    (imageId) => !currentCandidateIds.has(imageId)
  )
  store.checkedImageIds = checked
    ? [
        ...checkedOutsideCurrentGroup,
        ...thumbnailCandidates.value.map((candidate) => candidate.member.image_id)
      ]
    : checkedOutsideCurrentGroup
}

function removeCheckedImage(imageId: number) {
  store.checkedImageIds = store.checkedImageIds.filter((id) => id !== imageId)
}

function buildOriginalRows(members: ComparisonGroupMember[]): OriginalRow[] {
  if (members.length === 0) return []

  const originals = chooseOriginalMembers(members)
  const originalIds = new Set(originals.map((item) => item.member.image_id))
  const rows = originals.map((item) => ({
    id: `original-${item.member.image_id}`,
    member: item.member,
    source: item.source,
    reason: item.reason,
    candidates: [] as ThumbnailCandidate[]
  }))
  const rowByOriginalId = new Map(rows.map((row) => [row.member.image_id, row]))

  for (const member of members) {
    if (originalIds.has(member.image_id)) continue
    const candidate = buildThumbnailCandidate(member, originals.map((item) => item.member))
    if (!candidate) continue

    const row = rowByOriginalId.get(candidate.reference.image_id)
    if (row) row.candidates.push(candidate)
  }

  for (const row of rows) {
    row.candidates.sort((left, right) => {
      return (
        (right.similarity || 0) - (left.similarity || 0) ||
        left.member.relative_path.localeCompare(right.member.relative_path)
      )
    })
  }

  return rows
}

function chooseOriginalMembers(members: ComparisonGroupMember[]) {
  const maxPixels = Math.max(...members.map(getPixels))
  const automaticOriginalIds = group.value
    ? getAutomaticOriginalImageIds(group.value, originalRecognitionThreshold.value)
    : new Set<number>()

  const originals = members
    .filter((member) => !manualThumbnailIds.value.includes(member.image_id))
    .filter((member) => (
      manualOriginalIds.value.includes(member.image_id)
      || automaticOriginalIds.has(member.image_id)
    ))
    .map((member) => ({
      member,
      source: manualOriginalIds.value.includes(member.image_id) ? 'manual' as const : 'auto' as const,
      reason: manualOriginalIds.value.includes(member.image_id)
        ? '你手动设为原图'
        : getAutoOriginalReason(member, maxPixels)
    }))
    .sort((left, right) => compareQuality(right.member, left.member))

  if (originals.length > 0) return originals

  const fallback = getHighestQualityMember(members)
  return [{
    member: fallback,
    source: 'fallback' as const,
    reason: '组内最大的图片，默认作为原图'
  }]
}

function getAutoOriginalReason(member: ComparisonGroupMember, maxPixels: number) {
  if (group.value && member.image_id === group.value.representative_image_id) return '组内代表图'
  if (member.role === 'reference') return '系统参考图'
  if (getPixels(member) >= maxPixels * 0.9 && typeof member.ssim_score === 'number') {
    return `分辨率和画面比例接近，且任务标准 SSIM ${formatSimilarity(member.ssim_score)} 达到原图拆分阈值 ${formatSsimThreshold(originalRecognitionThreshold.value)}`
  }
  if (getPixels(member) >= maxPixels * 0.9) return '分辨率和画面比例接近；任务未记录 SSIM，按原图候选保留'
  return '系统识别为原图'
}

function buildThumbnailCandidate(
  member: ComparisonGroupMember,
  originals: ComparisonGroupMember[]
): ThumbnailCandidate | null {
  const reference = findReferenceOriginal(member, originals)
  if (!reference) return null

  const storedSimilarity = getStoredSimilarityForReference(member, reference)
  const crossSimilarity = getCrossSimilarity(member.image_id, reference.image_id)
  const activeSimilarity = crossSimilarity ?? storedSimilarity

  return {
    id: `candidate-${reference.image_id}-${member.image_id}`,
    member,
    reference,
    similarity: activeSimilarity
  }
}

function getStoredSimilarityForReference(
  member: ComparisonGroupMember,
  reference: ComparisonGroupMember
) {
  if (typeof member.ssim_score !== 'number') return undefined
  if (member.reference_image_id === reference.image_id) return member.ssim_score
  if (
    member.reference_image_id == null
    && member.reference_relative_path
    && member.reference_relative_path === reference.relative_path
  ) {
    return member.ssim_score
  }
  return undefined
}

function findReferenceOriginal(member: ComparisonGroupMember, originals: ComparisonGroupMember[]) {
  if (originals.length === 0) return null

  const bestCrossMatch = originals
    .map((original) => ({
      original,
      similarity: getCrossSimilarity(member.image_id, original.image_id)
    }))
    .filter((item): item is { original: ComparisonGroupMember; similarity: number } =>
      typeof item.similarity === 'number'
    )
    .sort((left, right) =>
      right.similarity - left.similarity || compareQuality(right.original, left.original)
    )[0]
  if (bestCrossMatch) return bestCrossMatch.original

  const explicitReference = originals.find((original) => original.image_id === member.reference_image_id)
  if (explicitReference) return explicitReference

  const sameReferenceName = member.reference_relative_path
    ? originals.find((original) => original.relative_path === member.reference_relative_path)
    : null
  if (sameReferenceName) return sameReferenceName

  return [...originals].sort((left, right) => compareQuality(right, left))[0]
}

function getCrossSimilarity(leftImageId: number, rightImageId: number) {
  return similarityScoreMap.value.get(getSimilarityKey(leftImageId, rightImageId))
}

function getSimilarityKey(leftImageId: number, rightImageId: number) {
  const [left, right] = [leftImageId, rightImageId].sort((a, b) => a - b)
  return `${left}:${right}`
}

function getHighestQualityMember(members: ComparisonGroupMember[]) {
  return [...members].sort((left, right) => compareQuality(right, left))[0]
}

function compareQuality(left: ComparisonGroupMember, right: ComparisonGroupMember) {
  return (
    getPixels(left) - getPixels(right) ||
    left.file_size - right.file_size ||
    right.relative_path.localeCompare(left.relative_path)
  )
}

function getPixels(member: ComparisonGroupMember) {
  return member.width * member.height
}

function openImageViewer(imageId: number) {
  const index = group.value?.members.findIndex((member) => member.image_id === imageId) ?? -1
  viewerIndex.value = Math.max(index, 0)
  resetImageViewerZoom()
  viewerVisible.value = true
}

function showPreviousImage() {
  viewerIndex.value = Math.max(0, viewerIndex.value - 1)
  resetImageViewerZoom()
}

function showNextImage() {
  const maxIndex = (group.value?.members.length || 1) - 1
  viewerIndex.value = Math.min(maxIndex, viewerIndex.value + 1)
  resetImageViewerZoom()
}

function handleViewerWheel(event: WheelEvent) {
  const zoomStep = event.deltaY < 0 ? 0.12 : -0.12
  viewerZoom.value = Math.min(4, Math.max(0.5, Math.round((viewerZoom.value + zoomStep) * 100) / 100))
}

function resetImageViewerZoom() {
  viewerZoom.value = 1
}
</script>

<style scoped lang="scss">
.group-detail {
  height: 100%;
  padding: 8px;
}

.detail-card {
  height: 100%;
  border-radius: 8px;
  display: flex;
  flex-direction: column;

  :deep(.el-card__body) {
    flex: 1;
    height: auto;
    padding: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
}

.detail-header {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}

.detail-title-block {
  min-width: max-content;
  flex: 1 1 120px;
}

.header-title {
  font-size: 16px;
  font-weight: 600;
  color: #303133;
}

.threshold-summary {
  margin-top: 4px;
  color: #909399;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.detail-actions {
  display: flex;
  flex: 1 1 100%;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;

  :deep(.el-button + .el-button) {
    margin-left: 0;
  }
}

.assignment-notice {
  margin: 0 12px 8px;
  padding: 8px 10px;
  border: 1px solid #f3d19e;
  border-radius: 8px;
  background: #fdf6ec;
  color: #b88230;
  font-size: 13px;
  line-height: 18px;
}

.recalculate-needed {
  animation: recalculate-nudge 1.4s ease-in-out infinite;
}

.sub-text {
  font-size: 12px;
  color: #909399;
}

.quality-control {
  padding: 0;
  background: #ffffff;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.quality-control-card {
  padding: 8px;
  border-radius: 8px;
  background: #f8fafc;
  border: 1px solid #edf1f7;
}

.quality-slider {
  padding: 0 28px;
  margin: 4px 0 32px;

  :deep(.el-slider__runway) {
    margin: 16px 0 30px;
  }

  :deep(.el-slider__marks-text) {
    margin-top: 12px;
    white-space: nowrap;
  }

  :deep(.el-slider__marks-text:last-child) {
    transform: translateX(-100%);
  }
}

.quality-heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  font-size: 13px;
  font-weight: 600;
  color: #303133;
}

.quality-value {
  color: #409eff;
  font-variant-numeric: tabular-nums;
}

.quality-help {
  color: #909399;
  font-size: 12px;
  line-height: 18px;
}

.column-header-help {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  color: #606266;
  cursor: help;

  .el-icon {
    color: #909399;
    font-size: 14px;
  }
}

.cross-check-loading {
  flex: 1;
  min-height: 260px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px;
  color: #606266;
  text-align: center;
  background: #ffffff;
}

.cross-check-spinner {
  font-size: 28px;
  color: #409eff;
  animation: cross-check-spin 1s linear infinite;
}

.cross-check-loading-title {
  color: #303133;
  font-size: 15px;
  font-weight: 650;
}

.cross-check-current {
  width: min(520px, 100%);
  color: #606266;
  font-size: 13px;
  line-height: 18px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cross-check-progress {
  width: min(520px, 100%);
}

.cross-check-meta {
  width: min(520px, 100%);
  color: #909399;
  font-size: 12px;
  line-height: 18px;
}

.detail-filter-bar {
  flex: 0 0 auto;
  margin: 0 12px 8px;
  padding: 8px 10px;
  border: 1px solid #edf1f7;
  border-radius: 8px;
  background: #fbfcff;
  color: #606266;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.no-candidate-empty {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.detail-table {
  flex: 1;
  min-height: 0;

  :deep(.el-checkbox__label) {
    width: 0;
    padding-left: 0;
    overflow: hidden;
  }

  :deep(.el-table__expanded-cell) {
    padding: 10px 16px 14px 56px;
    background: #fbfcff;
  }
}

.thumbnail-panel {
  border: 1px solid #edf1f7;
  border-radius: 8px;
  overflow: hidden;
  background: #ffffff;
}

.thumbnail-table {
  width: 100%;
}

.image-cell {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.thumb {
  max-height: 72px;
  max-width: 96px;
  object-fit: contain;
  border-radius: 6px;
  border: 2px solid transparent;
  background: #f5f7fa;
  cursor: zoom-in;

  &.active {
    border-color: #409eff;
    box-shadow: 0 0 0 2px #409eff30;
  }

  &.small {
    max-height: 56px;
    max-width: 76px;
  }
}

:global(.detail-image-preview-tooltip) {
  padding: 8px;
}

.detail-image-preview-tooltip-content {
  max-width: 520px;
  max-height: 420px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.detail-image-preview-large {
  max-width: 520px;
  max-height: 400px;
  object-fit: contain;
  border-radius: 6px;
  display: block;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}

.file-name-button {
  max-width: 100%;
  min-width: 0;
  height: auto;
  padding: 0;
  font-size: 12px;
  justify-content: flex-start;
  text-align: left;
  vertical-align: baseline;

  :deep(span) {
    min-width: 0;
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.path-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  white-space: nowrap;
}

.compact-action-button {
  padding: 0;
  font-size: 12px;

  & + & {
    margin-left: 0;
  }
}

.role-stack,
.metric-stack {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.metric-stack {
  color: #606266;
  font-size: 12px;
}

.metric-inline {
  display: inline-block;
  max-width: 100%;
  color: #606266;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: middle;
}

.empty-state {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

:deep(.active-member-row td) {
  background: #ecf5ff !important;
}

:deep(.original-row td:first-child) {
  border-left: 3px solid #67c23a;
}

:deep(.cluster-0 td) {
  box-shadow: inset 0 9999px #409eff08;
}

:deep(.cluster-1 td) {
  box-shadow: inset 0 9999px #67c23a08;
}

:deep(.cluster-2 td) {
  box-shadow: inset 0 9999px #e6a23c08;
}

:deep(.cluster-3 td) {
  box-shadow: inset 0 9999px #90939908;
}

.viewer {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  max-height: calc(100vh - 128px);
  min-height: 0;
}

.viewer-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
}

.viewer-zoom-label {
  min-width: 44px;
  color: #606266;
  text-align: center;
}

.viewer-stage {
  width: 100%;
  height: min(68vh, calc(100vh - 230px));
  min-height: 240px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  background: #f5f7fa;
  cursor: zoom-in;
}

.viewer-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  transform-origin: center center;
  transition: transform 0.08s ease-out;
  user-select: none;
  pointer-events: none;
}

.viewer-caption {
  max-width: 100%;
  color: #606266;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 0 0 auto;
}

:deep(.image-viewer-dialog) {
  max-height: calc(100vh - 32px);
  margin: 0;
  display: flex;
  flex-direction: column;
  margin:auto;
}

:deep(.image-viewer-dialog .el-dialog__body) {
  min-height: 0;
  overflow: hidden;
  padding-top: 12px;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@keyframes cross-check-spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

@keyframes recalculate-nudge {
  0%,
  100% {
    transform: translateX(0);
  }

  12% {
    transform: translateX(-1px);
  }

  24% {
    transform: translateX(1px);
  }

  36% {
    transform: translateX(-1px);
  }

  48% {
    transform: translateX(1px);
  }

  60% {
    transform: translateX(0);
  }
}
</style>
