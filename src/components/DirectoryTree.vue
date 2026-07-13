<template>
  <div class="directory-tree">
    <div class="tree-header">
      <h3>目录</h3>
      <el-button type="primary" size="small" @click="handleSelectDirectory">
        <el-icon><FolderOpened /></el-icon>
        选择目录
      </el-button>
    </div>

    <div v-if="!rootPath" class="empty-state">
      <el-empty description="请选择要扫描的目录" />
    </div>

    <div v-else class="tree-content">
      <el-tree
        :data="treeData"
        :props="defaultProps"
        default-expand-all
        highlight-current
        @node-click="handleNodeClick"
      >
        <template #default="{ node, data }">
          <span class="custom-tree-node">
            <el-icon><Folder /></el-icon>
            <span>{{ node.label }}</span>
            <span class="file-count">({{ data.fileCount }})</span>
          </span>
        </template>
      </el-tree>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Folder, FolderOpened } from '@element-plus/icons-vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

interface TreeNode {
  label: string
  path: string
  fileCount: number
  children?: TreeNode[]
}

const rootPath = ref('')
const treeData = ref<TreeNode[]>([])

const defaultProps = {
  children: 'children',
  label: 'label'
}

const handleSelectDirectory = async () => {
  try {
    // 调用 Tauri 文件选择对话框
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择要扫描的图片目录'
    })

    if (selected) {
      rootPath.value = selected as string
      ElMessage.success(`已选择目录: ${selected}`)
      // 加载目录树结构
      await loadDirectoryTree(selected as string)
    }
  } catch (error) {
    console.error('选择目录失败:', error)
    ElMessage.error('选择目录失败')
  }
}

const loadDirectoryTree = async (path: string) => {
  try {
    // 从后端加载目录树
    const tree = await invoke<TreeNode[]>('load_directory_tree', { path })
    treeData.value = tree
    ElMessage.success('目录树加载成功')
  } catch (error) {
    console.error('加载目录树失败:', error)
    ElMessage.error('加载目录树失败')
    throw error
  }
}

const handleNodeClick = (data: TreeNode) => {
  console.log('点击目录:', data.path)
  // TODO: 过滤显示该目录下的图片
}
</script>

<style scoped>
.directory-tree {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 16px;
}

.tree-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.tree-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.tree-content {
  flex: 1;
  overflow-y: auto;
}

.custom-tree-node {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
}

.file-count {
  color: #909399;
  font-size: 12px;
}
</style>
