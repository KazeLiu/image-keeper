<template>
  <section class="file-panel" aria-labelledby="file-table-title">
    <header class="table-toolbar">
      <div class="table-title">
        <div>
          <h3 id="file-table-title">文件列表</h3>
          <el-tag size="small" effect="plain">{{ store.filteredMatches.length }}</el-tag>
        </div>
        <p>勾选需要处理的文件，可直接修改新名称。</p>
      </div>

      <div class="table-filters">
        <el-select v-model="referenceFilter" size="small" aria-label="按参考图片筛选">
          <el-option label="全部参考图" value="all">
            <div class="reference-option all-references-option">
              <span class="all-reference-thumbs" aria-hidden="true">
                <img
                  v-for="reference in store.references.slice(0, 3)"
                  :key="reference.id"
                  :src="convertFileSrc(reference.path)"
                  alt=""
                />
              </span>
              <span>全部参考图</span>
            </div>
          </el-option>
          <el-option
            v-for="reference in store.references"
            :key="reference.id"
            :label="reference.name"
            :value="reference.id"
          >
            <div class="reference-option">
              <img
                data-test="reference-option-image"
                :src="convertFileSrc(reference.path)"
                :alt="reference.name"
              />
              <span :title="reference.name">{{ reference.name }}</span>
            </div>
          </el-option>
        </el-select>
        <el-select v-model="store.classificationFilter" size="small" aria-label="按匹配类型筛选">
          <el-option label="全部类型" value="all" />
          <el-option v-for="item in classifications" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
        <el-button size="small" :disabled="!store.selectedPaths.length" @click="store.clearSelection">
          清空选择
        </el-button>
      </div>
    </header>

    <div v-if="store.errors.length" class="scan-warning" role="status">
      <el-icon><WarningFilled /></el-icon>
      {{ store.errors.length }} 个文件无法读取，其他结果不受影响
    </div>

    <div v-if="store.filteredMatches.length" class="file-table" role="table" aria-label="查找结果与重命名表格">
      <div class="table-row table-header" role="row">
        <span role="columnheader">
          <el-checkbox
            data-test="select-all"
            :model-value="allVisibleSelected"
            :indeterminate="someVisibleSelected && !allVisibleSelected"
            aria-label="选择全部当前结果"
            @change="toggleAllVisible"
          />
        </span>
        <span role="columnheader">图片</span>
        <span role="columnheader">原文件</span>
        <span role="columnheader">匹配类型</span>
        <span role="columnheader">新文件名</span>
        <span role="columnheader">状态</span>
      </div>

      <div
        v-for="item in store.filteredMatches"
        :key="item.filePath"
        class="table-row file-row"
        :class="{ selected: isSelected(item.filePath), blocking: previewFor(item.filePath)?.blocking }"
        data-test="file-row"
        role="row"
      >
        <div class="select-cell" role="cell">
          <el-checkbox
            data-test="file-select"
            :model-value="isSelected(item.filePath)"
            :aria-label="`选择 ${item.fileName}`"
            @change="store.toggleSelected(item.filePath, Boolean($event))"
          />
        </div>
        <div role="cell">
          <button type="button" class="thumb-button" :aria-label="`预览 ${item.fileName}`" @click="preview = item">
            <img :src="convertFileSrc(item.filePath)" :alt="item.fileName" loading="lazy" />
          </button>
        </div>
        <div class="file-cell" role="cell">
          <strong :title="item.fileName">{{ item.fileName }}</strong>
          <span :title="item.relativePath || item.filePath">{{ item.relativePath || item.filePath }}</span>
          <small>{{ item.width }} × {{ item.height }} · {{ formatBytes(item.fileSize) }} · {{ item.format.toUpperCase() }}</small>
        </div>
        <div class="match-cell" role="cell">
          <el-tag :type="classificationType(effectiveRelation(item).classification)" size="small">
            {{ classificationLabel(effectiveRelation(item).classification) }}
          </el-tag>
          <span>{{ store.referenceStem(effectiveRelation(item).referenceId) }} · {{ similarityFor(item) }}</span>
        </div>
        <div class="name-cell" role="cell">
          <el-input
            :model-value="editableName(item.filePath, item.fileName)"
            :disabled="!isSelected(item.filePath)"
            :aria-label="`${item.fileName} 的新名称`"
            @update:model-value="updateName(item.filePath, $event)"
            @blur="validateManualNames"
          />
        </div>
        <div class="status-cell" role="cell">
          <template v-if="!isSelected(item.filePath)">
            <span class="status muted">未选择</span>
          </template>
          <template v-else-if="!previewFor(item.filePath)">
            <span class="status muted">正在检查</span>
          </template>
          <template v-else-if="previewFor(item.filePath)!.blocking">
            <span class="status error"><el-icon><CircleCloseFilled /></el-icon>需修正</span>
            <small :title="issueText(item.filePath)">{{ issueText(item.filePath) }}</small>
          </template>
          <template v-else-if="previewFor(item.filePath)!.issues.length">
            <span class="status warning"><el-icon><WarningFilled /></el-icon>请注意</span>
            <small :title="issueText(item.filePath)">{{ issueText(item.filePath) }}</small>
          </template>
          <span v-else class="status success"><el-icon><CircleCheckFilled /></el-icon>可执行</span>
        </div>
      </div>
    </div>

    <el-empty
      v-else
      :description="store.matches.length ? '当前筛选下没有结果' : '没有找到相关文件，可返回修改查找范围'"
    />

    <div v-if="store.orderedSelectedMatches.length" class="batch-tools">
      <el-collapse v-model="expandedTools">
        <el-collapse-item name="rename-rules">
          <template #title>
            <div class="collapse-title">
              <strong>批量命名规则</strong>
              <span>需要统一编号或替换名称时展开</span>
            </div>
          </template>

          <div class="rule-panel">
            <el-tabs v-model="ruleMode" class="rule-tabs">
              <el-tab-pane label="简单模板" name="simple">
                <label for="simple-template">新名称格式</label>
                <div class="rule-input-row">
                  <el-input id="simple-template" v-model="simpleTemplate" placeholder="$ref-$n:02.$ext" />
                  <el-button type="primary" :loading="store.isPreviewing" @click="applyCurrentRule">应用规则</el-button>
                  <el-button :disabled="store.isRunning" @click="quickRename">按首项快速编号</el-button>
                </div>
                <div class="variable-chips" aria-label="可用变量">
                  <button v-for="variable in variables" :key="variable.token" type="button" @click="insertVariable(variable.token)">
                    <code>{{ variable.token }}</code><span>{{ variable.label }}</span>
                  </button>
                </div>
              </el-tab-pane>

              <el-tab-pane label="高级捕获" name="advanced">
                <div class="advanced-grid">
                  <label>原名称匹配<el-input v-model="oldPattern" placeholder="*_*.png" /></label>
                  <label>新名称格式<el-input v-model="advancedTemplate" placeholder="$2-$1.png" /></label>
                </div>
                <div class="advanced-action">
                  <span>每个 <code>*</code> 依次对应 <code>$1</code>、<code>$2</code>……</span>
                  <el-button type="primary" :loading="store.isPreviewing" @click="applyCurrentRule">应用高级规则</el-button>
                </div>
              </el-tab-pane>
            </el-tabs>

            <details class="rule-help">
              <summary>查看变量说明与示例</summary>
              <div class="example-grid">
                <div><strong>按参考图编号</strong><code>$ref-$n:02.$ext</code><span>三月七-01.png</span></div>
                <div><strong>添加固定前缀</strong><code>收藏-$name.$ext</code><span>收藏-立绘_微笑.png</span></div>
                <div><strong>交换两段</strong><code>*_*.png → $2-$1.png</code><span>表情_角色 → 角色-表情</span></div>
              </div>
            </details>
          </div>
        </el-collapse-item>
      </el-collapse>
    </div>

    <footer v-if="store.orderedSelectedMatches.length" class="operation-bar">
      <div class="operation-summary" aria-live="polite">
        已选择 <strong>{{ store.orderedSelectedMatches.length }}</strong> 个文件
        <span v-if="blockingCount" class="summary-error">· {{ blockingCount }} 个冲突</span>
      </div>
      <div class="operation-actions">
        <el-button :disabled="busy || store.isRunning" @click="copySelected">复制到…</el-button>
        <el-button :disabled="busy || store.isRunning" @click="moveSelected">新建文件夹并移动</el-button>
        <el-button v-if="lastBatch?.reversible" :disabled="busy || store.isRunning" @click="undoLast">撤销上次操作</el-button>
        <el-button
          type="primary"
          data-test="preview-rename"
          :loading="busy"
          :disabled="store.isRunning || store.isPreviewing || blockingCount > 0 || !renameReady"
          @click="openRenamePreview"
        >
          预览重命名
        </el-button>
      </div>
    </footer>

    <el-dialog
      v-model="renamePreviewVisible"
      width="min(760px, 86vw)"
      title="确认批量重命名"
      destroy-on-close
    >
      <div data-test="rename-preview-dialog" class="rename-preview-dialog">
        <p>将重命名 {{ pendingRenamePreview.length }} 个文件，请确认新名称。</p>
        <div class="rename-preview-table" role="table" aria-label="批量重命名确认列表">
          <div class="rename-preview-row rename-preview-header" role="row">
            <span role="columnheader">原名称</span>
            <span role="columnheader">新名称</span>
          </div>
          <div v-for="item in pendingRenamePreview" :key="item.sourcePath" class="rename-preview-row" role="row">
            <span role="cell" :title="item.originalName">{{ item.originalName }}</span>
            <span role="cell" :title="item.proposedName">{{ item.proposedName }}</span>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button :disabled="busy" @click="renamePreviewVisible = false">返回修改</el-button>
        <el-button type="primary" data-test="confirm-rename" :loading="busy" @click="confirmRename">
          确认重命名
        </el-button>
      </template>
    </el-dialog>

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
import { computed, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage, ElMessageBox } from 'element-plus'
import { CircleCheckFilled, CircleCloseFilled, WarningFilled } from '@element-plus/icons-vue'
import {
  copyDifferenceFiles,
  executeDifferenceRename,
  moveDifferenceFiles,
  previewDifferenceExplicitRename,
  previewDifferenceTransfer,
  undoDifferenceBatch,
  type DifferenceMatchItem,
  type MatchClassification,
  type OperationBatchResult,
  type RenamePreviewItem,
  type RenameRule
} from '@/api/differenceFinder'
import { formatSsim } from '@/features/similarity'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

