<template>
  <div class="settings-view">
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <span>设置</span>
          <el-button type="primary" @click="handleSave">保存设置</el-button>
        </div>
      </template>

      <el-form :model="settingsForm" label-width="180px" label-position="left">
        <el-form-item label="图片相似度阈值">
          <el-slider
            v-model="settingsForm.ssimThreshold"
            :min="0.9"
            :max="1.0"
            :step="0.001"
            :format-tooltip="formatSsimTooltip"
          />
          <span class="threshold-value">{{ formatSsimTooltip(settingsForm.ssimThreshold) }}</span>
        </el-form-item>

        <el-form-item label="重复文件保留策略">
          <el-select v-model="settingsForm.duplicateKeepStrategy" style="width: 100%">
            <el-option label="保留路径较短的文件" value="shortest_path" />
            <el-option label="保留创建时间较早的文件" value="earliest_time" />
            <el-option label="保留指定目录的文件" value="preferred_dir" />
          </el-select>
        </el-form-item>

        <el-form-item
          v-if="settingsForm.duplicateKeepStrategy === 'preferred_dir'"
          label="优先保留目录"
        >
          <el-input v-model="settingsForm.preferredDirectory" placeholder="请输入目录路径" />
        </el-form-item>

        <el-form-item label="自动回收完全重复文件">
          <el-switch v-model="settingsForm.autoRecycleDuplicates" />
        </el-form-item>

        <el-form-item label="自动回收压缩版本图片">
          <el-switch v-model="settingsForm.autoRecycleCompressed" />
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
import { useSettingsStore } from '@/stores/settingsStore'
import { ElMessage } from 'element-plus'

const router = useRouter()
const settingsStore = useSettingsStore()

const settingsForm = ref({
  ssimThreshold: 0.995,
  duplicateKeepStrategy: 'shortest_path' as 'shortest_path' | 'earliest_time' | 'preferred_dir',
  preferredDirectory: '',
  autoRecycleDuplicates: true,
  autoRecycleCompressed: true
})

onMounted(async () => {
  await settingsStore.loadSettings()
  settingsForm.value = { ...settingsStore.settings }
})

const formatSsimTooltip = (value: number) => {
  return `${(value * 100).toFixed(1)}%`
}

const handleSave = async () => {
  try {
    settingsStore.settings = { ...settingsForm.value }
    await settingsStore.saveSettings()
    ElMessage.success('设置已保存')
  } catch (error) {
    ElMessage.error('保存设置失败')
  }
}

const handleBack = () => {
  router.push('/')
}
</script>

<style scoped>
.settings-view {
  width: 100%;
  height: 100vh;
  padding: 24px;
  overflow-y: auto;
}

.settings-card {
  max-width: 800px;
  margin: 0 auto;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.threshold-value {
  margin-left: 16px;
  font-weight: bold;
  color: #409eff;
}

.actions {
  max-width: 800px;
  margin: 24px auto 0;
  text-align: right;
}
</style>
