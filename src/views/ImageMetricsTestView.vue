<template>
  <main class="metrics-window">
    <header class="window-header">
      <div>
        <h1>图片指标测试</h1>
        <p>临时比较多张图片，关闭窗口后不会保存任何内容。</p>
      </div>
      <el-button
        :icon="Close"
        data-test="close-window"
        aria-label="关闭图片指标测试窗口"
        @click="requestClose"
      >
        关闭
      </el-button>
    </header>

    <section class="metric-guide" aria-label="指标说明">
      <div class="guide-item">
        <strong>pHash 距离</strong>
        <span>范围 0–64，越小表示视觉特征越接近；0 不代表文件或像素完全一致。</span>
      </div>
      <div class="guide-item">
        <strong>低精度相似度</strong>
        <span>范围 0–1，越接近 1 越相似；复现主程序当前的灰度像素差算法，不是标准 SSIM。</span>
      </div>
      <div class="guide-item">
        <strong>标准 SSIM</strong>
        <span>越接近 1 通常越相似，标准公式可能出现负值；仅在点击单张卡片后计算。</span>
      </div>
      <div class="guide-item">
        <strong>尺寸处理</strong>
        <span>低精度复用主程序尺寸策略且最长边 512px；标准 SSIM 不使用 200px 缩略图或 512px 限制，仅将较大原图缩小到较小原图的完整分辨率。</span>
      </div>
    </section>

    <section class="toolbar" aria-label="图片操作">
      <div class="toolbar-actions">
        <el-button type="primary" :icon="FolderOpened" @click="chooseImages">
          选择图片
        </el-button>
        <el-button :icon="Delete" :disabled="!session.hasContent.value" @click="clearAll">
          清空全部
        </el-button>
      </div>
      <div class="toolbar-summary">
        <span>已加载 {{ session.items.value.length }} 张</span>
        <span v-if="session.loadingCount.value > 0">
          · 正在加载 {{ session.loadingCount.value }} 张
        </span>
        <span class="drop-hint">支持多选，也可以将图片拖入窗口</span>
      </div>
    </section>

    <section
      class="gallery-shell"
      :class="{ 'is-dragging': isDragging }"
      aria-live="polite"
    >
      <div v-if="isDragging" class="drop-overlay">
        <el-icon><UploadFilled /></el-icon>
        <span>松开即可添加图片</span>
      </div>

      <div
        v-if="session.items.value.length === 0 && session.loadingCount.value === 0"
        class="empty-state"
      >
        <el-icon><Picture /></el-icon>
        <h2>添加图片开始测试</h2>
        <p>选择多张图片，或直接拖入此窗口。</p>
        <el-button type="primary" :icon="FolderOpened" @click="chooseImages">
          选择图片
        </el-button>
      </div>

      <template v-else>
        <div
          v-if="session.items.value.length > 0 && !session.baselinePath.value"
          class="baseline-hint"
        >
          点击一张图片卡片，将它设为底图
        </div>

        <div class="metrics-grid">
          <article
            v-for="(item, index) in session.items.value"
            :key="item.path"
            class="metrics-card"
            :class="{ 'is-baseline': item.path === session.baselinePath.value }"
            :data-test="`card-${index}`"
            role="button"
            tabindex="0"
            :aria-pressed="item.path === session.baselinePath.value"
            :aria-label="`将 ${item.fileName} 设为底图`"
            @click="selectBaseline(item.path)"
            @keydown.enter.prevent="selectBaseline(item.path)"
            @keydown.space.prevent="selectBaseline(item.path)"
          >
            <div class="image-wrap" @click.stop>
              <el-image
                class="metrics-image"
                :src="item.thumbnailDataUrl"
                :preview-src-list="previewUrls"
                :initial-index="index"
                :alt="item.fileName"
                fit="contain"
                preview-teleported
              >
                <template #error>
                  <div class="image-error">缩略图加载失败</div>
                </template>
              </el-image>
              <el-button
                class="remove-button"
                :icon="Close"
                circle
                plain
                size="small"
                :aria-label="`移除 ${item.fileName}`"
                @click.stop="session.remove(item.path)"
              />
            </div>

            <div class="card-body">
              <el-tooltip :content="item.fileName" placement="top-start">
                <h2 class="file-name">{{ item.fileName }}</h2>
              </el-tooltip>
              <p class="file-meta">
                {{ formatFileSize(item.fileSize) }}
              </p>

              <div v-if="item.path === session.baselinePath.value" class="baseline-label">
                底图
              </div>

              <div v-else class="metrics-list">
                <div class="metrics-inline">
                  <el-tooltip :content="phashTooltip(item.phash)" placement="top-start">
                    <span class="metric-chip">
                      <span>pHash 距离：</span>
                      <span class="metric-value">{{ phashValue(item) }}</span>
                    </span>
                  </el-tooltip>
                  <span class="metric-chip">
                    <span>低精度 SSIM：</span>
                    <el-button
                      v-if="item.low.status === 'error'"
                      text
                      type="primary"
                      size="small"
                      :data-test="`low-${index}`"
                      @click.stop="requestLowPrecision(item.path)"
                    >
                      重试
                    </el-button>
                    <span v-else class="metric-value">
                      {{ lowPrecisionValue(item) }}
                    </span>
                  </span>
                  <span class="metric-chip">
                    <span>标准 SSIM：</span>
                    <template v-if="item.high.status === 'done'">
                      <span class="metric-value">
                        {{ formatScore(item.high.value.score) }} · {{ formatDuration(item.high.value.durationMs) }}
                      </span>
                    </template>
                    <span v-else-if="item.high.status === 'loading'" class="metric-pending">
                      计算中…
                    </span>
                    <el-button
                      v-else
                      text
                      type="primary"
                      size="small"
                      :data-test="`high-${index}`"
                      :disabled="!session.baselinePath.value"
                      @click.stop="requestHighPrecision(item.path)"
                    >
                      {{ item.high.status === 'error' ? '重试' : '点击计算' }}
                    </el-button>
                  </span>
                </div>
                <p v-if="item.low.status === 'error'" class="metric-error">
                  低精度失败：{{ item.low.error }}
                </p>
                <p v-if="item.high.status === 'error'" class="metric-error">
                  标准 SSIM 失败：{{ item.high.error }}
                </p>
              </div>
            </div>
          </article>
          <article
            v-for="index in session.loadingCount.value"
            :key="`loading-${index}`"
            class="metrics-card loading-card"
            data-test="loading-card"
            aria-label="图片加载中"
          >
            <el-skeleton animated>
              <template #template>
                <el-skeleton-item variant="image" class="loading-image" />
                <div class="loading-body">
                  <el-skeleton-item variant="h3" style="width: 62%" />
                  <el-skeleton-item variant="text" style="width: 38%" />
                </div>
              </template>
            </el-skeleton>
          </article>
        </div>
      </template>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Close, Delete, FolderOpened, Picture, UploadFilled } from '@element-plus/icons-vue'