const store = useDifferenceFinderStore()
const ruleMode = ref<'simple' | 'advanced'>('simple')
const simpleTemplate = ref('$name.$ext')
const oldPattern = ref('*_*.png')
const advancedTemplate = ref('$2-$1.png')
const editableNames = ref<Record<string, string>>({})
const manualPreview = ref<RenamePreviewItem[]>([])
const busy = ref(false)
const lastBatch = ref<OperationBatchResult | null>(null)
const hasAppliedRule = ref(false)
const manualOverrides = ref<Set<string>>(new Set())
const appliedRule = ref<RenameRule>({ mode: 'simple', template: '$name.$ext' })
const expandedTools = ref<string[]>([])
const preview = ref<DifferenceMatchItem | null>(null)
const renamePreviewVisible = ref(false)
const pendingRenamePreview = ref<RenamePreviewItem[]>([])
let validationTimer: number | null = null
let manualValidationGeneration = 0

const variables = [
  { token: '$name', label: '原名称' }, { token: '$ext', label: '扩展名' },
  { token: '$ref', label: '参考图' }, { token: '$n', label: '顺序' },
  { token: '$n:02', label: '01' }, { token: '$n:03', label: '001' },
  { token: '$group', label: '参考组' }
]
const classifications: Array<{ value: MatchClassification; label: string }> = [
  { value: 'exact', label: '完全相同' },
  { value: 'compressed_or_reencoded', label: '压缩/重编码' },
  { value: 'variant', label: '差分图' },
  { value: 'related_group', label: '相关组图' },
  { value: 'weak_candidate', label: '弱相关候选' }
]

