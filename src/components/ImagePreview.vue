<template>
  <div class="image-preview">
    <div v-if="!imageStore.selectedImage" class="empty-state">
      <el-empty description="请选择图片查看详情" />
    </div>

    <div v-else class="preview-content">
      <!-- 图片预览区 -->
      <div class="preview-area" @wheel.prevent="handleWheel">
        <div
          class="preview-wrapper"
          :style="{
            transform: `scale(${scale}) translate(${translateX}px, ${translateY}px)`,
            cursor: isDragging ? 'grabbing' : 'grab'
          }"
          @mousedown="handleMouseDown"
          @mousemove="handleMouseMove"
          @mouseup="handleMouseUp"
          @mouseleave="handleMouseUp"
        >
          <img :src="getImageUrl(imageStore.selectedImage.filePath)" alt="预览图片" />
        </div>
      </div>

      <!-- 缩放控制 -->
      <div class="zoom-controls">
        <el-button-group>
          <el-button size="small" @click="setScale(1)">100%</el-button>
          <el-button size="small" @click="setScale(2)">200%</el-button>
          <el-button size="small" @click="setScale(4)">400%</el-button>
          <el-button size="small" @click="resetView">
            <el-icon><Refresh /></el-icon>
          </el-button>
        </el-button-group>
        <div class="scale-display">{{ Math.round(scale * 100) }}%</div>
      </div>

      <!-- 图片信息 -->
      <div class="info-panel">
        <el-descriptions :column="1" border size="small">
          <el-descriptions-item label="文件名">
            {{ getFileName(imageStore.selectedImage.relativePath) }}
          </el-descriptions-item>
          <el-descriptions-item label="路径">
            {{ imageStore.selectedImage.relativePath }}
          </el-descriptions-item>
          <el-descriptions-item label="尺寸">
            {{ imageStore.selectedImage.width }} × {{ imageStore.selectedImage.height }}
          </el-descriptions-item>
          <el-descriptions-item label="格式">
            {{ imageStore.selectedImage.format.toUpperCase() }}
          </el-descriptions-item>
          <el-descriptions-item label="文件大小">
            {{ formatFileSize(imageStore.selectedImage.fileSize) }}
          </el-descriptions-item>
          <el-descriptions-item label="宽高比">
            {{ imageStore.selectedImage.aspectRatio.toFixed(3) }}
          </el-descriptions-item>
          <el-descriptions-item label="BLAKE3 Hash" v-if="imageStore.selectedImage.blake3Hash">
            <el-text class="hash-text" size="small">
              {{ imageStore.selectedImage.blake3Hash }}
            </el-text>
          </el-descriptions-item>
        </el-descriptions>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useImageStore } from '@/stores/imageStore'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Refresh } from '@element-plus/icons-vue'

const imageStore = useImageStore()

// 缩放和拖拽状态
const scale = ref(1)
const translateX = ref(0)
const translateY = ref(0)
const isDragging = ref(false)
const dragStartX = ref(0)
const dragStartY = ref(0)

const getImageUrl = (filePath: string): string => {
  return convertFileSrc(filePath)
}

const getFileName = (relativePath: string): string => {
  const parts = relativePath.split(/[/\\]/)
  return parts[parts.length - 1]
}

const formatFileSize = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

// 滚轮缩放
const handleWheel = (event: WheelEvent) => {
  const delta = event.deltaY > 0 ? -0.1 : 0.1
  scale.value = Math.max(0.1, Math.min(10, scale.value + delta))
}

// 设置缩放比例
const setScale = (newScale: number) => {
  scale.value = newScale
}

// 重置视图
const resetView = () => {
  scale.value = 1
  translateX.value = 0
  translateY.value = 0
}

// 拖拽
const handleMouseDown = (event: MouseEvent) => {
  isDragging.value = true
  dragStartX.value = event.clientX - translateX.value
  dragStartY.value = event.clientY - translateY.value
}

const handleMouseMove = (event: MouseEvent) => {
  if (!isDragging.value) return
  translateX.value = event.clientX - dragStartX.value
  translateY.value = event.clientY - dragStartY.value
}

const handleMouseUp = () => {
  isDragging.value = false
}
</script>

<style scoped>
.image-preview {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 16px;
}

.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
}

.preview-area {
  flex: 1;
  background-color: #f5f7fa;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-wrapper {
  transition: transform 0.1s ease-out;
  user-select: none;
}

.preview-wrapper img {
  max-width: 100%;
  max-height: 100%;
  display: block;
}

.zoom-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.scale-display {
  font-size: 14px;
  font-weight: 600;
  color: #606266;
}

.info-panel {
  max-height: 300px;
  overflow-y: auto;
}

.hash-text {
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 11px;
  word-break: break-all;
}
</style>
