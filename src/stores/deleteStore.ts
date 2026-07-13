import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { RecycleBinEntry } from '@/types'

/**
 * 删除管理 Store
 */
export const useDeleteStore = defineStore('delete', () => {
  // 状态
  const recycleBin = ref<RecycleBinEntry[]>([])
  const selectedForDeletion = ref<Set<number>>(new Set())
  const isDeleting = ref(false)

  // Getters
  const recycleBinCount = computed(() => recycleBin.value.length)

  const selectedCount = computed(() => selectedForDeletion.value.size)

  const exactDuplicateCount = computed(() => {
    return recycleBin.value.filter((entry) => entry.deleteReason === 'exact_duplicate').length
  })

  const compressedCount = computed(() => {
    return recycleBin.value.filter((entry) => entry.deleteReason === 'lower_resolution').length
  })

  // Actions
  async function loadRecycleBin() {
    try {
      // TODO: 从后端加载回收站列表
      // const result = await invoke('load_recycle_bin')
      // recycleBin.value = result
    } catch (error) {
      console.error('加载回收站失败:', error)
      throw error
    }
  }

  async function moveToRecycleBin(imageIds: number[]) {
    isDeleting.value = true
    try {
      // TODO: 调用后端移动到回收站
      // await invoke('move_to_recycle_bin', { imageIds })
      await loadRecycleBin()
    } catch (error) {
      console.error('移动到回收站失败:', error)
      throw error
    } finally {
      isDeleting.value = false
    }
  }

  async function permanentDelete() {
    isDeleting.value = true
    try {
      // TODO: 调用后端永久删除
      // await invoke('permanent_delete', { entryIds: Array.from(selectedForDeletion.value) })
      selectedForDeletion.value.clear()
      await loadRecycleBin()
    } catch (error) {
      console.error('永久删除失败:', error)
      throw error
    } finally {
      isDeleting.value = false
    }
  }

  async function restoreFromRecycleBin(entryIds: number[]) {
    isDeleting.value = true
    try {
      // TODO: 调用后端恢复文件
      // await invoke('restore_from_recycle_bin', { entryIds })
      await loadRecycleBin()
    } catch (error) {
      console.error('恢复文件失败:', error)
      throw error
    } finally {
      isDeleting.value = false
    }
  }

  async function exportDeleteList(outputPath: string) {
    try {
      // TODO: 调用后端导出删除列表
      // await invoke('export_delete_list', { outputPath })
    } catch (error) {
      console.error('导出删除列表失败:', error)
      throw error
    }
  }

  async function exportReport(outputPath: string) {
    try {
      // TODO: 调用后端导出报告
      // await invoke('export_report', { outputPath })
    } catch (error) {
      console.error('导出报告失败:', error)
      throw error
    }
  }

  function toggleSelection(entryId: number) {
    if (selectedForDeletion.value.has(entryId)) {
      selectedForDeletion.value.delete(entryId)
    } else {
      selectedForDeletion.value.add(entryId)
    }
  }

  function selectAll() {
    recycleBin.value.forEach((entry) => {
      selectedForDeletion.value.add(entry.id)
    })
  }

  function clearSelection() {
    selectedForDeletion.value.clear()
  }

  return {
    recycleBin,
    selectedForDeletion,
    isDeleting,
    recycleBinCount,
    selectedCount,
    exactDuplicateCount,
    compressedCount,
    loadRecycleBin,
    moveToRecycleBin,
    permanentDelete,
    restoreFromRecycleBin,
    exportDeleteList,
    exportReport,
    toggleSelection,
    selectAll,
    clearSelection
  }
})