const referenceFilter = computed({
  get: () => store.activeReferenceId || 'all',
  set: (value: string) => { store.setActiveReference(value === 'all' ? null : value) }
})
const displayRows = computed(() => manualPreview.value.length ? manualPreview.value : store.renamePreview)
const previewRows = computed(() => new Map(displayRows.value.map(item => [normalizePath(item.sourcePath), item])))
const blockingCount = computed(() => displayRows.value.filter(item => item.blocking).length)
const renameReady = computed(() => {
  const previewSources = displayRows.value.map(item => normalizePath(item.sourcePath)).sort()
  const selectedSources = store.orderedSelectedMatches.map(item => normalizePath(item.filePath)).sort()
  return previewSources.length > 0 && previewSources.join('|') === selectedSources.join('|')
})
const visibleSelectedCount = computed(() => store.filteredMatches.filter(item => isSelected(item.filePath)).length)
const allVisibleSelected = computed(() => store.filteredMatches.length > 0 && visibleSelectedCount.value === store.filteredMatches.length)
const someVisibleSelected = computed(() => visibleSelectedCount.value > 0)
const previewVisible = computed({ get: () => Boolean(preview.value), set: value => { if (!value) preview.value = null } })

watch(() => store.renamePreview, rows => {
  editableNames.value = Object.fromEntries(rows.map(item => [
    item.sourcePath,
    manualOverrides.value.has(normalizePath(item.sourcePath))
      ? editableNames.value[item.sourcePath] ?? item.proposedName
      : item.proposedName
  ]))
  manualPreview.value = rows
}, { deep: true })

