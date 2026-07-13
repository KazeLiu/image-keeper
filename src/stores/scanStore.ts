import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Scan, ScanStatus, ScanProgressEvent } from '@/types'

/**
 * 扫描任务 Store
 */
export const useScanStore = defineStore('scan', () => {
  // 状态
  const currentScan = ref<Scan | null>(null)
  const scanHistory = ref<Scan[]>([])
  const progress = ref<ScanProgressEvent | null>(null)
  const isScanning = ref(false)

  // 监听扫描进度事件
  listen('scan_progress', (event) => {
    progress.value = event.payload as ScanProgressEvent
  })

  // Getters
  const scanProgress = computed(() => {
    if (!progress.value || progress.value.totalFiles === 0) {
      return 0
    }
    return (progress.value.scannedFiles / progress.value.totalFiles) * 100
  })

  const estimatedTimeRemainingText = computed(() => {
    if (!progress.value?.estimatedTimeRemaining) {
      return '计算中...'
    }
    const seconds = progress.value.estimatedTimeRemaining
    if (seconds < 60) {
      return `约 ${seconds} 秒`
    } else if (seconds < 3600) {
      return `约 ${Math.floor(seconds / 60)} 分钟`
    } else {
      return `约 ${Math.floor(seconds / 3600)} 小时`
    }
  })

  // Actions
  async function startScan(rootPath: string) {
    isScanning.value = true
    progress.value = null
    try {
      const scan = await invoke<Scan>('start_scan', { rootPath })
      currentScan.value = scan
      isScanning.value = false
    } catch (error) {
      console.error('启动扫描失败:', error)
      isScanning.value = false
      throw error
    }
  }

  async function pauseScan() {
    if (!currentScan.value?.id) return
    try {
      await invoke('pause_scan', { scanId: currentScan.value.id })
      if (currentScan.value) {
        currentScan.value.status = 'paused'
      }
    } catch (error) {
      console.error('暂停扫描失败:', error)
      throw error
    }
  }

  async function resumeScan() {
    if (!currentScan.value?.id) return
    try {
      await invoke('resume_scan', { scanId: currentScan.value.id })
      if (currentScan.value) {
        currentScan.value.status = 'running'
      }
    } catch (error) {
      console.error('恢复扫描失败:', error)
      throw error
    }
  }

  async function cancelScan() {
    if (!currentScan.value?.id) return
    try {
      await invoke('cancel_scan', { scanId: currentScan.value.id })
      isScanning.value = false
      currentScan.value = null
      progress.value = null
    } catch (error) {
      console.error('取消扫描失败:', error)
      throw error
    }
  }

  function updateProgress(event: ScanProgressEvent) {
    progress.value = event
  }

  function completeScan() {
    isScanning.value = false
    if (currentScan.value) {
      scanHistory.value.unshift(currentScan.value)
    }
  }

  return {
    currentScan,
    scanHistory,
    progress,
    isScanning,
    scanProgress,
    estimatedTimeRemainingText,
    startScan,
    pauseScan,
    resumeScan,
    cancelScan,
    updateProgress,
    completeScan
  }
})
