<template>
  <div class="group-detail">
    <el-card v-if="group" shadow="never" class="detail-card">
      <template #header>
        <div class="detail-header">
          <div class="detail-title-block">
            <div class="header-title">第 {{ group.group_index }} 组</div>
            <div class="threshold-summary">
              差分 {{ formatQualityPercent(originalRecognitionThreshold) }} · 低质量 {{ formatQualityPercent(store.qualitySelectionThreshold) }}
              ·
              <span
                class="precision-summary"
                :class="useHighPrecisionSimilarity ? 'is-high' : 'is-fast'"
              >
                {{ useHighPrecisionSimilarity ? '标准结构相似性' : '低精度' }}
              </span>
            </div>
          </div>
          <div class="detail-actions">
            <el-popover
              v-model:visible="thresholdPopoverVisible"
              trigger="click"
              placement="bottom-end"
              :width="500"
            >
              <template #reference>
                <el-button :icon="Setting" plain size="small">
                  识别设置
                </el-button>
              </template>

              <div class="quality-control">
                <div class="quality-control-card">
                  <div class="quality-heading">
                    <span>差分图识别阈值</span>
                    <span class="quality-value">{{ formatQualityPercent(originalRecognitionThreshold) }}</span>
                  </div>
                  <el-slider
                    class="quality-slider"
                    :model-value="originalRecognitionPercent"
                    :min="95"
                    :max="100"
                    :step="0.1"
                    :marks="originalRecognitionMarks"
                    :format-tooltip="formatPercentSliderTooltip"
                    @input="handleOriginalRecognitionInput"
                  />
                  <div class="quality-help">
                    用来判断大小相近的图片是否都算原图；数值越低，越容易把缩略图变成原图。
                  </div>
                </div>

                <div class="quality-control-card">
                  <div class="quality-heading">
                    <span>低质量勾选阈值</span>
                    <span class="quality-value">{{ formatQualityPercent(store.qualitySelectionThreshold) }}</span>
                  </div>
                  <el-slider
                    class="quality-slider"
                    :model-value="qualitySliderValue"
                    :min="0"
                    :max="138"
                    :step="1"
                    :marks="qualitySliderMarks"
                    :format-tooltip="formatQualitySliderTooltip"
                    @input="handleQualityThresholdInput"
                    @change="handleQualityThresholdChange"
                  />
                  <div class="quality-help">
                    自动根据相似度勾选低质量的图片；
                  </div>
                </div>

                <div class="quality-control-card">
                  <div class="quality-heading">
                    <span>相似度对比精度</span>
                    <el-switch
                      :model-value="useHighPrecisionSimilarity"
                      size="small"
                      active-text="标准结构相似性"
                      inactive-text="低精度"
                      @change="handlePrecisionModeChange"
                    />
                  </div>
                  <div class="quality-help">
                    低精度速度更快，适合大批量图片；标准结构相似性会使用更完整的窗口算法重新判断，相似度更细，但等待时间会明显变长。
                  </div>
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
        <span>已隐藏 {{ hiddenOriginalRowCount }} 张暂无低质量候选的原图</span>
        <el-switch
          v-model="showEmptyOriginalRows"
          size="small"
          active-text="显示无候选原图"
        />
      </div>

      <el-empty
        v-if="!isLoadingCrossCheck && visibleOriginalRows.length === 0"
        class="no-candidate-empty"
        description="本组暂无可删除候选"
        :image-size="72"
      />

      <el-table
        v-else-if="!isLoadingCrossCheck"
        :data="visibleOriginalRows"
        row-key="id"
        height="100%"
        class="detail-table"
        :default-expand-all="true"
        :row-class-name="getOriginalRowClassName"
        @row-click="handleOriginalRowClick"
      >
        <el-table-column type="expand" width="42">
          <template #default="{ row }">
            <div class="thumbnail-panel">
              <el-empty
                v-if="row.candidates.length === 0"
                description="这张原图下暂无低质量候选"
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
                      :disabled="!candidate.shouldDelete"
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
                  <template #default="{ row: candidate }">
                    <el-tooltip :content="candidate.reason" placement="right-start">
                      <el-tag
                        :type="candidate.shouldDelete ? 'danger' : 'info'"
                        size="small"
                        effect="light"
                      >
                        {{ candidate.shouldDelete ? '建议删除' : '候选缩略图' }}
                      </el-tag>
                    </el-tooltip>
                  </template>
                </el-table-column>

                <el-table-column label="分辨率 / 大小" width="162" align="center">
                  <template #default="{ row: candidate }">
                    <span class="metric-inline">{{ formatImageMetrics(candidate.member) }}</span>
                  </template>
                </el-table-column>

                <el-table-column label="相似度" width="96" align="center">
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
                <el-table-column label="路径操作" width="104" align="center">
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

        <el-table-column label="原图依据" width="150" align="center">
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

        <el-table-column label="低质量候选" width="96" align="center">
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
        <el-table-column label="路径操作" width="104" align="center">
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
import { Loading, Setting } from '@element-plus/icons-vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { batchRecycleImages, getGroupSimilarityScores } from '@/api/comparison'
import { useComparisonStore } from '@/stores/comparisonStore'
import type { ComparisonGroupMember, GroupSimilarityProgress, GroupSimilarityScore } from '@/types'