watch(
  () => `${store.activeReferenceId || ''}::${store.orderedSelectedMatches
    .map(item => normalizePath(item.filePath))
    .sort()
    .join('|')}`,
  async () => {
    manualValidationGeneration += 1
    if (!store.orderedSelectedMatches.length) {
      hasAppliedRule.value = false
      store.renamePreview = []
      manualPreview.value = []
      editableNames.value = {}
      manualOverrides.value = new Set()
      return
    }
    await refreshAppliedRule()
  },
  { immediate: true }
)

function currentRule(): RenameRule {
  return ruleMode.value === 'simple'
    ? { mode: 'simple', template: simpleTemplate.value }
    : { mode: 'advanced', oldPattern: oldPattern.value, newTemplate: advancedTemplate.value }
}

async function applyCurrentRule() {
  hasAppliedRule.value = true
  manualOverrides.value = new Set()
  appliedRule.value = currentRule()
  await store.generateRenamePreview(appliedRule.value)
}

async function quickRename() {
  const first = displayRows.value[0]
  const firstName = first
    ? (editableNames.value[first.sourcePath] || first.proposedName)
    : store.orderedSelectedMatches[0]?.fileName
  if (!firstName) return
  hasAppliedRule.value = true
  manualOverrides.value = new Set()
  appliedRule.value = { mode: 'quick', firstName }
  await store.generateRenamePreview(appliedRule.value)
}

function insertVariable(token: string) { simpleTemplate.value += token }

function isSelected(path: string) {
  const key = normalizePath(path)
  return store.selectedPaths.some(item => normalizePath(item) === key)
}

function toggleAllVisible(value: boolean | string | number) {
  if (Boolean(value)) store.selectFiltered()
  else store.deselectFiltered()
}

function previewFor(path: string) {
  return previewRows.value.get(normalizePath(path))
}

function editableName(path: string, fallback: string) {
  const row = previewFor(path)
  return editableNames.value[path] ?? row?.proposedName ?? fallback
}

function issueText(path: string) {
  return previewFor(path)?.issues.map(issue => issue.message).join('；') || ''
}

