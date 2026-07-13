<template>
  <div class="scan-progress">
    <div v-if="!scanStore.isScanning" class="start-panel">
      <el-button type="primary" size="large" @click="handleStartScan">
        <el-icon><VideoPlay /></el-icon>
        开始扫描
      </el-button>
      <el-button size="large" @click="handleGoToSettings">
        <el-icon><Setting /></el-icon>
        设置
      </el-button>
      <el-button size="large" @click="handleGoToExport">
        <el-icon><Download /></el-icon>
        导出
      </el-button>
    </div>

    <div v-else class="progress-panel">
      <div class="progress-info">
        <div class="info-row">
          <span class="label">总图片数量:</span>
          <span class="value">{{ scanStore.progress?.totalFiles || 0 }}</span>
        </div>
        <div class="info-row">
          <span class="label">已扫描:</span>
          <span class="value">{{ scanStore.progress?.scannedFiles || 0 }}</span>
        </div>
        <div class="info-row">
          <span class="label">当前文件:</span>
          <span class="value current-file">{{ scanStore.progress?.currentFile || '-' }}</span>
        </div>
        <div class="info-row">
          <span class="label">预计剩余时间:</span>
          <span class="value">{{ scanStore.estimatedTimeRemainingText }}</span>
        </div>
      </div>

      <el-progress
        :percentage="scanStore.scanProgress"
        :stroke-width="20"
        :text-inside="true"
        status="success"
      />

      <div class="progress-actions">
        <el-button
          v-if="scanStore.currentScan?.status === 'running'"
          type="warning"
          @click="handlePauseScan"
        >
          <el-icon><VideoPause /></el-icon>
          暂停
        </el-button>
        <el-button
          v-if="scanStore.currentScan?.status === 'paused'"
          type="primary"
          @click="handleResumeScan"
        >
          <el-icon><VideoPlay /></el-icon>
          继续
        </el-button>
        <el-button type="danger" @click="handleCancelScan">
          <el-icon><Close /></el-icon>
          取消
        </el-button>
      </div>
    </div>

    <div class="stats-panel">
      <el-statistic title="图片总数" :value="imageStore.totalImages" />
      <el-statistic title="重复文件" :value="imageStore.duplicateCount" />
      <el-statistic title="压缩版本" :value="imageStore.compressedVersionCount" />
      <el-statistic
        title="可删除总数"
        :value="imageStore.totalDeletableCount"
        value-style="color: #f56c6c"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { useScanStore } from '@/stores/scanStore'
import { useImageStore } from '@/stores/imageStore'
import { ElMessage, ElMessageBox } from 'element-plus'
import { VideoPlay, VideoPause, Close, Setting, Download } from '@element-plus/icons-vue'

const router = useRouter()
const scanStore = useScanStore()
const imageStore = useImageStore()

const handleStartScan = async () => {
  try {
    // 使用 Tauri 对话框选择目录
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selectedPath = await open({
      directory: true,
      multiple: false,
      title: '选择要扫描的图片目录'
    })

    if (!selectedPath) {
      return // 用户取消选择
    }

    ElMessage.info('开始扫描目录...')
    await scanStore.startScan(selectedPath as string)
    ElMessage.success('扫描完成！')
  } catch (error) {
    console.error('扫描错误:', error)
    ElMessage.error(`启动扫描失败: ${error}`)
  }
}

const handlePauseScan = async () => {
  try {
    await scanStore.pauseScan()
    ElMessage.success('已暂停扫描')
  } catch (error) {
    ElMessage.error('暂停失败')
  }
}

const handleResumeScan = async () => {
  try {
    await scanStore.resumeScan()
    ElMessage.success('已继续扫描')
  } catch (error) {
    ElMessage.error('恢复失败')
  }
}

const handleCancelScan = async () => {
  try {
    await ElMessageBox.confirm('确定要取消扫描吗?', '提示', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning'
    })
    await scanStore.cancelScan()
    ElMessage.success('已取消扫描')
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('取消失败')
    }
  }
}

const handleGoToSettings = () => {
  router.push('/settings')
}

const handleGoToExport = () => {
  router.push('/export')
}
</script>

<style scoped>
.scan-progress {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.start-panel {
  display: flex;
  gap: 12px;
}

.progress-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.progress-info {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
}

.info-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.info-row .label {
  font-weight: 600;
  color: #606266;
}

.info-row .value {
  color: #303133;
}

.info-row .current-file {
  font-size: 12px;
  color: #909399;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progress-actions {
  display: flex;
  gap: 12px;
}

.stats-panel {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  padding: 16px;
  background-color: #f5f7fa;
  border-radius: 4px;
}
</style>
