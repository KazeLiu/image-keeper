<template>
  <div class="export-view">
    <el-card class="export-card">
      <template #header>
        <div class="card-header">
          <span>导出</span>
        </div>
      </template>

      <el-form label-width="180px" label-position="left">
        <el-form-item label="回收站文件数量">
          <el-tag type="info" size="large">{{ deleteStore.recycleBinCount }} 个文件</el-tag>
        </el-form-item>

        <el-form-item label="完全重复文件">
          <el-tag type="warning" size="large">{{ deleteStore.exactDuplicateCount }} 个</el-tag>
        </el-form-item>

        <el-form-item label="压缩版本图片">
          <el-tag type="warning" size="large">{{ deleteStore.compressedCount }} 个</el-tag>
        </el-form-item>

        <el-divider />

        <el-form-item label="导出删除列表">
          <el-input
            v-model="deleteListPath"
            placeholder="delete-list.txt"
            style="margin-bottom: 8px"
          />
          <el-button type="primary" @click="handleExportDeleteList">导出 delete-list.txt</el-button>
        </el-form-item>

        <el-form-item label="导出详细报告">
          <el-input v-model="reportPath" placeholder="report.csv" style="margin-bottom: 8px" />
          <el-button type="primary" @click="handleExportReport">导出 report.csv</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <div class="actions">
      <el-button @click="handleBack">返回</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useDeleteStore } from '@/stores/deleteStore'
import { ElMessage } from 'element-plus'

const router = useRouter()
const deleteStore = useDeleteStore()

const deleteListPath = ref('delete-list.txt')
const reportPath = ref('report.csv')

onMounted(async () => {
  await deleteStore.loadRecycleBin()
})

const handleExportDeleteList = async () => {
  try {
    await deleteStore.exportDeleteList(deleteListPath.value)
    ElMessage.success('删除列表已导出')
  } catch (error) {
    ElMessage.error('导出删除列表失败')
  }
}

const handleExportReport = async () => {
  try {
    await deleteStore.exportReport(reportPath.value)
    ElMessage.success('详细报告已导出')
  } catch (error) {
    ElMessage.error('导出报告失败')
  }
}

const handleBack = () => {
  router.push('/')
}
</script>

<style scoped>
.export-view {
  width: 100%;
  height: 100vh;
  padding: 24px;
  overflow-y: auto;
}

.export-card {
  max-width: 800px;
  margin: 0 auto;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.actions {
  max-width: 800px;
  margin: 24px auto 0;
  text-align: right;
}
</style>
