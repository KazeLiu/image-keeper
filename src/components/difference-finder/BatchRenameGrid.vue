<template>
  <section class="organizer-panel" aria-labelledby="organizer-title">
    <div class="organizer-heading">
      <div>
        <span class="step">04</span>
        <h2 id="organizer-title">批量整理</h2>
        <el-tag size="small" effect="plain">{{ store.orderedSelectedMatches.length }}</el-tag>
      </div>
      <el-button
        type="primary"
        plain
        size="small"
        :disabled="store.isRunning || !store.orderedSelectedMatches.length"
        @click="quickRename"
      >
        按首项快速编号
      </el-button>
    </div>

    <template v-if="store.orderedSelectedMatches.length">
      <div class="rule-panel">
        <el-tabs v-model="ruleMode" class="rule-tabs">
          <el-tab-pane label="简单模板" name="simple">
            <label for="simple-template">新名称格式</label>
            <div class="rule-input-row">
              <el-input id="simple-template" v-model="simpleTemplate" placeholder="$ref-$n:02.$ext" />
              <el-button type="primary" :loading="store.isPreviewing" @click="applyCurrentRule">应用</el-button>
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

        <el-collapse class="rule-help">
          <el-collapse-item title="规则说明与示例" name="help">
            <div class="example-grid">
              <div><strong>按参考图编号</strong><code>$ref-$n:02.$ext</code><span>三月七-01.png</span></div>
              <div><strong>添加固定前缀</strong><code>收藏-$name.$ext</code><span>收藏-立绘_微笑.png</span></div>
              <div><strong>三位顺序号</strong><code>$name-$n:03.$ext</code><span>立绘-001.png</span></div>
              <div><strong>交换两段</strong><code>*_*.png → $2-$1.png</code><span>表情_角色 → 角色-表情</span></div>
              <div><strong>保留首尾片段</strong><code>*-*-*.jpg → $1_差分_$3.jpg</code><span>流萤-表情-07 → 流萤_差分_07</span></div>
              <div><strong>捕获后重编号</strong><code>*_*.webp → $1-$n:02.$ext</code><span>卡芙卡_旧编号 → 卡芙卡-01</span></div>
            </div>
          </el-collapse-item>
        </el-collapse>
        <p class="quick-help">快速重命名以第一行当前名称为基础，按拖拽顺序生成 <code>_1</code>、<code>_2</code>……并保留每张图原扩展名。</p>
      </div>

      <div class="rename-grid" role="table" aria-label="批量重命名预览">
        <div class="rename-header" role="row">
          <span>顺序</span><span>图片</span><span>原名称</span><span>新名称</span><span>状态</span>
        </div>
        <div
          v-for="(item, index) in displayRows"
          :key="item.sourcePath"
          class="rename-row"
          :class="{ blocking: item.blocking }"
          role="row"
          draggable="true"
          @dragstart="dragStart = index"
          @dragover.prevent
          @drop="dropAt(index)"
        >
          <div class="order-cell">
            <button type="button" class="drag-handle" title="拖拽排序" aria-label="拖拽排序"><el-icon><Rank /></el-icon></button>
            <span>{{ index + 1 }}</span>
            <span class="order-buttons">
              <el-button :icon="ArrowUp" text circle size="small" :disabled="index === 0" aria-label="上移" @click="moveRow(index, index - 1)" />
              <el-button :icon="ArrowDown" text circle size="small" :disabled="index === displayRows.length - 1" aria-label="下移" @click="moveRow(index, index + 1)" />
            </span>
          </div>
          <img class="rename-thumb" :src="convertFileSrc(item.sourcePath)" :alt="item.originalName" />
          <span class="original-name" :title="item.originalName">{{ item.originalName }}</span>
          <el-input
            :model-value="editableNames[item.sourcePath] ?? item.proposedName"
            :aria-label="`${item.originalName} 的新名称`"
            @update:model-value="updateName(item.sourcePath, $event)"
            @blur="validateManualNames"
          />
          <div class="status-cell">
            <span v-if="item.blocking" class="status error"><el-icon><CircleCloseFilled /></el-icon>需修正</span>
            <span v-else-if="item.issues.length" class="status warning"><el-icon><WarningFilled /></el-icon>{{ item.issues[0].message }}</span>
            <span v-else class="status success"><el-icon><CircleCheckFilled /></el-icon>可执行</span>
            <small v-if="item.issues.length" :title="item.issues.map(issue => issue.message).join('；')">
              {{ item.issues.map(issue => issue.message).join('；') }}
            </small>
          </div>
        </div>
      </div>

      <div class="operation-bar">
        <div class="operation-summary" aria-live="polite">
          <strong>{{ displayRows.length }}</strong> 个文件
          <span v-if="blockingCount" class="summary-error">· {{ blockingCount }} 个冲突</span>
        </div>
        <div class="operation-actions">
          <el-button :disabled="busy || store.isRunning" @click="copySelected">复制到…</el-button>
          <el-button :disabled="busy || store.isRunning" @click="moveSelected">新建文件夹并移动</el-button>
          <el-button v-if="lastBatch?.reversible" :disabled="busy || store.isRunning" @click="undoLast">撤销上次操作</el-button>
          <el-button type="primary" :loading="busy" :disabled="store.isRunning || blockingCount > 0" @click="executeRename">执行重命名</el-button>
        </div>
      </div>
    </template>

    <el-empty v-else description="从左侧结果中选择要整理的图片" />
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowDown, ArrowUp, CircleCheckFilled, CircleCloseFilled, Rank, WarningFilled
} from '@element-plus/icons-vue'
import {
  copyDifferenceFiles,
  executeDifferenceRename,
  moveDifferenceFiles,
  previewDifferenceExplicitRename,
  previewDifferenceTransfer,
  undoDifferenceBatch,
  type OperationBatchResult,
  type RenamePreviewItem,
  type RenameRule
} from '@/api/differenceFinder'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

