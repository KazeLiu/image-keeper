<template>
  <main class="metrics-window">
    <header class="window-header">
      <div>
        <p class="eyebrow">IMAGEKEEPER TOOL</p>
        <h1>图片指标测试</h1>
        <p>点击一张图片卡片，将它设为标准图</p>
      </div>
      <div class="header-actions">
        <el-popover
          v-model:visible="guidePopoverVisible"
          trigger="click"
          placement="bottom-end"
          :width="580"
        >
          <template #reference>
            <el-button :icon="InfoFilled" plain size="small">
              指标说明
            </el-button>
          </template>

          <div class="guide-content">
            <div class="guide-card">
              <div class="guide-heading">
                感知哈希距离
                <span class="guide-tag">快速粗筛</span>
              </div>
              <div class="guide-help">
                <p><strong>怎么看：</strong>它是两个 64 位视觉指纹中不同位的数量，范围为 0–64；数值越小，整体画面越可能接近。</p>
                <p><strong>它的作用：</strong>只负责快速找出可能相似的候选，减少后续精细计算的数量。</p>
                <p><strong>注意：</strong>距离为 0 只表示感知哈希相同，不代表文件或每个像素完全一致，不能单独用来判断重复或删除。</p>
              </div>
            </div>

            <div class="guide-card">
              <div class="guide-heading">
                标准 SSIM
                <span class="guide-tag">精细对比</span>
              </div>
              <div class="guide-help">
                <p><strong>怎么看：</strong>直接显示原始值，不转换成百分比。越接近 1，归一化后的亮度、对比度和局部结构通常越相似；标准公式也可能得到负值。</p>
                <p><strong>它的作用：</strong>使用 11×11、σ=1.5 的高斯窗口，对感知哈希筛出的候选做更细的结构比较。</p>
                <p><strong>注意：</strong>数值高只表示画面结构更接近，不代表当前图一定是原图、压缩图或低质量图。</p>
              </div>
            </div>

            <div class="guide-card">
              <div class="guide-heading">两个指标如何配合</div>
              <div class="guide-help">
                <p>感知哈希先粗筛“可能相似”，标准 SSIM 再确认“结构有多接近”。两项结果不一致时，通常需要检查裁剪、文字、调色、边框或构图变化。</p>
                <p>本页只展示算法证据，不给出删除结论。主程序还会综合文件哈希、分辨率、文件大小、宽高比和候选冲突，做更保守的分类。</p>
              </div>
            </div>

            <div class="guide-card">
              <div class="guide-heading">算法一致性与尺寸处理</div>
              <div class="guide-help">
                <p>与主程序、组内交叉比较和找差分图共用同一套实现，不再存在数值不同的低精度 SSIM。</p>
                <p>较大图片使用 Lanczos3 缩小到较小图片的完整宽高后再计算；不读取 200px 缩略图，也不使用 512px 限制。</p>
                <p>图片解码、感知哈希和标准 SSIM 统一受最多 4 路共享并行计算限制。</p>
              </div>
            </div>
          </div>
        </el-popover>
      </div>
    </header>

    <section class="panel upload-panel" aria-labelledby="metrics-upload-title">
      <div class="panel-heading">
        <div>
          <h2 id="metrics-upload-title">测试图片</h2>
          <p>
            可一次添加多张；已加载 {{ session.items.value.length }} 张
            <template v-if="session.loadingCount.value > 0">
              · 正在加载 {{ session.loadingCount.value }} 张
            </template>
          </p>
        </div>
        <div class="panel-actions">
          <el-button type="primary" plain :icon="FolderOpened" @click="chooseImages">
            添加图片
          </el-button>
          <el-button :icon="Delete" :disabled="!session.hasContent.value" @click="clearAll">
            清空全部
          </el-button>
        </div>
      </div>

      <section
        class="gallery-shell"
        :class="{ 'is-dragging': isDragging }"
        aria-live="polite"
      >
        <div v-if="isDragging" class="drop-overlay">
          <el-icon><UploadFilled /></el-icon>
          <span>松开即可添加图片</span>
        </div>

        <button
          v-if="session.items.value.length === 0 && session.loadingCount.value === 0"
          type="button"
          class="drop-empty"
          @click="chooseImages"
        >
          <el-icon><PictureRounded /></el-icon>
          <span>点击选择，或把图片拖进窗口</span>
          <small>支持 JPG、PNG、WebP、BMP、GIF、TIFF</small>
        </button>

        <template v-else>
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
              :aria-label="`将 ${item.fileName} 设为标准图`"
              :aria-disabled="item.loadState === 'loading'"
              @click="selectBaseline(item.path)"
              @keydown.enter.prevent="selectBaseline(item.path)"
              @keydown.space.prevent="selectBaseline(item.path)"
            >
            <div
              v-loading="isDifferenceLoading(item)"
              class="image-wrap"
              :class="{ 'is-difference-active': isDifferenceActive(item) }"
              element-loading-text="正在生成差异高亮…"
              @click.stop
            >
              <el-skeleton v-if="item.loadState === 'loading'" animated class="inline-loading">
                <template #template>
                  <el-skeleton-item variant="image" class="inline-loading-image" />
                </template>
              </el-skeleton>
              <el-image
                v-else
                class="metrics-image"
                :src="displayImageSource(item)"
                :preview-src-list="imagePreviewUrls(item)"
                :initial-index="imagePreviewIndex(item)"
                :alt="isDifferenceHighlighted(item) ? `${item.fileName} 的差异高亮` : item.fileName"
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
                @click.stop="removeImage(item.path)"
              />
            </div>

            <div class="card-body">
              <el-tooltip :content="item.fileName" placement="right-start">
                <h2 class="file-name">{{ item.fileName }}</h2>
              </el-tooltip>
              <div class="file-meta">
                <span>{{ item.loadState === 'loading' ? '加载中…' : formatFileSize(item.fileSize) }}</span>
                <el-button
                  v-if="item.loadState === 'ready' && session.baselinePath.value && item.path !== session.baselinePath.value"
                  class="difference-toggle"
                  :type="isDifferenceActive(item) ? 'primary' : 'default'"
                  :plain="!isDifferenceActive(item)"
                  size="small"
                  :icon="View"
                  :loading="isDifferenceLoading(item)"
                  :aria-pressed="isDifferenceActive(item)"
                  :data-test="`difference-${index}`"
                  @click.stop="toggleDifferenceHighlight(item)"
                >
                  差异高亮
                </el-button>
              </div>

              <div
                v-if="isDifferenceActive(item) && differencePreview.error.value"
                class="difference-inline-error"
                role="alert"
              >
                <span>差异高亮生成失败：{{ differencePreview.error.value }}</span>
                <el-button
                  text
                  type="primary"
                  size="small"
                  :data-test="`difference-retry-${index}`"
                  @click.stop="differencePreview.retry"
                >
                  重试
                </el-button>
              </div>

              <div v-if="item.loadState === 'loading'" class="baseline-label is-loading">
                正在读取图片
              </div>

              <div v-else-if="item.path === session.baselinePath.value" class="baseline-label">
                标准图
              </div>

              <div v-else class="metrics-list">
                <div class="metrics-inline">
                  <el-tooltip placement="right-start">
                    <template #content>
                      <div class="phash-tooltip-content">
                        <div class="phash-tooltip-line">
                          标准图感知哈希：{{ baselineItem?.phash || '未选择标准图' }}
                        </div>
                        <div class="phash-tooltip-line">
                          当前图片感知哈希：{{ item.phash }}
                        </div>
                      </div>
                    </template>
                    <span class="metric-chip">
                      <span>感知哈希距离：</span>
                      <span class="metric-value">{{ phashValue(item) }}</span>
                    </span>
                  </el-tooltip>
                  <span class="metric-chip">
                    <span>标准 SSIM：</span>
                    <el-button
                      v-if="item.ssim.status === 'error'"
                      text
                      type="primary"
                      size="small"
                      :data-test="`ssim-${index}`"
                      @click.stop="requestSsim(item.path)"
                    >
                      重试
                    </el-button>
                    <span v-else class="metric-value">
                      {{ ssimValue(item) }}
                    </span>
                  </span>
                </div>
                <p v-if="item.ssim.status === 'error'" class="metric-error">
                  标准 SSIM 失败：{{ item.ssim.error }}
                </p>
              </div>
            </div>
            </article>
          </div>
        </template>
      </section>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Close, Delete, FolderOpened, InfoFilled, PictureRounded, UploadFilled, View } from '@element-plus/icons-vue'
