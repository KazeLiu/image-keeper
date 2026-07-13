import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Image, Duplicate, SimilarPair } from '@/types'

/**
 * 图片数据 Store
 */
export const useImageStore = defineStore('image', () => {
  // 状态
  const images = ref<Image[]>([])
  const duplicates = ref<Duplicate[]>([])
  const similarPairs = ref<SimilarPair[]>([])
  const selectedImage = ref<Image | null>(null)
  const isLoading = ref(false)

  // Getters
  const totalImages = computed(() => images.value.length)

  const duplicateCount = computed(() => {
    return duplicates.value.filter((d) => d.status === 'pending').length
  })

  const compressedVersionCount = computed(() => {
    return similarPairs.value.filter((p) => p.isCompressedVersion && p.status === 'pending')
      .length
  })

  const totalDeletableCount = computed(() => {
    return duplicateCount.value + compressedVersionCount.value
  })

  // Actions
  async function loadImages(scanId: number) {
    isLoading.value = true
    try {
      // TODO: 从后端加载图片列表
      // const result = await invoke('load_images', { scanId })
      // images.value = result
    } catch (error) {
      console.error('加载图片失败:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  async function loadDuplicates(scanId: number) {
    isLoading.value = true
    try {
      // TODO: 从后端加载重复文件列表
      // const result = await invoke('load_duplicates', { scanId })
      // duplicates.value = result
    } catch (error) {
      console.error('加载重复文件失败:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  async function loadSimilarPairs(scanId: number) {
    isLoading.value = true
    try {
      // TODO: 从后端加载相似图片配对
      // const result = await invoke('load_similar_pairs', { scanId })
      // similarPairs.value = result
    } catch (error) {
      console.error('加载相似图片失败:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  function selectImage(image: Image) {
    selectedImage.value = image
  }

  function clearSelection() {
    selectedImage.value = null
  }

  return {
    images,
    duplicates,
    similarPairs,
    selectedImage,
    isLoading,
    totalImages,
    duplicateCount,
    compressedVersionCount,
    totalDeletableCount,
    loadImages,
    loadDuplicates,
    loadSimilarPairs,
    selectImage,
    clearSelection
  }
})
