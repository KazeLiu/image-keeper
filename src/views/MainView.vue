<template>
  <div v-if="activeScreen !== 'workspace'" class="entry-view">
    <section class="entry-panel">
      <div class="task-cards">
        <button type="button" class="task-card" @click="startNewTask">
          <span class="task-icon">
            <el-icon><Plus /></el-icon>
          </span>
          <span class="task-title">选择新建任务</span>
          <span class="task-copy">进入多目录对比工作台，重新选择文件夹并开始计算</span>
        </button>

        <button type="button" class="task-card" @click="showHistory">
          <span class="task-icon">
            <el-icon><FolderOpened /></el-icon>
          </span>
          <span class="task-title">加载任务</span>
          <span class="task-copy">从历史任务中加载已经计算过的结果</span>
        </button>
      </div>

      <section v-if="activeScreen === 'history'" class="history-panel">
        <div class="history-header">
          <div>
            <h2>历史任务</h2>
            <p>删除任务只会清理数据库记录，不会删除任何图片文件。</p>
          </div>
          <el-button
            :icon="Refresh"
            :loading="comparisonStore.isLoadingHistory"
            @click="refreshHistory"
          >
            刷新
          </el-button>
        </div>

        <div v-if="comparisonStore.historyRuns.length === 0" class="history-empty">
          还没有历史任务
        </div>

        <div v-else class="history-list">
          <article
            v-for="run in comparisonStore.historyRuns"
            :key="run.run_id"
            class="history-row"
          >
            <button type="button" class="history-load" @click="loadHistoryRun(run.run_id)">
              <span class="history-row-title">
                <span class="status-dot" :class="statusClass(run.status)" />
                {{ formatRunTime(run.created_at) }}
              </span>
              <span class="history-row-path">{{ formatPathSummary(run) }}</span>
              <span class="history-row-meta">
                {{ statusLabel(run.status) }} · {{ run.baseline_total }} 基准 · {{ run.comparison_total }} 对比 · {{ run.result_count }} 结果
              </span>
            </button>

            <el-tooltip content="删除历史任务" placement="right-start">
              <el-button
                :icon="Delete"
                type="danger"
                plain
                circle
                aria-label="删除历史任务"
                @click="deleteHistoryRun(run.run_id)"
              />
            </el-tooltip>
          </article>
        </div>
      </section>
    </section>
  </div>

  <div v-else class="main-view">
    <section
      class="aside-panel"
      :class="{ 'is-collapsed': isStatsPanelCollapsed }"
      :style="{ width: `${asidePanelWidth}px` }"
    >
      <div v-if="isStatsPanelCollapsed" class="aside-collapsed">
        <el-tooltip content="展开左栏" placement="right-start">
          <el-button
            :icon="Expand"
            circle
            plain
            aria-label="展开左栏"
            @click="setStatsPanelCollapsed(false)"
          />
        </el-tooltip>
      </div>

      <template v-else>
        <div class="workspace-actions"> 
          <el-button class="workspace-action" :icon="Fold" plain @click="setStatsPanelCollapsed(true)">
            隐藏左栏
          </el-button>
          <el-button class="workspace-action" :icon="ArrowLeft" plain @click="backToEntry">
            返回入口
          </el-button>
        </div>
        <ComparisonDirectorySelector v-if="showDirectorySelector" />
        <ComparisonProgress />
      </template>
    </section>

    <div
      v-if="!isStatsPanelCollapsed"
      class="resize-handle"
      role="separator"
      aria-label="调整左侧宽度"
      @mousedown="startResize('left', $event)"
    />

    <main class="main-panel">
      <ComparisonResults />
    </main>

    <div
      class="resize-handle"
      role="separator"
      aria-label="调整右侧宽度"
      @mousedown="startResize('right', $event)"
    />

    <section class="preview-panel" :style="{ width: `${rightWidth}px` }">
      <ComparisonGroupDetail />
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { ArrowLeft, Delete, Expand, FolderOpened, Fold, Plus, Refresh } from '@element-plus/icons-vue'
import ComparisonDirectorySelector from '@/components/ComparisonDirectorySelector.vue'
import ComparisonProgress from '@/components/ComparisonProgress.vue'
import ComparisonResults from '@/components/ComparisonResults.vue'
import ComparisonGroupDetail from '@/components/ComparisonGroupDetail.vue'
import { useComparisonStore } from '@/stores/comparisonStore'
import { RunStatus, type ComparisonRunHistoryItem } from '@/types'

