import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Settings } from '@/types'

/**
 * 用户设置 Store
 */
export const useSettingsStore = defineStore('settings', () => {
  // 状态
  const settings = ref<Settings>({
    ssimThreshold: 0.995,
    duplicateKeepStrategy: 'shortest_path',
    preferredDirectory: '',
    autoRecycleDuplicates: true,
    autoRecycleCompressed: true
  })

  const isLoading = ref(false)

  // Getters
  const ssimThresholdPercent = computed(() => {
    return (settings.value.ssimThreshold * 100).toFixed(1)
  })

  // Actions
  async function loadSettings() {
    isLoading.value = true
    try {
      // TODO: 从后端加载设置
      // const loadedSettings = await invoke('load_settings')
      // settings.value = loadedSettings
    } catch (error) {
      console.error('加载设置失败:', error)
    } finally {
      isLoading.value = false
    }
  }

  async function saveSettings() {
    isLoading.value = true
    try {
      // TODO: 保存设置到后端
      // await invoke('save_settings', { settings: settings.value })
    } catch (error) {
      console.error('保存设置失败:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  function updateSetting<K extends keyof Settings>(key: K, value: Settings[K]) {
    settings.value[key] = value
  }

  return {
    settings,
    isLoading,
    ssimThresholdPercent,
    loadSettings,
    saveSettings,
    updateSetting
  }
})
