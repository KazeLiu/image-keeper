<template>
  <div class="comparison-groups">
    <el-card v-if="store.stats" shadow="never" class="groups-card">
      <template #header>
        <div class="card-header">
          <div>
            <div class="header-title">相似图片分组</div>
          </div>
          <el-button size="small" @click="store.refreshAnalysisData" :icon="Refresh">刷新</el-button>
        </div>
      </template>

      <div class="group-controls">
        <div class="control-block">
          <div class="control-heading">
            <span>分组宽松度</span>
            <span class="distance-value">{{ store.groupingDistance }}</span>
          </div>
          <el-slider
            class="grouping-slider"
            :model-value="store.groupingDistance"
            :min="0"
            :max="24"
            :step="1"
            :marks="groupingMarks"
            :disabled="store.isRefreshingGroups"
            @input="handleGroupingInput"
          />
          <div class="control-help">
            严格：内容差别更大；宽松：内容差别更大。
          </div>
          <div v-if="store.isRefreshingGroups" class="refreshing-hint">正在按新的宽松度整理分组...</div>
        </div>

        <div class="edit-toolbar">
          <el-switch v-model="store.groupEditMode" active-text="分组编辑" inactive-text="查看模式" />
          <div v-if="store.groupEditMode" class="merge-actions">
            <span class="selection-count">已选 {{ store.selectedGroupIds.length }} 组</span>
            <el-button
              size="small"
              type="primary"
              :disabled="store.selectedGroupIds.length < 2"
              @click="handleMergeClick"
            >
              合并选中分组
            </el-button>
          </div>
        </div>
      </div>

      <el-empty v-if="store.groups.length === 0" description="暂无分组结果" />

      <el-table
        v-else
        :data="store.groups"
        row-key="group_index"
        height="100%"
        class="group-table"
        :row-class-name="getRowClassName"
        @row-click="handleRowClick"
      >
        <el-table-column v-if="store.groupEditMode" label="" width="48">
          <template #default="{ row }">
            <el-checkbox
              v-model="store.selectedGroupIds"
              :value="row.group_index"
              @click.stop
            >
              <span class="sr-only">选择分组</span>
            </el-checkbox>
          </template>
        </el-table-column>

        <el-table-column prop="group_index" label="组序号" width="90" align="center"/>
        <el-table-column label="预览" width="84" align="center">
          <template #default="{ row }">
            <el-tooltip placement="right-start" :show-after="200" popper-class="group-preview-tooltip">
              <template #content>
                <div class="group-preview-tooltip-content">
                  <img
                    class="group-preview-large"
                    :src="getRepresentativeImageUrl(row)"
                    :alt="getRepresentativeDisplayName(row)"
                  />
                </div>
              </template>
              <img
                class="group-preview-thumb"
                :src="getRepresentativeImageUrl(row)"
                :alt="getRepresentativeDisplayName(row)"
                loading="lazy"
                @click.stop="handleRowClick(row)"
              />
            </el-tooltip>
          </template>
        </el-table-column>
        <el-table-column label="代表文件" >
          <template #default="{ row }">
            <div class="file-cell">
              <el-tooltip placement="right-start">
                <template #content>
                  <div class="group-tooltip-list">
                    <div
                      v-for="name in getGroupTooltipNames(row)"
                      :key="name"
                      class="group-tooltip-item"
                    >
                      {{ name }}
                    </div>
                    <div v-if="row.members.length > 6" class="group-tooltip-more">
                      另外还有 {{ row.members.length - 6 }} 张
                    </div>
                  </div>
                </template>
                <span class="file-name">{{ getRepresentativeDisplayName(row) }}</span>
              </el-tooltip>
              <el-tag v-if="row.manual_merged" size="small" type="primary" effect="plain">
                手动合并
              </el-tag>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="图片数量" width="90" align="center">
          <template #default="{ row }">{{ row.member_count }}</template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-dialog v-model="mergeConfirmVisible" title="确认合并分组" width="420px">
      <div class="merge-warning">
        第一版合并后暂时不能在这里手动拆开。如果合错，可以调整分组宽松度重新生成自动分组，但会清空手动合并和当前勾选。
      </div>
      <el-checkbox v-model="skipMergeWarning">以后不再提示</el-checkbox>
      <template #footer>
        <el-button @click="mergeConfirmVisible = false">取消</el-button>
        <el-button type="primary" @click="confirmMerge">确认合并</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useComparisonStore } from '@/stores/comparisonStore'
import type { ComparisonGroup, ComparisonGroupMember } from '@/types'

const store = useComparisonStore()

const groupingMarks = {
  0: '严格',
  10: '标准',
  24: '宽松'
}

const mergeConfirmVisible = ref(false)
const skipMergeWarning = ref(localStorage.getItem('imagekeeper.skipMergeWarning') === 'true')

function handleGroupingInput(value: number | number[]) {
  const nextValue = Array.isArray(value) ? value[0] : value
  if (nextValue === store.groupingDistance) return
  store.setGroupingDistance(nextValue)
}