const store = useComparisonStore()
const ORIGINAL_RECOGNITION_PERCENT_KEY = 'imagekeeper:original-recognition-percent'
const HIGH_PRECISION_SIMILARITY_KEY = 'imagekeeper:high-precision-similarity'
const GROUP_SCORE_CACHE_LIMIT = 30

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
  shouldDelete: boolean
  reason: string
}

const viewerVisible = ref(false)
const viewerIndex = ref(0)
const viewerZoom = ref(1)
const thresholdPopoverVisible = ref(false)
const isRecycling = ref(false)
const originalRecognitionPercent = ref(readStoredOriginalRecognitionPercent())
const useHighPrecisionSimilarity = ref(readStoredHighPrecisionSimilarity())
const manualOriginalIds = ref<number[]>([])
const manualThumbnailIds = ref<number[]>([])
const showEmptyOriginalRows = ref(false)
const groupSimilarityScores = ref<GroupSimilarityScore[]>([])
const groupSimilarityScoreCache = new Map<string, GroupSimilarityScore[]>()
const crossCheckProgress = ref<GroupSimilarityProgress | null>(null)
const isLoadingCrossCheck = ref(false)
const hasManualAssignmentChanges = ref(false)
let crossCheckRequestId = 0
let activeCrossCheckRequestKey = ''
let unlistenGroupSimilarityProgress: UnlistenFn | null = null
let groupSimilarityProgressListenerPromise: Promise<void> | null = null

const group = computed(() => store.selectedGroup)
const originalRecognitionThreshold = computed(() => originalRecognitionPercent.value / 100)

const qualitySliderMarks = {
  0: '80%',
  18: '98%',
  38: '99%',
  138: '100%'
}

const originalRecognitionMarks = {
  95: '宽松',
  98: '标准',
  100: '保守'
}

const qualitySliderValue = computed(() => thresholdToSliderValue(store.qualitySelectionThreshold))

