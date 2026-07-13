<template>
  <div class="main-view">
    <el-container>
      <!-- 左侧目录树 -->
      <el-aside width="250px" class="aside-panel">
        <DirectoryTree />
      </el-aside>

      <!-- 中间扫描结果 -->
      <el-container>
        <el-header height="auto" class="header-panel">
          <ScanProgress />
        </el-header>
        <el-main class="main-panel">
          <ImageGrid />
        </el-main>
      </el-container>

      <!-- 右侧图片预览 -->
      <el-aside width="400px" class="preview-panel">
        <ImagePreview />
      </el-aside>
    </el-container>
  </div>
</template>

<script setup lang="ts">
import DirectoryTree from '@/components/DirectoryTree.vue'
import ScanProgress from '@/components/ScanProgress.vue'
import ImageGrid from '@/components/ImageGrid.vue'
import ImagePreview from '@/components/ImagePreview.vue'
</script>

<style scoped>
.main-view {
  width: 100%;
  height: 100vh;
}

.el-container {
  height: 100%;
}

.aside-panel {
  border-right: 1px solid #dcdfe6;
  background-color: #f5f7fa;
}

.header-panel {
  border-bottom: 1px solid #dcdfe6;
  padding: 16px;
}

.main-panel {
  background-color: #ffffff;
}

.preview-panel {
  border-left: 1px solid #dcdfe6;
  background-color: #f5f7fa;
}
</style>
