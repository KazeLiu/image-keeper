<template>
  <section class="panel setup-panel" aria-labelledby="folder-title">
    <div class="panel-heading">
      <div>
        <span class="step">02</span>
        <h2 id="folder-title">搜索目录</h2>
        <p>目录只扫描一次，多张参考图复用同一批特征。</p>
      </div>
      <el-button plain :icon="FolderAdd" @click="chooseFolders">添加目录</el-button>
    </div>

    <div v-if="store.targetRoots.length" class="folder-list">
      <div v-for="(path, index) in store.targetRoots" :key="path" class="folder-row">
        <el-icon><Folder /></el-icon>
        <span :title="path">{{ path }}</span>
        <el-button :icon="Close" text circle aria-label="移除目录" @click="store.targetRoots.splice(index, 1)" />
      </div>
    </div>
    <div v-else class="folder-empty">尚未添加搜索目录</div>

    <div class="setup-actions">
      <el-switch v-model="store.recursive" active-text="递归子目录" />
      <div class="action-buttons">
        <el-button v-if="store.isRunning" type="danger" plain @click="store.cancel">取消</el-button>
        <el-button v-else type="primary" :disabled="!store.canSearch" :icon="Search" @click="runSearch">
          开始查找
        </el-button>
      </div>
    </div>

    <div v-if="store.isRunning || store.progress" class="progress-block" aria-live="polite">
      <div class="progress-copy">
        <span>{{ phaseLabel }}</span>
        <span>{{ store.progress?.processed || 0 }} / {{ store.progress?.total || 0 }}</span>
      </div>
      <el-progress :percentage="percentage" :show-text="false" />
      <p v-if="store.progress?.currentFile" :title="store.progress.currentFile">
        {{ store.progress.currentFile }}
      </p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage } from 'element-plus'
import { Close, Folder, FolderAdd, Search } from '@element-plus/icons-vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

const store = useDifferenceFinderStore()
const phaseLabels = {
  scanning: '发现图片文件', extracting: '提取图片特征', matching: '与参考图匹配',
  aggregating: '汇总匹配结果', completed: '查找完成'
}
const phaseLabel = computed(() => store.progress ? phaseLabels[store.progress.phase] : '准备查找')
const percentage = computed(() => {
  const total = store.progress?.total || 0
  return total ? Math.min(100, Math.round((store.progress!.processed / total) * 100)) : 0
})

async function chooseFolders() {
  const result = await open({ directory: true, multiple: true, title: '选择要搜索的目录' })
  if (!result) return
  store.addTargetRoots(Array.isArray(result) ? result : [result])
}

async function runSearch() {
  try {
    await store.search()
    ElMessage.success(`查找完成，共发现 ${store.matches.length} 个相关文件`)
  } catch (error: any) {
    ElMessage.error(error?.message || String(error) || '查找失败')
  }
}
</script>

<style scoped>
.panel { border: 1px solid #dcdfe6; border-radius: 10px; background: #fff; }
.setup-panel { padding: 18px; }
.panel-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
.panel-heading h2 { display: inline; margin: 0 0 0 8px; font-size: 17px; }
.panel-heading p { margin: 5px 0 0 32px; color: #606266; font-size: 12px; }
.step { color: #409eff; font-size: 12px; font-weight: 800; }
.folder-list { max-height: 112px; display: flex; flex-direction: column; gap: 7px; overflow-y: auto; }
.folder-row { min-height: 34px; padding-left: 10px; border: 1px solid #ebeef5; border-radius: 6px; display: flex; align-items: center; gap: 8px; color: #606266; }
.folder-row > span { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.folder-empty { min-height: 72px; border: 1px dashed #dcdfe6; border-radius: 8px; display: grid; place-items: center; color: #909399; font-size: 13px; }
.setup-actions { margin-top: 14px; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.action-buttons { display: flex; gap: 8px; }
.progress-block { margin-top: 12px; padding: 10px 12px; border-radius: 7px; background: #f5f9ff; }
.progress-copy { display: flex; justify-content: space-between; margin-bottom: 7px; color: #606266; font-size: 12px; font-variant-numeric: tabular-nums; }
.progress-block p { margin: 6px 0 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #909399; font-size: 11px; }
</style>