const MIN_LEFT_WIDTH = 320
const MIN_CENTER_WIDTH = 460
const MIN_RIGHT_WIDTH = 420
const COLLAPSED_LEFT_WIDTH = 52
const DEFAULT_LEFT_RATIO = 0.22
const DEFAULT_RIGHT_RATIO = 0.52
const LAYOUT_STORAGE_KEY = 'imagekeeper:workspace-column-ratios'
const STATS_PANEL_COLLAPSED_STORAGE_KEY = 'imagekeeper:workspace-stats-panel-collapsed'

const leftWidth = ref(400)
const rightWidth = ref(560)
const layoutRatios = ref({ leftRatio: DEFAULT_LEFT_RATIO, rightRatio: DEFAULT_RIGHT_RATIO })
const comparisonStore = useComparisonStore()
const activeScreen = ref<'home' | 'history' | 'workspace'>('home')
const isHistoryRunView = ref(false)
const isStatsPanelCollapsed = ref(readStoredStatsPanelCollapsed())

let activeResize: 'left' | 'right' | null = null
let resizeStartX = 0
let resizeStartLeftWidth = 0
let resizeStartRightWidth = 0

const showDirectorySelector = computed(() => {
  if (isHistoryRunView.value) return false
  if (!comparisonStore.currentRunId) return true
  if (comparisonStore.isRunning) return true
  return comparisonStore.currentPhase !== 'complete'
})

const asidePanelWidth = computed(() =>
  isStatsPanelCollapsed.value ? COLLAPSED_LEFT_WIDTH : leftWidth.value
)

function clampLayout(nextLeftWidth: number, nextRightWidth: number) {
  const availableWidth = getWorkspaceAvailableWidth()
  let clampedLeft = Math.max(MIN_LEFT_WIDTH, nextLeftWidth)
  let clampedRight = Math.max(MIN_RIGHT_WIDTH, nextRightWidth)
  const displayedLeftWidth = () =>
    isStatsPanelCollapsed.value ? COLLAPSED_LEFT_WIDTH : clampedLeft

  let centerWidth = availableWidth - displayedLeftWidth() - clampedRight
  if (centerWidth < MIN_CENTER_WIDTH) {
    let shortage = MIN_CENTER_WIDTH - centerWidth

    if (!isStatsPanelCollapsed.value) {
      const leftReduction = Math.min(shortage, clampedLeft - MIN_LEFT_WIDTH)
      clampedLeft -= leftReduction
      shortage -= leftReduction
    }

    if (shortage > 0) {
      const rightReduction = Math.min(shortage, clampedRight - MIN_RIGHT_WIDTH)
      clampedRight -= rightReduction
    }
  }

  centerWidth = availableWidth - displayedLeftWidth() - clampedRight
  if (centerWidth > clampedRight && centerWidth > MIN_CENTER_WIDTH) {
    const transferableWidth = centerWidth - MIN_CENTER_WIDTH
    const neededToMakeRightLargest = (centerWidth - clampedRight) / 2
    clampedRight += Math.max(0, Math.min(transferableWidth, neededToMakeRightLargest))
  }

  leftWidth.value = clampedLeft
  rightWidth.value = clampedRight
}

function startResize(target: 'left' | 'right', event: MouseEvent) {
  activeResize = target
  resizeStartX = event.clientX
  resizeStartLeftWidth = leftWidth.value
  resizeStartRightWidth = rightWidth.value

  document.body.classList.add('is-resizing-columns')
  window.addEventListener('mousemove', handleResize)
  window.addEventListener('mouseup', stopResize)
}