import {
  computeTestDifferencePreview,
  computeTestPhash,
  computeTestSsim,
  loadTestImage
} from '@/api/imageMetrics'
import { createImageMetricsSession, type TestImageItem } from '@/features/imageMetrics/session'
import { createDifferencePreview } from '@/features/imageMetrics/differencePreview'
import { formatSsim } from '@/features/similarity'

const appWindow = getCurrentWindow()
const session = createImageMetricsSession({
  loadImage: loadTestImage,
  computePhash: computeTestPhash,
  computeSsim: computeTestSsim
})
const differencePreview = createDifferencePreview(computeTestDifferencePreview)
const isDragging = ref(false)
const guidePopoverVisible = ref(false)
const previewUrls = computed(() =>
  session.items.value
    .filter((item) => item.loadState === 'ready')
    .map((item) => convertFileSrc(item.path))
)
const baselineItem = computed(() =>
  session.items.value.find((item) => item.path === session.baselinePath.value)
)

let unlistenClose: (() => void) | undefined
let unlistenDrop: (() => void) | undefined
let allowNextCloseRequest = false
let closeConfirmationPending = false

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
  const item = session.items.value.find((item) => item.path === path)
  if (item?.loadState !== 'ready') return
  differencePreview.close()
  void session.setBaseline(path)
}

