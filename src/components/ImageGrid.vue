<template>
  <div class="image-grid">
    <el-tabs v-model="activeTab" type="border-card">
      <el-tab-pane label="全部图片" name="all">
        <div v-if="imageStore.images.length === 0" class="empty-state">
          <el-empty description="暂无图片数据" />
        </div>
        <div v-else class="grid-container">
          <div
            v-for="image in imageStore.images"
            :key="image.id"
            class="grid-item"
            @click="handleSelectImage(image)"
          >
            <div class="image-wrapper">
              <img :src="getImageUrl(image.filePath)" :alt="image.relativePath" />
            </div>
            <div class="image-info">
              <div class="filename">{{ getFileName(image.relativePath) }}</div>
              <div class="resolution">{{ image.width }} × {{ image.height }}</div>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane name="duplicates">
        <template #label>
          <span>重复文件</span>
          <el-badge :value="imageStore.duplicateCount" class="tab-badge" />
        </template>
        <div v-if="imageStore.duplicates.length === 0" class="empty-state">
          <el-empty description="暂无重复文件" />
        </div>
        <div v-else class="duplicate-list">
          <!-- TODO: 显示重复文件分组 -->
        </div>
      </el-tab-pane>

      <el-tab-pane name="similar">
        <template #label>
          <span>相似图片</span>
          <el-badge :value="imageStore.compressedVersionCount" class="tab-badge" />
        </template>
        <div v-if="imageStore.similarPairs.length === 0" class="empty-state">
          <el-empty description="暂无相似图片" />
        </div>
        <div v-else class="similar-list">
          <!-- TODO: 显示相似图片配对 -->
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useImageStore } from '@/stores/imageStore'
import type { Image } from '@/types'
import { convertFileSrc } from '@tauri-apps/api/core'

const imageStore = useImageStore()
const activeTab = ref('all')

const getImageUrl = (filePath: string): string => {
  // Tauri 转换文件路径为可访问的 URL
  return convertFileSrc(filePath)
}

const getFileName = (relativePath: string): string => {
  const parts = relativePath.split(/[/\\]/)
  return parts[parts.length - 1]
}

const handleSelectImage = (image: Image) => {
  imageStore.selectImage(image)
}
</script>

<style scoped>
.image-grid {
  height: 100%;
  overflow: hidden;
}

.empty-state {
  height: 400px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.grid-container {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 16px;
  padding: 16px;
  max-height: calc(100vh - 250px);
  overflow-y: auto;
}

.grid-item {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
  cursor: pointer;
  transition: all 0.3s;
}

.grid-item:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  transform: translateY(-2px);
}

.image-wrapper {
  width: 100%;
  height: 180px;
  overflow: hidden;
  background-color: #f5f7fa;
  display: flex;
  align-items: center;
  justify-content: center;
}

.image-wrapper img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.image-info {
  padding: 8px;
  background-color: #ffffff;
}

.filename {
  font-size: 12px;
  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.resolution {
  font-size: 11px;
  color: #909399;
  margin-top: 4px;
}

.tab-badge {
  margin-left: 8px;
}

.duplicate-list,
.similar-list {
  padding: 16px;
}
</style>