function handleResize(event: MouseEvent) {
  if (!activeResize) return

  const deltaX = event.clientX - resizeStartX
  if (activeResize === 'left') {
    clampLayout(resizeStartLeftWidth + deltaX, rightWidth.value)
  } else {
    clampLayout(leftWidth.value, resizeStartRightWidth - deltaX)
  }
}

function stopResize() {
  activeResize = null
  document.body.classList.remove('is-resizing-columns')
  window.removeEventListener('mousemove', handleResize)
  window.removeEventListener('mouseup', stopResize)
  rememberCurrentLayoutRatios()
  saveLayoutRatios()
}

function handleWindowResize() {
  applyStoredLayoutRatios()
}

function setStatsPanelCollapsed(collapsed: boolean) {
  isStatsPanelCollapsed.value = collapsed
  saveStatsPanelCollapsed()
  handleWindowResize()
}

function startNewTask() {
  isHistoryRunView.value = false
  comparisonStore.clearCurrentRunView()
  activeScreen.value = 'workspace'
  handleWindowResize()
}

async function showHistory() {
  activeScreen.value = 'history'
  await refreshHistory()
}

async function refreshHistory() {
  try {
    await comparisonStore.refreshHistory()
  } catch (error: any) {
    ElMessage.error(error?.message || '刷新历史任务失败')
  }
}

async function loadHistoryRun(runId: string) {
  try {
    await comparisonStore.loadHistoryRun(runId)
    isHistoryRunView.value = true
    activeScreen.value = 'workspace'
    handleWindowResize()
    ElMessage.success('已加载历史任务')
  } catch (error: any) {
    ElMessage.error(error?.message || '加载历史任务失败')
  }
}