function toggleDifferenceHighlight(candidate: TestImageItem) {
  if (!baselineItem.value || candidate.loadState !== 'ready') return
  if (isDifferenceActive(candidate)) {
    differencePreview.close()
    return
  }
  void differencePreview.open(baselineItem.value, candidate)
}

function isDifferenceActive(item: TestImageItem) {
  return differencePreview.visible.value && differencePreview.candidate.value?.path === item.path
}

function isDifferenceLoading(item: TestImageItem) {
  return isDifferenceActive(item) && differencePreview.loading.value
}

function isDifferenceHighlighted(item: TestImageItem) {
  return isDifferenceActive(item) && Boolean(differencePreview.result.value)
}

function displayImageSource(item: TestImageItem) {
  return isDifferenceHighlighted(item)
    ? differencePreview.result.value!.highlightDataUrl
    : item.thumbnailDataUrl
}

function imagePreviewUrls(item: TestImageItem) {
  return isDifferenceHighlighted(item)
    ? [differencePreview.result.value!.highlightDataUrl]
    : previewUrls.value
}

function imagePreviewIndex(item: TestImageItem) {
  return isDifferenceHighlighted(item) ? 0 : previewIndex(item)
}

function removeImage(path: string) {
  differencePreview.close()
  session.remove(path)
}

function previewIndex(item: TestImageItem) {
  return session.items.value
    .filter((candidate) => candidate.loadState === 'ready')
    .findIndex((candidate) => candidate.path === item.path)
}

async function requestSsim(path: string) {
  await session.retrySsim(path)
}