import {
  computeTestLowPrecision,
  computeTestStandardSsim,
  loadTestImage
} from '@/api/imageMetrics'
import { createImageMetricsSession, type TestImageItem } from '@/features/imageMetrics/session'

const appWindow = getCurrentWindow()
const session = createImageMetricsSession({
  loadImage: loadTestImage,
  computeLow: computeTestLowPrecision,
  computeHigh: computeTestStandardSsim
})
const isDragging = ref(false)
const previewUrls = computed(() => session.items.value.map((item) => convertFileSrc(item.path)))
const baselineItem = computed(() =>
  session.items.value.find((item) => item.path === session.baselinePath.value)
)

let allowNativeClose = false
let unlistenClose: (() => void) | undefined
let unlistenDrop: (() => void) | undefined

async function chooseImages() {
  try {
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [{
        name: '图片',
        extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff']
      }]
    })
    if (Array.isArray(selected)) await importPaths(selected)
  } catch (error) {
    ElMessage.error(`选择图片失败：${message(error)}`)
  }
}

async function importPaths(paths: string[]) {
  const previousBaseline = session.baselinePath.value
  await session.addPaths(paths)
  if (previousBaseline && session.items.value.some((item) => item.path === previousBaseline)) {
    await session.setBaseline(previousBaseline)
  }
  if (session.importErrors.value.length > 0) {
    ElMessage.error(`${session.importErrors.value.length} 张图片加载失败：${session.importErrors.value[0]}`)
    session.clearImportErrors()
  }
  if (session.duplicateCount.value > 0) {
    ElMessage.info(`已跳过 ${session.duplicateCount.value} 张重复图片`)
    session.clearDuplicateCount()
  }
}

