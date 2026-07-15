<template>
  <div class="finder-view">
    <header class="finder-header">
      <div>
        <p class="eyebrow">IMAGEKEEPER TOOL</p>
        <h1>找差分图</h1>
        <p>添加多张参考图，只扫描目录一次，再集中整理所有命中图片。</p>
      </div>
      <el-tag type="primary" effect="plain">独立小工具</el-tag>
    </header>

    <main class="finder-main">
      <div class="setup-grid">
        <ReferenceImageStrip />
        <SearchSetupPanel />
      </div>
      <div class="workspace-grid">
        <DifferenceResultList />
        <BatchRenameGrid />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import ReferenceImageStrip from '@/components/difference-finder/ReferenceImageStrip.vue'
import SearchSetupPanel from '@/components/difference-finder/SearchSetupPanel.vue'
import DifferenceResultList from '@/components/difference-finder/DifferenceResultList.vue'
import BatchRenameGrid from '@/components/difference-finder/BatchRenameGrid.vue'
</script>

<style scoped>
.finder-view {
  height: 100vh;
  box-sizing: border-box;
  padding: 24px;
  background: #f5f7fa;
  color: #303133;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.finder-header {
  max-width: 1440px;
  width: 100%;
  box-sizing: border-box;
  margin: 0 auto 16px;
  padding: 20px 24px;
  border: 1px solid #dcdfe6;
  border-radius: 10px;
  background: #ffffff;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 24px;
}

.eyebrow {
  margin: 0 0 4px !important;
  color: #409eff !important;
  font-size: 11px !important;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.finder-header h1 {
  margin: 0;
  font-size: 26px;
  line-height: 1.3;
}

.finder-header p {
  margin: 6px 0 0;
  color: #606266;
  font-size: 14px;
}

.finder-main {
  max-width: 1440px;
  width: 100%;
  min-height: 0;
  margin: 0 auto;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.setup-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(340px, .85fr);
  gap: 14px;
}

.workspace-grid {
  min-height: 0;
  flex: 1;
  display: grid;
  grid-template-columns: minmax(340px, .82fr) minmax(560px, 1.38fr);
  gap: 14px;
}

@media (max-width: 1120px) {
  .finder-view { padding: 16px; }
  .finder-header { padding: 16px 18px; }
  .setup-grid { grid-template-columns: 1fr 1fr; }
  .workspace-grid { grid-template-columns: minmax(320px, .75fr) minmax(520px, 1.25fr); }
}
</style>