function clearAll() {
  differencePreview.close()
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

function phashValue(item: TestImageItem) {
  if (!session.baselinePath.value) return '等待中'
  if (item.phashState === 'error') return '失败'
  if (item.phashState === 'loading') return '计算中…'
  if (!baselineItem.value || baselineItem.value.phashState === 'loading') return '计算中…'
  if (baselineItem.value.phashState === 'error') return '失败'
  return item.phashDistance === null ? '失败' : String(item.phashDistance)
}

function ssimValue(item: TestImageItem) {
  if (item.ssim.status === 'done') {
    return `${formatSsim(item.ssim.value.score)} · ${formatDuration(item.ssim.value.durationMs)}`
  }
  if (item.ssim.status === 'error') return '失败'
  if (item.ssim.status === 'loading') return '计算中…'
  return '等待中'
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
    if (allowNextCloseRequest) {
      allowNextCloseRequest = false
      return
    }
    if (!session.hasContent.value) return
    event.preventDefault()
    if (closeConfirmationPending) return
    closeConfirmationPending = true
    void confirmDiscard()
      .then(async (confirmed) => {
        if (!confirmed) return
        allowNextCloseRequest = true
        try {
          await appWindow.close()
        } catch (error) {
          allowNextCloseRequest = false
          ElMessage.error(`关闭窗口失败：${message(error)}`)
        }
      })
      .catch((error) => {
        allowNextCloseRequest = false
        ElMessage.error(`关闭确认失败：${message(error)}`)
      })
      .finally(() => {
        closeConfirmationPending = false
      })
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
  differencePreview.close()
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

.window-header {
  padding: 20px 24px;
  border: 1px solid #dcdfe6;
  border-radius: 10px;
  background: #ffffff;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 24px;
}

.eyebrow {
  margin: 0 0 4px !important;
  color: #409eff !important;
  font-size: 11px !important;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.window-header h1 {
  margin: 0;
  font-size: 26px;
  line-height: 1.3;
}

.window-header p {
  margin: 6px 0 0;
  color: #606266;
  font-size: 14px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.metric-guide {
  padding: 0;
}

.guide-content {
  padding: 0;
  max-height: min(70vh, 560px);
  overflow-y: auto;
  background: #ffffff;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.guide-card {
  padding: 12px;
  border-radius: 8px;
  background: #f8fafc;
  border: 1px solid #edf1f7;
}

.guide-heading {
  margin-bottom: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: #303133;
}

.guide-tag {
  padding: 2px 6px;
  border-radius: 999px;
  background: #ecf5ff;
  color: #337ecc;
  font-size: 11px;
  font-weight: 600;
}

.guide-help {
  color: #909399;
  font-size: 12px;
  line-height: 18px;
}

.guide-help p {
  margin: 0;
}

.guide-help p + p {
  margin-top: 6px;
}

.guide-help strong {
  color: #606266;
  font-weight: 600;
}

.panel {
  border: 1px solid #dcdfe6;
  border-radius: 10px;
  background: #ffffff;
}

.upload-panel {
  flex: 1 1 auto;
  min-height: 0;
  padding: 18px;
  display: flex;
  flex-direction: column;
}

.panel-heading {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}

.panel-heading h2 {
  margin: 0;
  font-size: 17px;
}

.panel-heading p {
  margin: 5px 0 0;
  color: #606266;
  font-size: 12px;
}

.panel-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.gallery-shell {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  border-radius: 8px;
  background: transparent;

  &.is-dragging {
    outline: 2px dashed #409eff;
    outline-offset: -2px;
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

.drop-empty {
  width: 100%;
  min-height: 112px;
  border: 1px dashed #c0c4cc;
  border-radius: 8px;
  background: #fafcff;
  color: #606266;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 5px;
  cursor: pointer;
  height:100%;
}

.drop-empty:hover {
  border-color: #409eff;
  color: #409eff;
}

.drop-empty .el-icon {
  font-size: 28px;
}

.drop-empty small {
  color: #909399;
}

.metrics-grid {
  padding: 12px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 260px), 1fr));
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

  &[aria-disabled='true'] {
    cursor: wait;
  }
}

.image-wrap {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 86px;
  max-height: 140px;
  background: #f5f7fa;
  overflow: hidden;
  transition: box-shadow 180ms ease;
}

.image-wrap.is-difference-active {
  box-shadow: inset 0 0 0 2px #e6a23c;
}

.metrics-image {
  width: 100%;
  max-height: 140px;
  display: block;

  :deep(.el-image__inner) {
    max-height: 140px;
  }
}

.inline-loading {
  width: 100%;
}

.inline-loading-image {
  width: 100%;
  height: 140px;
}

.image-error {
  padding: 32px;
  color: #909399;
  font-size: 13px;
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
  min-height: 28px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: #909399;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

:deep(.difference-toggle) {
  width: 94px;
  height: 28px;
  flex: 0 0 94px;
}

.difference-inline-error {
  margin-top: 7px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: #f56c6c;
  font-size: 12px;
  line-height: 1.4;

  span {
    min-width: 0;
  }

  :deep(.el-button) {
    min-height: 28px;
    flex: 0 0 auto;
  }
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

.baseline-label.is-loading {
  background: #f4f4f5;
  color: #909399;
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

:global(.phash-tooltip-content) {
  display: flex;
  flex-direction: column;
  gap: 4px;
  line-height: 1.5;
  font-variant-numeric: tabular-nums;
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
  .panel-heading {
    align-items: flex-start;
    flex-direction: column;
  }

  .panel-actions {
    flex-wrap: wrap;
  }
}

@media (prefers-reduced-motion: reduce) {
  .metrics-card,
  .image-wrap {
    transition: none;
  }
}
</style>
