<template>
  <section class="result-panel" aria-labelledby="result-title">
    <div class="result-heading">
      <div>
        <span class="step">03</span>
        <h2 id="result-title">匹配结果</h2>
        <el-tag size="small" effect="plain">{{ store.filteredMatches.length }}</el-tag>
      </div>
      <div class="result-actions">
        <el-button size="small" :disabled="store.isRunning || !store.filteredMatches.length" @click="store.selectFiltered">选择当前结果</el-button>
        <el-button size="small" :disabled="store.isRunning || !store.selectedPaths.length" @click="store.clearSelection">清空选择</el-button>
      </div>
    </div>

    <div class="filter-row">
      <el-select v-model="store.classificationFilter" size="small" aria-label="按匹配类型筛选">
        <el-option label="全部类型" value="all" />
        <el-option v-for="item in classifications" :key="item.value" :label="item.label" :value="item.value" />
      </el-select>
      <span>已选择 {{ store.selectedPaths.length }} 张</span>
    </div>

    <div v-if="store.errors.length" class="scan-warning" role="status">
      <el-icon><Warning /></el-icon>
      {{ store.errors.length }} 个文件无法读取，未影响其他结果
    </div>

    <div v-if="store.filteredMatches.length" class="result-list">
      <article
        v-for="item in store.filteredMatches"
        :key="item.filePath"
        class="result-item"
        :class="{ selected: isSelected(item.filePath) }"
      >
        <el-checkbox
          :model-value="isSelected(item.filePath)"
          :aria-label="`选择 ${item.fileName}`"
          @change="handleSelectionChange(item.filePath, $event)"
        />
        <button type="button" class="thumb-button" :aria-label="`预览 ${item.fileName}`" @click="preview = item">
          <img :src="convertFileSrc(item.filePath)" :alt="item.fileName" loading="lazy" />
        </button>
        <div class="result-main">
          <div class="name-row">
            <strong :title="item.fileName">{{ item.fileName }}</strong>
            <el-tag :type="classificationType(store.classificationForItem(item))" size="small">
              {{ classificationLabel(store.classificationForItem(item)) }}
            </el-tag>
          </div>
          <p :title="item.filePath">{{ item.relativePath || item.filePath }}</p>
          <div class="meta-row">
            <span>{{ item.width }} × {{ item.height }}</span>
            <span>{{ formatBytes(item.fileSize) }}</span>
            <span>{{ item.format.toUpperCase() }}</span>
          </div>
          <div class="relation-row">
            <el-tag
              v-for="relation in item.relations.slice(0, 3)"
              :key="relation.referenceId"
              size="small"
              effect="plain"
              :title="relation.referencePath"
            >
              {{ store.referenceStem(relation.referenceId) }} · {{ formatSimilarity(relation.similarity) }}
            </el-tag>
          </div>
        </div>
      </article>
    </div>

    <el-empty v-else :description="store.matches.length ? '当前筛选下没有结果' : '添加参考图和目录后开始查找'" />

    <el-dialog v-model="previewVisible" width="min(900px, 84vw)" title="图片预览" destroy-on-close>
      <div v-if="preview" class="preview-dialog">
        <img :src="convertFileSrc(preview.filePath)" :alt="preview.fileName" />
        <div>
          <h3>{{ preview.fileName }}</h3>
          <p>{{ preview.filePath }}</p>
          <p>{{ preview.width }} × {{ preview.height }} · {{ formatBytes(preview.fileSize) }}</p>
        </div>
      </div>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Warning } from '@element-plus/icons-vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'
import type { DifferenceMatchItem, MatchClassification } from '@/api/differenceFinder'

const store = useDifferenceFinderStore()
const preview = ref<DifferenceMatchItem | null>(null)
const previewVisible = computed({ get: () => Boolean(preview.value), set: value => { if (!value) preview.value = null } })
const classifications: Array<{ value: MatchClassification; label: string }> = [
  { value: 'exact', label: '完全相同' },
  { value: 'compressed_or_reencoded', label: '压缩/重编码' },
  { value: 'variant', label: '差分图' },
  { value: 'related_group', label: '相关组图' },
  { value: 'weak_candidate', label: '弱相关候选' }
]

function isSelected(path: string) {
  const key = path.replace(/\//g, '\\').toLowerCase()
  return store.selectedPaths.some(item => item.replace(/\//g, '\\').toLowerCase() === key)
}
function handleSelectionChange(path: string, value: boolean | string | number) {
  store.toggleSelected(path, Boolean(value))
}
function classificationLabel(value: MatchClassification) { return classifications.find(item => item.value === value)?.label || value }
function classificationType(value: MatchClassification) {
  return value === 'exact' ? 'success' : value === 'variant' ? 'primary' : value === 'weak_candidate' ? 'warning' : 'info'
}
function formatSimilarity(value?: number | null) { return value == null ? '—' : `${(value * 100).toFixed(1)}%` }
function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 ** 2).toFixed(1)} MB`
}
</script>

<style scoped>
.result-panel { min-height: 0; border: 1px solid #dcdfe6; border-radius: 10px; background: #fff; display: flex; flex-direction: column; overflow: hidden; }
.result-heading { padding: 16px 18px 12px; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.result-heading > div:first-child { display: flex; align-items: center; gap: 8px; }
.result-heading h2 { margin: 0; font-size: 17px; }
.step { color: #409eff; font-size: 12px; font-weight: 800; }
.result-actions { display: flex; gap: 6px; }
.filter-row { padding: 0 18px 12px; display: flex; align-items: center; justify-content: space-between; gap: 12px; color: #606266; font-size: 12px; }
.filter-row .el-select { width: 152px; }
.scan-warning { margin: 0 18px 10px; padding: 8px 10px; border-radius: 6px; background: #fdf6ec; color: #b88230; display: flex; align-items: center; gap: 7px; font-size: 12px; }
.result-list { min-height: 0; padding: 0 10px 12px; display: flex; flex: 1; flex-direction: column; gap: 7px; overflow-y: auto; }
.result-item { padding: 9px; border: 1px solid #ebeef5; border-radius: 8px; display: grid; grid-template-columns: 28px 72px minmax(0, 1fr); align-items: start; gap: 9px; transition: border-color .18s, background-color .18s; }
.result-item.selected { border-color: #409eff; background: #f5f9ff; }
.thumb-button { width: 72px; height: 72px; padding: 0; border: 0; border-radius: 6px; overflow: hidden; background: #f2f3f5; cursor: pointer; }
.thumb-button:focus-visible { outline: 2px solid #409eff; outline-offset: 2px; }
.thumb-button img { width: 100%; height: 100%; display: block; object-fit: cover; }
.result-main { min-width: 0; }
.name-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.name-row strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
.result-main p { margin: 5px 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #909399; font-size: 11px; }
.meta-row { display: flex; gap: 10px; color: #606266; font-size: 11px; font-variant-numeric: tabular-nums; }
.relation-row { margin-top: 7px; display: flex; gap: 5px; overflow: hidden; }
.preview-dialog { display: grid; grid-template-columns: minmax(0, 1fr) 220px; gap: 18px; }
.preview-dialog img { width: 100%; max-height: 68vh; object-fit: contain; background: #f2f3f5; }
.preview-dialog h3 { margin-top: 0; word-break: break-all; }
.preview-dialog p { color: #606266; word-break: break-all; }
</style>