function updateName(path: string, value: string | number) {
  if (!isSelected(path)) return
  manualOverrides.value = new Set(manualOverrides.value).add(normalizePath(path))
  editableNames.value = { ...editableNames.value, [path]: String(value) }
  if (validationTimer !== null) window.clearTimeout(validationTimer)
  validationTimer = window.setTimeout(validateManualNames, 220)
}

async function validateManualNames() {
  if (!displayRows.value.length) return []
  const generation = ++manualValidationGeneration
  const renamePreview = await previewDifferenceExplicitRename(displayRows.value.map(item => ({
    sourcePath: item.sourcePath,
    newName: editableNames.value[item.sourcePath] ?? item.proposedName,
    expectedFingerprint: fingerprintFor(item.sourcePath)
  })))
  if (generation === manualValidationGeneration) manualPreview.value = renamePreview
  return renamePreview
}

async function openRenamePreview() {
  const renamePreview = await validateManualNames()
  const previewSources = renamePreview.map(item => normalizePath(item.sourcePath)).sort()
  const selectedSources = store.orderedSelectedMatches.map(item => normalizePath(item.filePath)).sort()
  if (previewSources.join('|') !== selectedSources.join('|')) {
    ElMessage.error('选择内容已变化，请重新生成重命名预览')
    await refreshAppliedRule()
    return
  }
  if (renamePreview.some(item => item.blocking)) {
    ElMessage.error('请先处理标红的文件名冲突')
    return
  }
  pendingRenamePreview.value = renamePreview
  renamePreviewVisible.value = true
}

async function confirmRename() {
  const previewSources = pendingRenamePreview.value.map(item => normalizePath(item.sourcePath)).sort()
  const selectedSources = store.orderedSelectedMatches.map(item => normalizePath(item.filePath)).sort()
  if (previewSources.join('|') !== selectedSources.join('|')) {
    renamePreviewVisible.value = false
    ElMessage.error('选择内容已变化，请重新预览重命名')
    await refreshAppliedRule()
    return
  }
  busy.value = true
  try {
    const batch = await executeDifferenceRename(pendingRenamePreview.value.map(item => ({
      sourcePath: item.sourcePath,
      newName: item.proposedName,
      expectedFingerprint: fingerprintFor(item.sourcePath)
    })))
    lastBatch.value = batch
    store.applyOperationPaths(batch.entries)
    renamePreviewVisible.value = false
    pendingRenamePreview.value = []
    ElMessage.success(`重命名完成：${batch.succeeded} 个成功`)
    await refreshAppliedRule()
  } finally { busy.value = false }
}

