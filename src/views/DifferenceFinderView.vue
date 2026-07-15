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

    <main class="finder-empty">
      <el-empty description="正在准备找差分图工作台" />
    </main>
  </div>
</template>

<style scoped>
.finder-view {
  min-height: 100vh;
  box-sizing: border-box;
  padding: 24px;
  background: #f5f7fa;
  color: #303133;
}

.finder-header {
  max-width: 1440px;
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

.finder-empty {
  max-width: 1440px;
  min-height: 420px;
  margin: 0 auto;
  border: 1px solid #dcdfe6;
  border-radius: 10px;
  background: #ffffff;
  display: grid;
  place-items: center;
}
</style>