function selectBaseline(path: string) {
  void session.setBaseline(path)
}

async function requestHighPrecision(path: string) {
  const started = await session.computeHighPrecision(path)
  if (!started) ElMessage.info('请等待当前高精度计算完成')
}

async function requestLowPrecision(path: string) {
  await session.retryLowPrecision(path)
}

function clearAll() {
  session.reset()
  ElMessage.success('已清空本次测试')
}

async function confirmDiscard() {
  if (!session.hasContent.value) return true
  const runningNotice = session.hasRunningTasks.value
    ? '当前计算无法立即中止，会在后台完成后自动释放资源。\n'
    : ''
  try {
    await ElMessageBox.confirm(
      `${runningNotice}本次测试内容不会保存，关闭后将全部清空。确定关闭吗？`,
      '关闭图片指标测试',
      {
        confirmButtonText: '确定关闭',
        cancelButtonText: '继续测试',
        type: 'warning'
      }
    )
    return true
  } catch (error) {
    if (error === 'cancel' || error === 'close') return false
    throw error
  }
}

async function requestClose() {
  if (!await confirmDiscard()) return
  allowNativeClose = true
  try {
    await appWindow.close()
  } catch (error) {
    allowNativeClose = false
    ElMessage.error(`关闭窗口失败：${message(error)}`)
  }
}

function phashTooltip(candidatePhash: string) {
  const baselinePhash = baselineItem.value?.phash || '未选择底图'
  return `底图 pHash：${baselinePhash}\n当前图片 pHash：${candidatePhash}`
}

function phashValue(item: TestImageItem) {
  if (!session.baselinePath.value) return '等待中'
  return item.phashDistance === null ? '失败' : String(item.phashDistance)
}

function lowPrecisionValue(item: TestImageItem) {
  if (item.low.status === 'done') return formatScore(item.low.value.similarity)
  if (item.low.status === 'error') return '失败'
  if (item.low.status === 'loading') return '计算中…'
  return '等待中'
}

function formatScore(value: number) {
  return value.toFixed(6)
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function formatDuration(milliseconds: number) {
  return milliseconds < 1000 ? `${milliseconds} ms` : `${(milliseconds / 1000).toFixed(2)} s`
}

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

onMounted(async () => {
  unlistenClose = await appWindow.onCloseRequested((event) => {
    if (allowNativeClose) return
    event.preventDefault()
    void requestClose()
  })
  unlistenDrop = await appWindow.onDragDropEvent(async (event) => {
    if (event.payload.type === 'over') {
      isDragging.value = true
    } else if (event.payload.type === 'drop') {
      isDragging.value = false
      await importPaths(event.payload.paths)
    } else {
      isDragging.value = false
    }
  })
})

onBeforeUnmount(() => {
  unlistenClose?.()
  unlistenDrop?.()
  session.reset()
})

defineExpose({ addImagePathsForTest: importPaths })
</script>

<style scoped lang="scss">
.metrics-window {
  height: 100vh;
  min-height: 0;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: #f5f7fa;
  color: #303133;
  overflow: hidden;
}

.window-header,
.toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.window-header {
  h1 {
    margin: 0;
    font-size: 22px;
  }

  p {
    margin: 4px 0 0;
    color: #606266;
    font-size: 13px;
  }
}

.metric-guide {
  flex: 0 0 auto;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  padding: 12px;
  border: 1px solid #d9ecff;
  border-radius: 8px;
  background: #ecf5ff;
}

.guide-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
  font-size: 12px;
  line-height: 1.5;

  strong {
    flex: 0 0 auto;
    color: #337ecc;
  }

  span {
    color: #4c5967;
  }
}

