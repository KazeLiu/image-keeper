<template>
  <div class="finder-view">
    <header class="finder-header">
      <div class="title-block">
        <h1>找差分图</h1>
        <p>{{ currentStep === 'setup' ? '选择参考图片和搜索文件夹' : '勾选需要的文件，直接编辑或批量生成新名称' }}</p>
      </div>

      <nav class="workflow-steps" aria-label="处理步骤">
        <div
          class="workflow-step"
          :class="{ active: currentStep === 'setup', completed: currentStep === 'results' }"
          data-test="step-setup"
          :aria-current="currentStep === 'setup' ? 'step' : undefined"
        >
          <span class="step-number">1</span>
          <span><strong>选择范围</strong><small>图片与文件夹</small></span>
        </div>
        <span class="step-line" aria-hidden="true" />
        <div
          class="workflow-step"
          :class="{ active: currentStep === 'results' }"
          data-test="step-results"
          :aria-current="currentStep === 'results' ? 'step' : undefined"
        >
          <span class="step-number">2</span>
          <span><strong>整理文件</strong><small>选择与重命名</small></span>
        </div>
      </nav>
    </header>

    <main class="finder-main">
      <section v-if="currentStep === 'setup'" class="setup-stage" aria-label="第一步：选择查找范围">
        <div class="setup-grid">
          <ReferenceImageStrip />
          <SearchSetupPanel @search-complete="showResults" />
        </div>
      </section>

      <section v-else class="results-stage" aria-label="第二步：选择并整理文件">
        <div class="results-heading">
          <div>
            <h2>查找结果</h2>
            <p>共找到 {{ store.matches.length }} 个相关文件</p>
          </div>
          <el-button data-test="edit-search" :icon="ArrowLeft" @click="currentStep = 'setup'">
            重新选择
          </el-button>
        </div>
        <BatchRenameGrid />
      </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { ArrowLeft } from '@element-plus/icons-vue'
import ReferenceImageStrip from '@/components/difference-finder/ReferenceImageStrip.vue'
import SearchSetupPanel from '@/components/difference-finder/SearchSetupPanel.vue'
import BatchRenameGrid from '@/components/difference-finder/BatchRenameGrid.vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

const store = useDifferenceFinderStore()
const currentStep = ref<'setup' | 'results'>('setup')

function showResults() {
  store.activeReferenceId = null
  currentStep.value = 'results'
}
</script>

<style scoped>
.finder-view {
  height: 100vh;
  box-sizing: border-box;
  padding: 20px 24px;
  background: #f4f6f8;
  color: #303133;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.finder-header {
  max-width: 1440px;
  width: 100%;
  box-sizing: border-box;
  margin: 0 auto 20px;
  padding: 4px 4px 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
}

.finder-header h1 {
  margin: 0;
  font-size: 24px;
  line-height: 1.3;
}

.finder-header p {
  margin: 4px 0 0;
  color: #606266;
  font-size: 14px;
}

.workflow-steps {
  display: flex;
  align-items: center;
  min-width: 430px;
}

.workflow-step {
  min-width: 160px;
  display: flex;
  align-items: center;
  gap: 10px;
  color: #909399;
}

.step-number {
  width: 34px;
  height: 34px;
  border: 1px solid #c0c4cc;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: #fff;
  font-size: 14px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.workflow-step > span:last-child { display: flex; flex-direction: column; gap: 1px; }
.workflow-step strong { color: #606266; font-size: 13px; font-weight: 650; }
.workflow-step small { font-size: 11px; }
.workflow-step.active .step-number,
.workflow-step.completed .step-number { border-color: #409eff; background: #409eff; color: #fff; }
.workflow-step.active strong { color: #303133; }
.step-line { width: 72px; height: 1px; margin: 0 14px; background: #dcdfe6; }

.finder-main {
  max-width: 1440px;
  width: 100%;
  min-height: 0;
  margin: 0 auto;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.setup-stage,
.results-stage { min-height: 0; flex: 1; }

.setup-stage { display: flex; align-items: flex-start; }

.setup-grid {
  width: 100%;
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(380px, .9fr);
  gap: 16px;
}

.results-stage {
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.results-heading {
  margin-bottom: 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.results-heading h2 { margin: 0; font-size: 18px; }
.results-heading p { margin: 3px 0 0; color: #606266; font-size: 12px; }

@media (max-width: 900px) {
  .finder-header { align-items: flex-start; flex-direction: column; }
  .workflow-steps { min-width: 0; width: 100%; }
  .workflow-step { min-width: 0; flex: 1; }
  .step-line { width: 40px; }
  .setup-grid { grid-template-columns: 1fr; }
}

@media (max-width: 1120px) {
  .finder-view { padding: 16px; }
}
</style>