function handleRowClick(group: ComparisonGroup) {
  store.selectGroup(group.group_index)
}

function handleMergeClick() {
  if (skipMergeWarning.value) {
    mergeSelectedGroups()
    return
  }
  mergeConfirmVisible.value = true
}

function confirmMerge() {
  localStorage.setItem('imagekeeper.skipMergeWarning', String(skipMergeWarning.value))
  mergeConfirmVisible.value = false
  mergeSelectedGroups()
}

function mergeSelectedGroups() {
  const mergedGroup = store.mergeSelectedGroups()
  if (mergedGroup) {
    ElMessage.success(`已合并为第 ${mergedGroup.group_index} 组`)
  }
}

function getGroupTooltipNames(group: ComparisonGroup) {
  return getMembersByQuality(group).map((member) => member.relative_path).slice(0, 6)
}

function getRepresentativeDisplayName(group: ComparisonGroup) {
  const representative = getRepresentativeMember(group)
  return representative ? fileNameFromPath(representative.relative_path) : group.representative_file_name
}

function getRepresentativeImageUrl(group: ComparisonGroup) {
  const representative = getRepresentativeMember(group)
  return representative ? convertFileSrc(representative.file_path) : ''
}

function getRepresentativeMember(group: ComparisonGroup): ComparisonGroupMember | undefined {
  return getMembersByQuality(group)[0]
}

function getMembersByQuality(group: ComparisonGroup) {
  return [...group.members].sort((left, right) => {
    const leftPixels = left.width * left.height
    const rightPixels = right.width * right.height
    return (
      rightPixels - leftPixels ||
      right.file_size - left.file_size ||
      left.relative_path.localeCompare(right.relative_path)
    )
  })
}

function fileNameFromPath(path: string) {
  const parts = path.split(/[/\\]/).filter(Boolean)
  return parts[parts.length - 1] || path
}

function getRowClassName({ row }: { row: ComparisonGroup }) {
  const classes: string[] = []
  if (store.selectedGroupIndex === row.group_index) classes.push('selected-group-row')
  if (row.manual_merged) classes.push('manual-merged-row')
  return classes.join(' ')
}
</script>

<style scoped lang="scss">
.comparison-groups {
  width: 100%;
  height: 100%;
}

.groups-card {
  height: 100%;
  border-radius: 8px;
  display: flex;
  flex-direction: column;

  :deep(.el-card__body) {
    flex: 1;
    min-height: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
}

.card-header {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
}

.header-title {
  font-size: 16px;
  font-weight: 600;
  color: #303133;
}

.header-subtitle {
  margin-top: 4px;
  font-size: 12px;
  color: #909399;
}

.group-controls {
  padding: 8px;
  border-bottom: 1px solid #ebeef5;
  background: #ffffff;
}

.control-block {
  padding: 8px;
  border-radius: 8px;
  background: #f8fafc;
  border: 1px solid #edf1f7;
}

.grouping-slider {
  padding: 0 14px;

  :deep(.el-slider__marks-text) {
    white-space: nowrap;
  }
}

.control-heading,
.edit-toolbar,
.merge-actions {
  display: flex;
  align-items: center;
}

.control-heading {
  justify-content: space-between;
  color: #303133;
  font-weight: 600;
  font-size: 13px;
}

.distance-value {
  color: #409eff;
  font-variant-numeric: tabular-nums;
}

.control-help,
.refreshing-hint,
.selection-count {
  color: #909399;
  font-size: 12px;
}

.control-help {
  margin-top: 20px;
}

.refreshing-hint {
  margin-top: 4px;
  color: #409eff;
}

.edit-toolbar {
  justify-content: space-between;
  gap: 12px;
  margin-top: 12px;
}

.merge-actions {
  gap: 8px;
}

.group-table {
  flex: 1;
  min-height: 0;
  cursor: pointer;

  :deep(.el-checkbox__label) {
    width: 0;
    padding-left: 0;
    overflow: hidden;
  }
}

.file-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-preview-thumb {
  width: 44px;
  height: 44px;
  object-fit: contain;
  border-radius: 6px;
  border: 1px solid #ebeef5;
  background: #f5f7fa;
  cursor: pointer;
  vertical-align: middle;
}

:global(.group-preview-tooltip) {
  padding: 8px;
}

.group-preview-tooltip-content {
  max-width: 520px;
  max-height: 420px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.group-preview-large {
  max-width: 520px;
  max-height: 400px;
  object-fit: contain;
  border-radius: 6px;
  display: block;
}

.merge-warning {
  margin-bottom: 12px;
  color: #606266;
  line-height: 1.6;
}

.group-tooltip-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-width: 420px;
}

.group-tooltip-item {
  white-space: nowrap;
}

.group-tooltip-more {
  color: #c0c4cc;
  white-space: nowrap;
}

:deep(.selected-group-row) {
  background: #ecf5ff;

  td {
    background: #ecf5ff !important;
  }
}

:deep(.manual-merged-row td:first-child) {
  border-left: 3px solid #409eff;
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
</style>