const groupKey = computed(() =>
  `${group.value?.group_index || ''}:${group.value?.members.map((member) => member.image_id).join(',') || ''}`
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

const suggestedSelectionKey = computed(() =>
  originalRows.value
    .flatMap((row) => row.candidates)
    .filter((candidate) => candidate.shouldDelete)
    .map((candidate) => candidate.member.image_id)
    .sort((left, right) => left - right)
    .join(',')
)

const viewerMember = computed(() => {
  const members = group.value?.members || []
  return members[viewerIndex.value] || null
})

watch(
  groupKey,
  () => {
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
  [suggestedSelectionKey, isLoadingCrossCheck],
  ([, loading]) => {
    if (loading) return
    applyAutoQualitySelection()
  },
  { immediate: true }
)

onBeforeUnmount(() => {
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
  if (row.shouldDelete) classes.push('low-quality-row')
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

async function openFolder(member: ComparisonGroupMember) {
  try {
    await invoke('open_folder', { path: getFolderPath(member) })
  } catch (error: any) {
    ElMessage.error(error || '打开文件夹失败')
  }
}

function formatSimilarity(value?: number | null) {
  if (value === undefined || value === null) return '—'
  return formatQualityPercent(value)
}

function formatQualityPercent(value: number) {
  return `${(value * 100).toFixed(2)}%`
}

function formatQualitySliderTooltip(value: number) {
  return formatQualityPercent(sliderValueToThreshold(value))
}

function formatPercentSliderTooltip(value: number) {
  return `${value.toFixed(1)}%`
}

function handleOriginalRecognitionInput(value: number | number[]) {
  const nextValue = Array.isArray(value) ? value[0] : value
  originalRecognitionPercent.value = Math.min(100, Math.max(95, Math.round(nextValue * 10) / 10))
  rememberOriginalRecognitionPercent(originalRecognitionPercent.value)
}

function handleQualityThresholdInput(value: number | number[]) {
  const nextValue = Array.isArray(value) ? value[0] : value
  store.setQualitySelectionThreshold(sliderValueToThreshold(nextValue))
  applyAutoQualitySelection()
}

function handleQualityThresholdChange() {
  ElMessage.info('已按当前阈值更新勾选')
}

function sliderValueToThreshold(sliderValue: number) {
  const tick = Math.min(138, Math.max(0, Math.round(sliderValue)))
  if (tick <= 18) return (80 + tick) / 100
  if (tick <= 38) return (98 + (tick - 18) * 0.05) / 100
  return (99 + (tick - 38) * 0.01) / 100
}

function thresholdToSliderValue(threshold: number) {
  const percent = Math.min(100, Math.max(80, threshold * 100))
  if (percent <= 98) return Math.round(percent - 80)
  if (percent <= 99) return 18 + Math.round((percent - 98) / 0.05)
  return 38 + Math.round((percent - 99) / 0.01)
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
      `将 ${imageIds.length} 张图片移动到当前任务的回收站目录（.recycle），之后可按回收站记录恢复。是否继续？`,
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
      ElMessage.warning(`已移动 ${successImageIds.length} 张，失败 ${failedOutcomes.length} 张：${firstError}`)
      return
    }

    ElMessage.success(`已移动 ${successImageIds.length} 张图片到回收站`)
  } catch (error: any) {
    ElMessage.error(error?.message || '移动到回收站失败')
  } finally {
    isRecycling.value = false
  }
}

function readStoredOriginalRecognitionPercent() {
  const rawValue = window.localStorage.getItem(ORIGINAL_RECOGNITION_PERCENT_KEY)
  const parsedValue = rawValue ? Number(rawValue) : 98.5
  if (!Number.isFinite(parsedValue)) return 98.5
  return Math.min(100, Math.max(95, Math.round(parsedValue * 10) / 10))
}

function rememberOriginalRecognitionPercent(value: number) {
  window.localStorage.setItem(ORIGINAL_RECOGNITION_PERCENT_KEY, value.toString())
}

function readStoredHighPrecisionSimilarity() {
  return window.localStorage.getItem(HIGH_PRECISION_SIMILARITY_KEY) === 'true'
}

function rememberHighPrecisionSimilarity(value: boolean) {
  window.localStorage.setItem(HIGH_PRECISION_SIMILARITY_KEY, String(value))
}

async function handlePrecisionModeChange(value: string | number | boolean) {
  const nextValue = Boolean(value)
  if (nextValue === useHighPrecisionSimilarity.value) return

  if (nextValue) {
    try {
      await ElMessageBox.confirm(
        '标准结构相似性会使用更完整的窗口算法重新比对当前组，相似度判断更细，但会明显拉长等待时间。是否开启？',
        '开启标准结构相似性对比',
        {
          confirmButtonText: '开启标准结构相似性',
          cancelButtonText: '继续低精度',
          type: 'warning'
        }
      )
    } catch (error) {
      if (error === 'cancel' || error === 'close') return
      throw error
    }
  }

  useHighPrecisionSimilarity.value = nextValue
  rememberHighPrecisionSimilarity(nextValue)
  groupSimilarityScoreCache.clear()
  groupSimilarityScores.value = []
  ElMessage.info(nextValue ? '已切换为标准结构相似性，将重新计算当前组' : '已切换为低精度，将重新计算当前组')
  await loadGroupCrossCheckScores()
}

async function loadGroupCrossCheckScores() {
  const requestId = ++crossCheckRequestId
  const requestKey = `group-${requestId}-${Date.now()}`
  activeCrossCheckRequestKey = requestKey
  const currentGroup = group.value
  groupSimilarityScores.value = []
  crossCheckProgress.value = null

  if (!currentGroup || !store.currentRunId || currentGroup.members.length < 2) {
    isLoadingCrossCheck.value = false
    return
  }
  const scoreCacheKey = getGroupScoreCacheKey(
    store.currentRunId,
    currentGroup.members,
    useHighPrecisionSimilarity.value
  )
  const cachedScores = groupSimilarityScoreCache.get(scoreCacheKey)
  if (cachedScores) {
    groupSimilarityScores.value = cachedScores
    isLoadingCrossCheck.value = false
    return
  }

  isLoadingCrossCheck.value = true
  try {
    await ensureGroupSimilarityProgressListener()
    const scores = await getGroupSimilarityScores(
      store.currentRunId,
      currentGroup.members.map((member) => member.image_id),
      requestKey,
      useHighPrecisionSimilarity.value
    )
    if (requestId === crossCheckRequestId) {
      groupSimilarityScores.value = scores
      rememberGroupSimilarityScores(scoreCacheKey, scores)
    }
  } catch (error) {
    if (requestId === crossCheckRequestId) {
      console.warn('组内交叉相似度计算失败:', error)
      ElMessage.warning('组内交叉验证失败，暂时使用已有参考关系')
    }
  } finally {
    if (requestId === crossCheckRequestId) {
      isLoadingCrossCheck.value = false
    }
  }
}

function getGroupScoreCacheKey(runId: string, members: ComparisonGroupMember[], useHighPrecision: boolean) {
  const memberKeys = members
    .map((member) => [
      member.image_id,
      member.file_path,
      member.file_size,
      member.width,
      member.height,
      member.phash || ''
    ].join('|'))
    .sort()
    .join(';')
  return `${runId}:${useHighPrecision ? 'high' : 'fast'}:${memberKeys}`
}

function rememberGroupSimilarityScores(cacheKey: string, scores: GroupSimilarityScore[]) {
  if (groupSimilarityScoreCache.has(cacheKey)) {
    groupSimilarityScoreCache.delete(cacheKey)
  }
  groupSimilarityScoreCache.set(cacheKey, scores)
  while (groupSimilarityScoreCache.size > GROUP_SCORE_CACHE_LIMIT) {
    const oldestKey = groupSimilarityScoreCache.keys().next().value
    if (!oldestKey) break
    groupSimilarityScoreCache.delete(oldestKey)
  }
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
  applyAutoQualitySelection()
}

function markAsThumbnail(imageId: number) {
  manualOriginalIds.value = manualOriginalIds.value.filter((id) => id !== imageId)
  if (!manualThumbnailIds.value.includes(imageId)) {
    manualThumbnailIds.value = [...manualThumbnailIds.value, imageId]
  }
  hasManualAssignmentChanges.value = true
  applyAutoQualitySelection()
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

function removeCheckedImage(imageId: number) {
  store.checkedImageIds = store.checkedImageIds.filter((id) => id !== imageId)
}

function applyAutoQualitySelection() {
  store.checkedImageIds = originalRows.value
    .flatMap((row) => row.candidates)
    .filter((candidate) => candidate.shouldDelete)
    .map((candidate) => candidate.member.image_id)
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
        Number(right.shouldDelete) - Number(left.shouldDelete) ||
        (right.similarity || 0) - (left.similarity || 0) ||
        left.member.relative_path.localeCompare(right.member.relative_path)
      )
    })
  }

  return rows
}