.toolbar {
  padding: 10px 12px;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #fff;
}

.toolbar-actions,
.toolbar-summary {
  display: flex;
  align-items: center;
  gap: 8px;
}

.toolbar-summary {
  color: #606266;
  font-size: 13px;
}

.drop-hint {
  margin-left: 8px;
  color: #909399;
}

.gallery-shell {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #fff;

  &.is-dragging {
    border-color: #409eff;
  }
}

.drop-overlay {
  position: fixed;
  z-index: 20;
  inset: 12px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  border: 2px dashed #409eff;
  border-radius: 12px;
  background: rgba(236, 245, 255, 0.94);
  color: #337ecc;
  font-size: 18px;
  pointer-events: none;

  .el-icon {
    font-size: 44px;
  }
}

.empty-state {
  min-height: 360px;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: #606266;

  > .el-icon {
    font-size: 54px;
    color: #a8abb2;
  }

  h2,
  p {
    margin: 0;
  }

  h2 {
    font-size: 18px;
  }

  p {
    color: #909399;
    font-size: 13px;
  }
}

.baseline-hint {
  margin: 12px 12px 0;
  padding: 9px 12px;
  border-radius: 6px;
  background: #fdf6ec;
  color: #b88230;
  font-size: 13px;
}

.metrics-grid {
  padding: 12px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 480px), 1fr));
  gap: 12px;
  align-content: start;
}

.metrics-card {
  min-width: 0;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #fff;
  overflow: hidden;
  cursor: pointer;
  transition: border-color 0.18s ease, box-shadow 0.18s ease, background-color 0.18s ease;

  &:hover {
    border-color: #79bbff;
    box-shadow: 0 6px 18px rgba(64, 158, 255, 0.12);
  }

  &:focus-visible {
    outline: 2px solid #409eff;
    outline-offset: 2px;
  }

  &.is-baseline {
    border: 2px solid #409eff;
    background: #ecf5ff;
  }
}

.image-wrap {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 120px;
  max-height: 200px;
  background: #f5f7fa;
  overflow: hidden;
}

.metrics-image {
  width: 100%;
  max-height: 200px;
  display: block;

  :deep(.el-image__inner) {
    max-height: 200px;
  }
}

.image-error {
  padding: 32px;
  color: #909399;
  font-size: 13px;
}

.loading-card {
  cursor: default;
}

.loading-image {
  width: 100%;
  height: 160px;
}

.loading-body {
  padding: 12px;
  display: grid;
  gap: 10px;
}

.remove-button {
  position: absolute;
  top: 8px;
  right: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.14);
}

.card-body {
  padding: 12px;
}

.file-name {
  margin: 0;
  overflow: hidden;
  color: #303133;
  font-size: 15px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-meta {
  margin: 5px 0 0;
  color: #909399;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.baseline-label {
  margin-top: 12px;
  padding: 10px;
  border-radius: 6px;
  background: #409eff;
  color: #fff;
  font-size: 14px;
  font-weight: 650;
  text-align: center;
}

.metrics-list {
  margin-top: 12px;
  border-top: 1px solid #ebeef5;
}

.metrics-inline {
  padding-top: 8px;
  display: flex;
  align-items: center;
  gap: 6px 10px;
  flex-wrap: wrap;
  color: #606266;
  font-size: 12px;
}

.metric-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  min-width: 0;

  :deep(.el-button) {
    height: auto;
    padding: 0;
  }
}

.metric-value,
.metric-pending {
  color: #303133;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}

.metric-pending {
  color: #909399;
  font-weight: 400;
}

.is-error,
.metric-error {
  color: #f56c6c;
}

.metric-error {
  margin: 7px 0 0;
  font-size: 12px;
  line-height: 1.4;
}

@media (max-width: 900px) {
  .metric-guide {
    grid-template-columns: 1fr;
  }

  .toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .toolbar-summary {
    flex-wrap: wrap;
  }
}

@media (prefers-reduced-motion: reduce) {
  .metrics-card {
    transition: none;
  }
}
</style>
