<template>
  <div class="directory-selector">
    <el-card shadow="never" class="selector-card">
      <template #header>
        <div class="card-header">
          <div class="title-block">
            <span class="header-title">多目录对比</span>
            <span class="header-hint">可跨目录对比；勾选“内部同时对比”后也可单目录运行</span>
          </div>
          <el-button type="primary" size="small" @click="startComparison" :disabled="!canStart">
            开始对比
          </el-button>
        </div>
      </template>

      <div class="toolbar">
        <el-button @click="addDirectory" type="success" plain class="toolbar-btn">
          <el-icon><Plus /></el-icon>
          添加目录
        </el-button>
        <el-button @click="store.clearSelection" plain class="toolbar-btn" :disabled="!hasDirectories">
          清空
        </el-button>
      </div>

      <el-table
        :data="store.directorySelection.directories"
        border
        size="small"
        class="directory-table"
        empty-text="还没有添加目录"
      >
        <el-table-column prop="name" label="文件夹名称" width="92">
          <template #default="{ row, $index }">
            <div class="name-cell">
              <el-tag class="alias-tag" :type="$index === 0 ? 'primary' : 'info'">
                {{ row.alias }}
              </el-tag>
              <el-tooltip :content="row.name" placement="right-start" :show-after="300">
                <span class="folder-name">{{ row.name }}</span>
              </el-tooltip>
            </div>
          </template>
        </el-table-column>

        <el-table-column prop="path" label="路径" min-width="100">
          <template #default="{ row }">
            <el-tooltip :content="row.path" placement="right-start" :show-after="300">
              <button class="path-button" type="button" @click="openDirectory(row.path)">
                {{ formatMiddlePath(row.path) }}
              </button>
            </el-tooltip>
          </template>
        </el-table-column>

        <el-table-column label="内部同时对比" width="78" align="center">
          <template #default="{ row }">
            <el-checkbox v-model="row.compareWithin" aria-label="内部同时对比" />
          </template>
        </el-table-column>

        <el-table-column label="删除当前文件夹" width="58" align="center">
          <template #default="{ $index }">
            <el-button
              @click="store.removeDirectory($index)"
              type="danger"
              :icon="Delete"
              circle
              aria-label="删除当前文件夹"
            />
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Plus, Delete } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useComparisonStore } from '@/stores/comparisonStore'

const store = useComparisonStore()

const hasDirectories = computed(() => store.directorySelection.directories.length > 0)

const canStart = computed(() => {
  return store.canStartComparison && !store.isRunning
})

async function addDirectory() {
  const result = await open({
    directory: true,
    multiple: true,
    title: '选择要参与对比的目录'
  })

  if (result) {
    const paths = Array.isArray(result) ? result : [result]
    const { addedCount, skippedCount } = store.addDirectories(paths as string[])

    if (addedCount > 0 && skippedCount > 0) {
      ElMessage.success(`已添加 ${addedCount} 个目录，跳过 ${skippedCount} 个重复目录`)
    } else if (addedCount > 0) {
      ElMessage.success(`已添加 ${addedCount} 个目录`)
    } else if (skippedCount > 0) {
      ElMessage.warning('选择的目录都已添加')
    }
  }
}

async function openDirectory(path: string) {
  try {
    await invoke('open_folder', { path })
  } catch (error: any) {
    ElMessage.error(error || '打开目录失败')
  }
}

async function startComparison() {
  try {
    await store.startComparison()
    ElMessage.success('对比任务已启动')
  } catch (error: any) {
    ElMessage.error(error.message || '启动对比失败')
  }
}

function formatMiddlePath(path: string): string {
  const maxLength = 28
  if (path.length <= maxLength) return path

  const headLength = 11
  const tailLength = 14
  return `${path.slice(0, headLength)}…${path.slice(-tailLength)}`
}
</script>

<style scoped lang="scss">
.directory-selector {
  width: 100%;
}

.selector-card {
  border-radius: 8px;

  :deep(.el-card__body) {
    padding: 14px;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;

    .title-block {
      min-width: 0;
      display: flex;
      flex-direction: column;
      gap: 4px;

      .header-title {
        font-size: 16px;
        font-weight: 600;
        color: #303133;
      }

      .header-hint {
        font-size: 12px;
        line-height: 1.4;
        color: #909399;
      }
    }
  }
}

.toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;

  .toolbar-btn {
    flex: 1;
  }
}

.directory-table {
  width: 100%;

  :deep(.el-table__cell) {
    padding: 8px 0;
  }

  :deep(.cell) {
    padding: 0 8px;
    line-height: 1.3;
  }
}

.name-cell {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;

  .alias-tag {
    flex: 0 0 auto;
    min-width: 28px;
    justify-content: center;
    font-weight: 600;
  }

  .folder-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
}

.path-button {
  width: 100%;
  min-height: 28px;
  padding: 0;
  border: 0;
  background: transparent;
  color: #409eff;
  font: inherit;
  line-height: 1.4;
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;

  &:hover {
    color: #337ecc;
    text-decoration: underline;
  }

  &:focus-visible {
    outline: 2px solid #409eff;
    outline-offset: 2px;
    border-radius: 3px;
  }
}
</style>