async function deleteHistoryRun(runId: string) {
  try {
    await ElMessageBox.confirm(
      '只会删除数据库里的历史任务记录，不会删除任何图片文件。',
      '删除历史任务',
      {
        confirmButtonText: '删除记录',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )

    await comparisonStore.deleteHistoryRun(runId)
    ElMessage.success('历史任务已删除')
  } catch (error: any) {
    if (error === 'cancel' || error === 'close') return
    ElMessage.error(error?.message || '删除历史任务失败')
  }
}

function backToEntry() {
  activeScreen.value = 'home'
}

function restoreLayoutRatios() {
  try {
    const rawValue = window.localStorage.getItem(LAYOUT_STORAGE_KEY)
    if (!rawValue) {
      applyStoredLayoutRatios()
      return
    }

    const parsed = JSON.parse(rawValue) as { leftRatio?: number; rightRatio?: number }
    const leftRatio = normalizeRatio(parsed.leftRatio)
    const rightRatio = normalizeRatio(parsed.rightRatio)
    if (leftRatio === null || rightRatio === null) {
      applyStoredLayoutRatios()
      return
    }

    layoutRatios.value = { leftRatio, rightRatio }
    applyStoredLayoutRatios()
  } catch (error) {
    console.warn('恢复工作台栏宽失败:', error)
    applyStoredLayoutRatios()
  }
}

function saveLayoutRatios() {
  try {
    window.localStorage.setItem(
      LAYOUT_STORAGE_KEY,
      JSON.stringify(layoutRatios.value)
    )
  } catch (error) {
    console.warn('保存工作台栏宽失败:', error)
  }
}

function getWorkspaceAvailableWidth() {
  const handleWidth = isStatsPanelCollapsed.value ? 6 : 12
  return Math.max(0, window.innerWidth - handleWidth)
}

function applyStoredLayoutRatios() {
  const availableWidth = getWorkspaceAvailableWidth()
  if (availableWidth <= 0) return

  clampLayout(
    availableWidth * layoutRatios.value.leftRatio,
    availableWidth * layoutRatios.value.rightRatio
  )
}

function rememberCurrentLayoutRatios() {
  const availableWidth = getWorkspaceAvailableWidth()
  if (availableWidth <= 0) return

  layoutRatios.value = {
    leftRatio: isStatsPanelCollapsed.value
      ? layoutRatios.value.leftRatio
      : normalizeRatio(leftWidth.value / availableWidth) ?? DEFAULT_LEFT_RATIO,
    rightRatio: normalizeRatio(rightWidth.value / availableWidth) ?? DEFAULT_RIGHT_RATIO
  }
}

function readStoredStatsPanelCollapsed() {
  try {
    return window.localStorage.getItem(STATS_PANEL_COLLAPSED_STORAGE_KEY) === 'true'
  } catch (error) {
    console.warn('恢复左侧面板显示状态失败:', error)
    return false
  }
}

function saveStatsPanelCollapsed() {
  try {
    window.localStorage.setItem(
      STATS_PANEL_COLLAPSED_STORAGE_KEY,
      String(isStatsPanelCollapsed.value)
    )
  } catch (error) {
    console.warn('保存左侧面板显示状态失败:', error)
  }
}

function normalizeRatio(value: unknown) {
  if (typeof value !== 'number' || Number.isNaN(value)) return null
  return Math.min(0.75, Math.max(0.05, value))
}

function statusLabel(status: RunStatus) {
  const labels: Record<RunStatus, string> = {
    [RunStatus.Pending]: '准备中',
    [RunStatus.Preflight]: '预检查',
    [RunStatus.Indexing]: '索引中',
    [RunStatus.Matching]: '匹配中',
    [RunStatus.Scoring]: '评分中',
    [RunStatus.Resolving]: '整理中',
    [RunStatus.ReviewPending]: '待复核',
    [RunStatus.AnalysisComplete]: '已完成',
    [RunStatus.ActionInProgress]: '处理中',
    [RunStatus.ActionComplete]: '操作完成',
    [RunStatus.CompletedWithErrors]: '有错误',
    [RunStatus.Paused]: '已暂停',
    [RunStatus.Canceled]: '已取消',
    [RunStatus.Failed]: '失败'
  }
  return labels[status] || status
}

function statusClass(status: RunStatus) {
  if ([RunStatus.AnalysisComplete, RunStatus.ReviewPending, RunStatus.ActionComplete].includes(status)) {
    return 'success'
  }
  if ([RunStatus.Failed, RunStatus.Canceled, RunStatus.CompletedWithErrors].includes(status)) {
    return 'danger'
  }
  if ([RunStatus.Paused, RunStatus.Pending].includes(status)) {
    return 'muted'
  }
  return 'running'
}

function formatRunTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  })
}

function formatPathSummary(run: ComparisonRunHistoryItem) {
  const baselineName = folderName(run.baseline_root_path)
  return run.comparison_root_paths.length > 0
    ? `${baselineName} + ${run.comparison_root_paths.length} 个目录`
    : baselineName
}

function folderName(path: string) {
  const normalizedPath = path.replace(/[\\/]+$/, '')
  const parts = normalizedPath.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] || normalizedPath || path
}

onMounted(() => {
  restoreLayoutRatios()
  handleWindowResize()
  window.addEventListener('resize', handleWindowResize)
})

onBeforeUnmount(() => {
  stopResize()
  window.removeEventListener('resize', handleWindowResize)
})
</script>

<style scoped>
.entry-view {
  width: 100%;
  height: 100vh;
  display: flex;
  justify-content: center;
  background-color: #f5f7fa;
  overflow: hidden;
}

