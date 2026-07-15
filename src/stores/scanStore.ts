import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { Scan, ScanProgressEvent } from '@/types'

const legacyScanUnavailableMessage = '单目录扫描流程已迁移，请使用“多目录对比”。'

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
  async function startScan(_rootPath: string) {
    progress.value = null
    isScanning.value = false
    throw new Error(legacyScanUnavailableMessage)
  }

  async function pauseScan() {
    if (!currentScan.value?.id) return
    throw new Error(legacyScanUnavailableMessage)
  }

  async function resumeScan() {
    if (!currentScan.value?.id) return
    throw new Error(legacyScanUnavailableMessage)
  }

  async function cancelScan() {
    if (!currentScan.value?.id) return
    isScanning.value = false
    currentScan.value = null
    progress.value = null
    throw new Error(legacyScanUnavailableMessage)
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
