<template>
  <el-dialog
    :model-value="visible"
    title="差异高亮"
    width="92%"
    :style="{ maxWidth: '1500px' }"
    class="difference-dialog"
    append-to-body
    destroy-on-close
    @update:model-value="handleVisibility"
  >
    <div class="difference-content">
      <div class="difference-toolbar">
        <div>
          <strong v-if="result && result.regionCount > 0">
            检测到 {{ result.regionCount }} 个差异区域
          </strong>
          <strong v-else-if="result">未发现显著差异</strong>
          <span v-if="result" class="difference-stats">
            显著差异像素 {{ formatRatio(result.changedPixelRatio) }} ·
            归一化预览 {{ result.width }} × {{ result.height }}
          </span>
        </div>
        <div class="sensitivity-control">
          <div class="sensitivity-heading">
            <span>差异灵敏度</span>
            <strong>{{ sensitivity }}</strong>
          </div>
          <el-slider
            :model-value="sensitivity"
            :min="0"
            :max="100"
            :disabled="loading"
            aria-label="差异灵敏度"
            @update:model-value="updateSensitivity"
            @change="$emit('refresh')"
          />
          <div class="sensitivity-labels">
            <span>忽略细碎差异</span>
            <span>捕捉细微差异</span>
          </div>
        </div>
      </div>

      <div v-if="error" class="difference-error" role="alert">
        <el-alert :title="`差异预览生成失败：${error}`" type="error" :closable="false" show-icon />
        <el-button
          type="primary"
          plain
          data-test="difference-retry"
          @click="$emit('retry')"
        >
          重试
        </el-button>
      </div>

      <div
        v-else
        v-loading="loading"
        element-loading-text="正在生成差异高亮…"
        class="preview-stage"
        :class="{ 'is-loading': loading }"
      >
        <template v-if="result">
          <figure class="preview-panel">
            <figcaption>
              <strong>标准图</strong>
              <span>{{ baselineName }}</span>
            </figcaption>
            <img :src="result.baselineDataUrl" :alt="`标准图 ${baselineName}`">
          </figure>
          <figure class="preview-panel">
            <figcaption>
              <strong>对比图</strong>
              <span>{{ candidateName }}</span>
            </figcaption>
            <img :src="result.candidateDataUrl" :alt="`对比图 ${candidateName}`">
          </figure>
          <figure class="preview-panel is-highlight">
            <figcaption>
              <strong>差异高亮</strong>
              <span>红色蒙层 + 黄色轮廓框</span>
            </figcaption>
            <img :src="result.highlightDataUrl" alt="差异高亮结果">
          </figure>
        </template>
      </div>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import type { TestDifferencePreviewResult } from '@/api/imageMetrics'

defineProps<{
  visible: boolean
  loading: boolean
  error: string
  result: TestDifferencePreviewResult | null
  baselineName: string
  candidateName: string
  sensitivity: number
}>()

const emit = defineEmits<{
  close: []
  retry: []
  refresh: []
  'update:sensitivity': [value: number]
}>()

function handleVisibility(visible: boolean) {
  if (!visible) emit('close')
}

function updateSensitivity(value: number | number[]) {
  emit('update:sensitivity', Array.isArray(value) ? value[0] : value)
}

function formatRatio(value: number) {
  return `${(value * 100).toFixed(value >= 0.01 ? 2 : 3)}%`
}
</script>

<style scoped>
.difference-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: min(620px, 70vh);
}

.difference-toolbar {
  display: grid;
  grid-template-columns: minmax(260px, 1fr) minmax(280px, 420px);
  align-items: center;
  gap: 24px;
  padding: 12px 16px;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #f8fafc;
}

.difference-stats {
  display: block;
  margin-top: 6px;
  color: #606266;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.sensitivity-heading,
.sensitivity-labels {
  display: flex;
  justify-content: space-between;
  gap: 16px;
}

.sensitivity-heading {
  color: #303133;
  font-size: 13px;
}

.sensitivity-labels {
  margin-top: -8px;
  color: #909399;
  font-size: 11px;
}

.difference-error {
  display: flex;
  align-items: center;
  gap: 12px;
}

.difference-error .el-alert {
  flex: 1;
}

.preview-stage {
  min-height: 440px;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.preview-stage.is-loading {
  border: 1px dashed #c0c4cc;
  border-radius: 8px;
}

.preview-panel {
  min-width: 0;
  margin: 0;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #f5f7fa;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.preview-panel.is-highlight {
  border-color: #e6a23c;
}

.preview-panel figcaption {
  min-height: 48px;
  padding: 10px 12px;
  box-sizing: border-box;
  background: #ffffff;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.preview-panel figcaption span {
  overflow: hidden;
  color: #909399;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-panel img {
  width: 100%;
  min-height: 0;
  flex: 1;
  object-fit: contain;
}

@media (max-width: 900px) {
  .difference-toolbar,
  .preview-stage {
    grid-template-columns: 1fr;
  }

  .preview-panel img {
    max-height: 55vh;
  }
}
</style>