function chooseOriginalMembers(members: ComparisonGroupMember[]) {
  const maxPixels = Math.max(...members.map(getPixels))
  const maxAspect = getAspectRatio(getHighestQualityMember(members))

  const originals = members
    .filter((member) => !manualThumbnailIds.value.includes(member.image_id))
    .filter((member) => manualOriginalIds.value.includes(member.image_id) || isAutoOriginal(member, maxPixels, maxAspect))
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

function isAutoOriginal(member: ComparisonGroupMember, maxPixels: number, maxAspect: number) {
  if (group.value && member.image_id === group.value.representative_image_id) return true
  if (member.role === 'reference') return true

  const highResolution = getPixels(member) >= maxPixels * 0.9
  const sameShape = getAspectDiff(member, maxAspect) <= 0.02
  const similarEnough = typeof member.ssim_score !== 'number' || member.ssim_score >= originalRecognitionThreshold.value

  return highResolution && sameShape && similarEnough
}

function getAutoOriginalReason(member: ComparisonGroupMember, maxPixels: number) {
  if (group.value && member.image_id === group.value.representative_image_id) return '组内代表图'
  if (member.role === 'reference') return '系统参考图'
  if (getPixels(member) >= maxPixels * 0.9) return '分辨率接近组内最大图'
  return '系统识别为原图'
}

function buildThumbnailCandidate(
  member: ComparisonGroupMember,
  originals: ComparisonGroupMember[]
): ThumbnailCandidate | null {
  const reference = findReferenceOriginal(member, originals)
  if (!reference) return null

  const similarity = member.ssim_score
  const crossSimilarity = getCrossSimilarity(member.image_id, reference.image_id)
  const activeSimilarity = crossSimilarity ?? similarity
  const lowerResolution = getPixels(member) < getPixels(reference) * 0.9
  const smallerFile = member.file_size < reference.file_size * 0.75
  const manuallyDowngraded = manualThumbnailIds.value.includes(member.image_id)
  const similarEnough = typeof activeSimilarity === 'number' && activeSimilarity >= store.qualitySelectionThreshold
  const shouldDelete = similarEnough && (lowerResolution || smallerFile || manuallyDowngraded)

  return {
    id: `candidate-${reference.image_id}-${member.image_id}`,
    member,
    reference,
    similarity: activeSimilarity,
    shouldDelete,
    reason: getCandidateReason({
      lowerResolution,
      smallerFile,
      manuallyDowngraded,
      similarEnough,
      hasCrossSimilarity: typeof crossSimilarity === 'number'
    })
  }
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

function getCandidateReason(flags: {
  lowerResolution: boolean
  smallerFile: boolean
  manuallyDowngraded: boolean
  similarEnough: boolean
  hasCrossSimilarity: boolean
}) {
  if (!flags.similarEnough) return '未达到当前勾选阈值'
  if (flags.manuallyDowngraded) return '你手动设为缩略图'
  if (flags.hasCrossSimilarity && flags.lowerResolution && flags.smallerFile) return '交叉验证最相似，且分辨率和体积都更低'
  if (flags.hasCrossSimilarity && flags.lowerResolution) return '交叉验证最相似，且分辨率更低'
  if (flags.hasCrossSimilarity && flags.smallerFile) return '交叉验证最相似，且文件体积更小'
  if (flags.lowerResolution && flags.smallerFile) return '分辨率和体积都更低'
  if (flags.lowerResolution) return '分辨率更低'
  if (flags.smallerFile) return '文件体积更小'
  return '大小接近，默认不自动删除'
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

function getAspectRatio(member: ComparisonGroupMember) {
  if (member.height === 0) return 0
  return member.width / member.height
}

function getAspectDiff(member: ComparisonGroupMember, referenceAspect: number) {
  if (referenceAspect === 0) return 0
  return Math.abs(getAspectRatio(member) - referenceAspect) / referenceAspect
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
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.detail-title-block {
  min-width: 0;
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

.precision-summary {
  font-weight: 650;

  &.is-high {
    color: #f56c6c;
  }

  &.is-fast {
    color: #67c23a;
  }
}

.detail-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
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

:deep(.low-quality-row td:first-child) {
  border-left: 3px solid #f56c6c;
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