.entry-panel {
  width: min(980px, calc(100vw - 48px));
  height: 100%;
  box-sizing: border-box;
  padding: 72px 0 48px;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.task-cards {
  flex: 0 0 auto;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 24px;
}

.task-card {
  min-height: 260px;
  padding: 32px;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #ffffff;
  color: #303133;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  text-align: center;
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease;
}

.task-card:hover {
  border-color: #409eff;
  box-shadow: 0 10px 28px rgba(64, 158, 255, 0.14);
  transform: translateY(-2px);
}

.task-card:focus-visible {
  outline: 2px solid #409eff;
  outline-offset: 3px;
}

.task-icon {
  width: 58px;
  height: 58px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: #ecf5ff;
  color: #409eff;
  font-size: 28px;
}

.task-title {
  font-size: 24px;
  font-weight: 700;
}

.task-copy {
  max-width: 300px;
  color: #606266;
  font-size: 14px;
  line-height: 1.6;
}

.history-panel {
  min-height: 0;
  margin-top: 24px;
  padding: 24px;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #ffffff;
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  overflow: hidden;
}

.history-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  margin-bottom: 18px;
}

.history-header h2 {
  margin: 0;
  font-size: 20px;
  color: #303133;
}

.history-header p {
  margin: 6px 0 0;
  color: #909399;
  font-size: 13px;
}

.history-empty {
  min-height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #909399;
  font-size: 14px;
  border: 1px dashed #dcdfe6;
  border-radius: 8px;
}

.history-list {
  min-height: 0;
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
}

.history-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  background: #ffffff;
}

.history-load {
  min-width: 0;
  flex: 1;
  padding: 0;
  border: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.history-load:focus-visible {
  outline: 2px solid #409eff;
  outline-offset: 4px;
  border-radius: 4px;
}

.history-row-title,
.history-row-path,
.history-row-meta {
  display: flex;
  align-items: center;
  min-width: 0;
}

.history-row-title {
  gap: 8px;
  color: #303133;
  font-size: 15px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}

.status-dot {
  width: 8px;
  height: 8px;
  flex: 0 0 auto;
  border-radius: 999px;
  background-color: #409eff;
}

.status-dot.success {
  background-color: #67c23a;
}

.status-dot.danger {
  background-color: #f56c6c;
}

.status-dot.muted {
  background-color: #c0c4cc;
}

.history-row-path {
  margin-top: 6px;
  color: #606266;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-row-meta {
  margin-top: 6px;
  color: #909399;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.main-view {
  width: 100%;
  height: 100vh;
  display: flex;
  min-width: 0;
  overflow: hidden;
  background-color: #f5f7fa;
}

.aside-panel {
  min-width: 320px;
  border-right: 1px solid #dcdfe6;
  background-color: #f5f7fa;
  padding: 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 0 0 auto;
}

.aside-panel.is-collapsed {
  min-width: 52px;
  padding: 12px 8px;
  align-items: center;
  overflow: hidden;
}

.aside-collapsed {
  width: 100%;
  display: flex;
  justify-content: center;
}

.workspace-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  flex: 0 0 auto;
}

.workspace-action {
  width: 100%;
  margin-left:0;
}

.main-panel {
  min-width: 460px;
  flex: 1 1 auto;
  background-color: #f5f7fa;
  padding: 8px;
  overflow: hidden;
}

.preview-panel {
  min-width: 420px;
  border-left: 1px solid #dcdfe6;
  background-color: #f5f7fa;
  flex: 0 0 auto;
  overflow: hidden;
}

.resize-handle {
  width: 6px;
  flex: 0 0 6px;
  cursor: col-resize;
  background: transparent;
  position: relative;
  z-index: 2;
}

.resize-handle::before {
  content: '';
  position: absolute;
  inset: 0 2px;
  background: transparent;
  transition: background-color 0.15s ease;
}

.resize-handle:hover::before {
  background: #409eff66;
}

@media (max-width: 760px) {
  .entry-panel {
    width: calc(100vw - 32px);
    padding-top: 32px;
  }

  .entry-view {
    min-height: 100vh;
    height: auto;
    overflow-y: auto;
  }

  .entry-panel {
    height: auto;
  }

  .task-cards {
    grid-template-columns: 1fr;
  }

  .task-card {
    min-height: 190px;
  }

  .history-header {
    flex-direction: column;
  }
}
</style>

<style>
body.is-resizing-columns {
  cursor: col-resize;
  user-select: none;
}
</style>