const store = useDifferenceFinderStore()
const ruleMode = ref<'simple' | 'advanced'>('simple')
const simpleTemplate = ref('$name.$ext')
const oldPattern = ref('*_*.png')
const advancedTemplate = ref('$2-$1.png')
const editableNames = ref<Record<string, string>>({})
const manualPreview = ref<RenamePreviewItem[]>([])
const dragStart = ref<number | null>(null)
const busy = ref(false)
const lastBatch = ref<OperationBatchResult | null>(null)
const hasAppliedRule = ref(false)
const manualOverrides = ref<Set<string>>(new Set())
const appliedRule = ref<RenameRule>({ mode: 'simple', template: '$name.$ext' })
let validationTimer: number | null = null

const variables = [
  { token: '$name', label: '原名称' }, { token: '$ext', label: '扩展名' },
  { token: '$ref', label: '参考图' }, { token: '$n', label: '顺序' },
  { token: '$n:02', label: '01' }, { token: '$n:03', label: '001' },
  { token: '$group', label: '参考组' }
]
const displayRows = computed(() => manualPreview.value.length ? manualPreview.value : store.renamePreview)
const blockingCount = computed(() => displayRows.value.filter(item => item.blocking).length)

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
  () => store.orderedSelectedMatches
    .map(item => item.filePath.replace(/\//g, '\\').toLowerCase())
    .sort()
    .join('|'),
  async signature => {
    if (!signature) {
      hasAppliedRule.value = false
      store.renamePreview = []
      manualPreview.value = []
      editableNames.value = {}
      manualOverrides.value = new Set()
      return
    }
    await store.generateRenamePreview(
      hasAppliedRule.value ? appliedRule.value : { mode: 'simple', template: '$name.$ext' }
    )
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
  const firstName = first ? (editableNames.value[first.sourcePath] || first.proposedName) : store.orderedSelectedMatches[0]?.fileName
  if (!firstName) return
  hasAppliedRule.value = true
  manualOverrides.value = new Set()
  appliedRule.value = { mode: 'quick', firstName }
  await store.generateRenamePreview(appliedRule.value)
}

function insertVariable(token: string) { simpleTemplate.value += token }

function updateName(path: string, value: string | number) {
  manualOverrides.value = new Set(manualOverrides.value).add(normalizePath(path))
  editableNames.value = { ...editableNames.value, [path]: String(value) }
  if (validationTimer !== null) window.clearTimeout(validationTimer)
  validationTimer = window.setTimeout(validateManualNames, 220)
}

async function validateManualNames() {
  if (!displayRows.value.length) return []
  manualPreview.value = await previewDifferenceExplicitRename(displayRows.value.map(item => ({
    sourcePath: item.sourcePath,
    newName: editableNames.value[item.sourcePath] ?? item.proposedName,
    expectedFingerprint: fingerprintFor(item.sourcePath)
  })))
  return manualPreview.value
}

async function dropAt(index: number) {
  if (dragStart.value === null || dragStart.value === index) return
  store.reorderSelected(dragStart.value, index)
  dragStart.value = null
  await refreshAppliedRule()
}

async function moveRow(from: number, to: number) {
  store.reorderSelected(from, to)
  await refreshAppliedRule()
}

async function executeRename() {
  const preview = await validateManualNames()
  const previewSources = preview.map(item => normalizePath(item.sourcePath)).sort()
  const selectedSources = store.orderedSelectedMatches.map(item => normalizePath(item.filePath)).sort()
  if (previewSources.join('|') !== selectedSources.join('|')) {
    ElMessage.error('选择内容已变化，请重新生成重命名预览')
    await refreshAppliedRule()
    return
  }
  if (preview.some(item => item.blocking)) {
    ElMessage.error('请先处理标红的文件名冲突')
    return
  }
  await ElMessageBox.confirm(
    `将重命名 ${preview.length} 个文件。不会覆盖已有同名文件。`,
    '确认批量重命名',
    { confirmButtonText: '执行重命名', cancelButtonText: '返回检查', type: 'warning' }
  )
  busy.value = true
  try {
    const batch = await executeDifferenceRename(preview.map(item => ({
      sourcePath: item.sourcePath,
      newName: editableNames.value[item.sourcePath] || item.proposedName,
      expectedFingerprint: fingerprintFor(item.sourcePath)
    })))
    lastBatch.value = batch
    store.applyOperationPaths(batch.entries)
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
  const preview = await previewDifferenceTransfer(request)
  if (preview.items.some(item => item.issues.some(issue => ['source_missing', 'source_changed'].includes(issue.kind)))) {
    ElMessage.error('部分文件在搜索后已变化，请重新搜索后再移动')
    return
  }
  await ElMessageBox.confirm(
    transferConfirmation('移动', preview.destination || destination, preview.conflictCount, preview.items.map(item => item.targetPath)),
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
  const preview = await previewDifferenceTransfer(request)
  if (preview.items.some(item => item.issues.some(issue => ['source_missing', 'source_changed'].includes(issue.kind)))) {
    ElMessage.error('部分文件在搜索后已变化，请重新搜索后再复制')
    return
  }
  await ElMessageBox.confirm(
    transferConfirmation('复制', preview.destination, preview.conflictCount, preview.items.map(item => item.targetPath)),
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
</script>

<style scoped>
.organizer-panel { min-height: 0; border: 1px solid #dcdfe6; border-radius: 10px; background: #fff; display: flex; flex-direction: column; overflow: hidden; }
.organizer-heading { padding: 16px 18px 10px; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.organizer-heading > div { display: flex; align-items: center; gap: 8px; }
.organizer-heading h2 { margin: 0; font-size: 17px; }
.step { color: #409eff; font-size: 12px; font-weight: 800; }
.rule-panel { margin: 0 14px 10px; padding: 10px 12px; border: 1px solid #ebeef5; border-radius: 8px; background: #fafcff; }
.rule-tabs :deep(.el-tabs__header) { margin-bottom: 10px; }
.rule-panel label { display: block; margin-bottom: 6px; color: #606266; font-size: 12px; font-weight: 600; }
.rule-input-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; }
.variable-chips { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 6px; }
.variable-chips button { padding: 4px 7px; border: 1px solid #dcdfe6; border-radius: 5px; background: #fff; color: #606266; display: inline-flex; gap: 5px; cursor: pointer; font-size: 11px; }
.variable-chips button:hover, .variable-chips button:focus-visible { border-color: #409eff; color: #409eff; }
.variable-chips code { color: #409eff; }
.advanced-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.advanced-action { display: flex; align-items: center; justify-content: space-between; gap: 12px; color: #606266; font-size: 11px; }
.rule-help { margin-top: 8px; }
.example-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; }
.example-grid > div { padding: 8px; border: 1px solid #ebeef5; border-radius: 6px; background: #fff; display: flex; flex-direction: column; gap: 3px; font-size: 11px; }
.example-grid code { color: #409eff; word-break: break-all; }
.example-grid span { color: #909399; }
.quick-help { margin: 8px 0 0; color: #606266; font-size: 11px; line-height: 1.5; }
.rename-grid { min-height: 0; margin: 0 10px; flex: 1; overflow-y: auto; }
.rename-header, .rename-row { display: grid; grid-template-columns: 94px 58px minmax(110px, .8fr) minmax(170px, 1.2fr) minmax(110px, .7fr); gap: 8px; align-items: center; }
.rename-header { position: sticky; top: 0; z-index: 2; padding: 7px 9px; border-bottom: 1px solid #dcdfe6; background: #f5f7fa; color: #606266; font-size: 11px; font-weight: 650; }
.rename-row { min-height: 64px; padding: 7px 9px; border-bottom: 1px solid #ebeef5; background: #fff; }
.rename-row.blocking { background: #fff5f5; }
.order-cell { display: flex; align-items: center; gap: 3px; font-variant-numeric: tabular-nums; }
.drag-handle { width: 30px; height: 36px; border: 0; background: transparent; color: #909399; cursor: grab; }
.order-buttons { display: flex; }
.rename-thumb { width: 52px; height: 52px; border-radius: 5px; object-fit: cover; background: #f2f3f5; }
.original-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.status-cell { min-width: 0; display: flex; flex-direction: column; gap: 3px; }
.status { display: flex; align-items: center; gap: 4px; font-size: 11px; font-weight: 650; }
.status.error, .summary-error { color: #d03050; }
.status.warning { color: #b88230; }
.status.success { color: #529b2e; }
.status-cell small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #909399; font-size: 10px; }
.operation-bar { padding: 12px 14px; border-top: 1px solid #dcdfe6; background: #fff; display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.operation-summary { color: #606266; font-size: 12px; }
.operation-summary strong { color: #303133; font-size: 16px; }
.operation-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 7px; }
@media (max-width: 1120px) {
  .rename-header, .rename-row { grid-template-columns: 80px 54px minmax(90px, .7fr) minmax(150px, 1.1fr) 100px; }
  .example-grid { grid-template-columns: 1fr; }
}
</style>