async function moveSelected() {
  const parent = await open({ directory: true, multiple: false, title: '选择新文件夹的父目录' })
  if (!parent || Array.isArray(parent)) return
  const { value } = await ElMessageBox.prompt('输入要创建的文件夹名称', '新建文件夹并移动', {
    inputPlaceholder: '例如：三月七差分图',
    inputValidator: value => {
      const name = value.trim()
      if (!name) return '请输入文件夹名称'
      if (/[<>:"/\\|?*]/.test(name) || /[ .]$/.test(name)) return '文件夹名称包含 Windows 不允许的字符'
      return true
    }
  })
  const destination = `${parent.replace(/[\\/]$/, '')}\\${value.trim()}`
  const request = { files: selectedTransferFiles(), targetDirectory: parent, newFolderName: value.trim() }
  const transferPreview = await previewDifferenceTransfer(request)
  if (transferPreview.items.some(item => item.issues.some(issue => ['source_missing', 'source_changed'].includes(issue.kind)))) {
    ElMessage.error('部分文件在搜索后已变化，请重新搜索后再移动')
    return
  }
  await ElMessageBox.confirm(
    transferConfirmation('移动', transferPreview.destination || destination, transferPreview.conflictCount, transferPreview.items.map(item => item.targetPath)),
    '确认新建文件夹并移动',
    { confirmButtonText: '确认移动', cancelButtonText: '取消', type: 'warning' }
  )
  busy.value = true
  try {
    const batch = await moveDifferenceFiles(request)
    lastBatch.value = batch
    store.applyOperationPaths(batch.entries)
    ElMessage.success(`移动完成：${batch.succeeded} 个成功，${batch.skipped} 个跳过`)
    await refreshAppliedRule()
  } finally { busy.value = false }
}

async function copySelected() {
  const target = await open({ directory: true, multiple: false, title: '选择复制目标目录' })
  if (!target || Array.isArray(target)) return
  const request = { files: selectedTransferFiles(), targetDirectory: target }
  const transferPreview = await previewDifferenceTransfer(request)
  if (transferPreview.items.some(item => item.issues.some(issue => ['source_missing', 'source_changed'].includes(issue.kind)))) {
    ElMessage.error('部分文件在搜索后已变化，请重新搜索后再复制')
    return
  }
  await ElMessageBox.confirm(
    transferConfirmation('复制', transferPreview.destination, transferPreview.conflictCount, transferPreview.items.map(item => item.targetPath)),
    '确认批量复制',
    { confirmButtonText: '确认复制', cancelButtonText: '取消', type: 'warning' }
  )
  busy.value = true
  try {
    const batch = await copyDifferenceFiles(request)
    ElMessage.success(`复制完成：${batch.succeeded} 个成功，${batch.skipped} 个跳过`)
  } finally { busy.value = false }
}

async function undoLast() {
  if (!lastBatch.value) return
  busy.value = true
  try {
    const batch = await undoDifferenceBatch(lastBatch.value.batchId)
    store.applyOperationPaths(batch.entries)
    lastBatch.value = null
    ElMessage.success(`已撤销 ${batch.succeeded} 个文件操作`)
    await refreshAppliedRule()
  } finally { busy.value = false }
}

function fingerprintFor(path: string) {
  const match = store.matches.find(item => normalizePath(item.filePath) === normalizePath(path))
  if (!match) throw new Error(`找不到文件的搜索指纹: ${path}`)
  return { blake3Hash: match.blake3Hash, fileSize: match.fileSize, modifiedAt: match.modifiedAt }
}

function selectedTransferFiles() {
  return store.orderedSelectedMatches.map(item => ({
    sourcePath: item.filePath,
    expectedFingerprint: fingerprintFor(item.filePath)
  }))
}

function normalizePath(path: string) {
  return path.replace(/\//g, '\\').toLowerCase()
}

async function refreshAppliedRule() {
  await store.generateRenamePreview(
    hasAppliedRule.value ? appliedRule.value : { mode: 'simple', template: '$name.$ext' }
  )
}

function transferConfirmation(action: string, destination: string, conflicts: number, targets: string[]) {
  const examples = targets.slice(0, 3).join('；')
  const remaining = Math.max(0, targets.length - 3)
  return `将${action} ${targets.length} 个文件到 ${destination}。检测到 ${conflicts} 个冲突（会安全跳过）。目标示例：${examples}${remaining ? `；另有 ${remaining} 个` : ''}`
}

function classificationLabel(value: MatchClassification) {
  return classifications.find(item => item.value === value)?.label || value
}

function classificationType(value: MatchClassification) {
  return value === 'exact' ? 'success' : value === 'variant' ? 'primary' : value === 'weak_candidate' ? 'warning' : 'info'
}

function similarityFor(item: DifferenceMatchItem) {
  const relation = effectiveRelation(item)
  return relation?.similarity == null ? '—' : formatSsim(relation.similarity)
}

function effectiveRelation(item: DifferenceMatchItem) {
  return (store.activeReferenceId
    ? item.relations.find(relation => relation.referenceId === store.activeReferenceId)
    : undefined)
    || item.relations.find(relation => relation.referenceId === item.bestReferenceId)
    || item.relations[0]
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 ** 2).toFixed(1)} MB`
}
</script>

<style scoped>
.file-panel {
  min-height: 0;
  flex: 1;
  border: 1px solid #dcdfe6;
  border-radius: 8px;
  background: #fff;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.table-toolbar {
  padding: 12px 16px;
  border-bottom: 1px solid #ebeef5;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.table-title > div { display: flex; align-items: center; gap: 8px; }
.table-title h3 { margin: 0; font-size: 16px; }
.table-title p { margin: 3px 0 0; color: #606266; font-size: 12px; }
.table-filters { display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
.table-filters .el-select:first-child { width: 174px; }
.table-filters .el-select:nth-child(2) { width: 150px; }
.reference-option { min-width: 0; display: flex; align-items: center; gap: 9px; }
.reference-option > img { width: 32px; height: 32px; border-radius: 4px; object-fit: cover; background: #f2f3f5; }
.reference-option > span:last-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.all-reference-thumbs { width: 48px; height: 32px; position: relative; flex: 0 0 auto; }
.all-reference-thumbs img { position: absolute; top: 0; width: 32px; height: 32px; border: 2px solid #fff; border-radius: 4px; object-fit: cover; background: #f2f3f5; }
.all-reference-thumbs img:nth-child(1) { left: 0; z-index: 3; }
.all-reference-thumbs img:nth-child(2) { left: 8px; z-index: 2; }
.all-reference-thumbs img:nth-child(3) { left: 16px; z-index: 1; }

.scan-warning {
  margin: 10px 16px 0;
  padding: 8px 10px;
  border-radius: 6px;
  background: #fdf6ec;
  color: #946321;
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
}

.file-table { min-height: 0; flex: 1; overflow-y: auto; }
.table-row {
  min-width: 900px;
  display: grid;
  grid-template-columns: 42px 56px minmax(210px, 1.35fr) minmax(140px, .72fr) minmax(220px, 1.1fr) minmax(100px, .55fr);
  gap: 10px;
  align-items: center;
}

.table-header {
  position: sticky;
  top: 0;
  z-index: 2;
  min-height: 36px;
  padding: 0 14px;
  border-bottom: 1px solid #dcdfe6;
  background: #f5f7fa;
  color: #606266;
  font-size: 11px;
  font-weight: 650;
}

.file-row {
  min-height: 70px;
  padding: 7px 14px;
  border-bottom: 1px solid #ebeef5;
  transition: background-color .18s, box-shadow .18s;
}

.file-row.selected { background: #f4f8ff; box-shadow: inset 3px 0 #409eff; }
.file-row.blocking { background: #fff5f5; box-shadow: inset 3px 0 #d03050; }
.select-cell { display: flex; justify-content: center; }
.thumb-button { width: 48px; height: 48px; padding: 0; border: 0; border-radius: 5px; overflow: hidden; background: #f2f3f5; cursor: pointer; }
.thumb-button:focus-visible { outline: 2px solid #409eff; outline-offset: 2px; }
.thumb-button img { width: 100%; height: 100%; display: block; object-fit: cover; }

.file-cell,
.match-cell,
.status-cell { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.file-cell strong,
.file-cell span,
.status-cell small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-cell strong { font-size: 12px; font-weight: 650; }
.file-cell span { color: #606266; font-size: 11px; }
.file-cell small { color: #909399; font-size: 10px; font-variant-numeric: tabular-nums; }
.match-cell { align-items: flex-start; }
.match-cell span { color: #606266; font-size: 10px; }
.name-cell { min-width: 0; }
.name-cell :deep(.el-input.is-disabled .el-input__wrapper) { background: #f5f7fa; }
.name-cell :deep(.el-input.is-disabled .el-input__inner) { color: #909399; -webkit-text-fill-color: #909399; }
.status { display: flex; align-items: center; gap: 4px; font-size: 11px; font-weight: 650; }
.status.error,
.summary-error { color: #c9344e; }
.status.warning { color: #946321; }
.status.success { color: #3f7d20; }
.status.muted { color: #909399; font-weight: 500; }
.status-cell small { color: #909399; font-size: 10px; }

.batch-tools { border-top: 1px solid #ebeef5; background: #fafcff; }
.batch-tools :deep(.el-collapse) { border: 0; }
.batch-tools :deep(.el-collapse-item__header) { height: 44px; padding: 0 16px; background: #fafcff; }
.batch-tools :deep(.el-collapse-item__wrap) { background: #fafcff; }
.batch-tools :deep(.el-collapse-item__content) { padding-bottom: 12px; }
.collapse-title { display: flex; align-items: baseline; gap: 10px; }
.collapse-title strong { color: #303133; font-size: 13px; }
.collapse-title span { color: #909399; font-size: 11px; font-weight: 400; }
.rule-panel { padding: 0 16px; }
.rule-tabs :deep(.el-tabs__header) { margin-bottom: 10px; }
.rule-panel label { display: block; margin-bottom: 6px; color: #606266; font-size: 12px; font-weight: 600; }
.rule-input-row { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 8px; }
.variable-chips { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 6px; }
.variable-chips button { padding: 4px 7px; border: 1px solid #dcdfe6; border-radius: 5px; background: #fff; color: #606266; display: inline-flex; gap: 5px; cursor: pointer; font-size: 11px; }
.variable-chips button:hover,
.variable-chips button:focus-visible { border-color: #409eff; color: #409eff; }
.variable-chips code { color: #337ecc; }
.advanced-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.advanced-action { display: flex; align-items: center; justify-content: space-between; gap: 12px; color: #606266; font-size: 11px; }
.rule-help { margin-top: 10px; color: #606266; font-size: 11px; }
.rule-help summary { cursor: pointer; }
.example-grid { margin-top: 8px; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 7px; }
.example-grid > div { padding: 8px; border: 1px solid #ebeef5; border-radius: 6px; background: #fff; display: flex; flex-direction: column; gap: 3px; }
.example-grid code { color: #337ecc; word-break: break-all; }
.example-grid span { color: #909399; }

.operation-bar { padding: 11px 16px; border-top: 1px solid #dcdfe6; background: #fff; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.operation-summary { color: #606266; font-size: 12px; }
.operation-summary strong { color: #303133; font-size: 16px; font-variant-numeric: tabular-nums; }
.operation-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 7px; }
.rename-preview-dialog > p { margin: 0 0 12px; color: #606266; font-size: 13px; }
.rename-preview-table { max-height: min(430px, 54vh); border: 1px solid #dcdfe6; border-radius: 6px; overflow-y: auto; }
.rename-preview-row { min-height: 42px; padding: 8px 12px; border-bottom: 1px solid #ebeef5; display: grid; grid-template-columns: minmax(0, 1fr) 28px minmax(0, 1fr); align-items: center; gap: 8px; font-size: 12px; }
.rename-preview-row::after { content: '→'; grid-column: 2; grid-row: 1; color: #909399; text-align: center; }
.rename-preview-row > span:first-child { grid-column: 1; }
.rename-preview-row > span:last-child { grid-column: 3; color: #337ecc; font-weight: 600; }
.rename-preview-row > span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rename-preview-header { position: sticky; top: 0; z-index: 1; min-height: 34px; background: #f5f7fa; color: #606266; font-size: 11px; font-weight: 650; }
.rename-preview-header::after { content: ''; }
.rename-preview-header > span:last-child { color: #606266; }
.rename-preview-row:last-child { border-bottom: 0; }
.preview-dialog { display: grid; grid-template-columns: minmax(0, 1fr) 220px; gap: 18px; }
.preview-dialog img { width: 100%; max-height: 68vh; object-fit: contain; background: #f2f3f5; }
.preview-dialog h3 { margin-top: 0; word-break: break-all; }
.preview-dialog p { color: #606266; word-break: break-all; }

@media (max-width: 1080px) {
  .table-toolbar { align-items: flex-start; }
  .table-filters { flex-wrap: wrap; }
  .table-row { grid-template-columns: 38px 52px minmax(190px, 1.2fr) 132px minmax(190px, 1fr) 92px; }
  .rule-input-row { grid-template-columns: minmax(0, 1fr) auto; }
  .rule-input-row .el-button:last-child { grid-column: 1 / -1; justify-self: end; }
  .example-grid { grid-template-columns: 1fr; }
}

@media (prefers-reduced-motion: reduce) {
  .file-row { transition: none; }
}
</style>
