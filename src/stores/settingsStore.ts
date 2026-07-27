import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Settings } from '@/types'

interface BackendSettings {
  default_compressed_ssim_threshold: number
  default_variant_review_lower_bound: number
  default_phash_max_distance: number
  default_aspect_ratio_tolerance: number
  auto_preselect_exact_duplicates: boolean
  max_candidate_per_image: number
}

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
  let backendSettings: BackendSettings | null = null

  // Getters
  const ssimThresholdValue = computed(() => {
    return settings.value.ssimThreshold.toFixed(4)
  })

  // Actions
  async function loadSettings() {
    isLoading.value = true
    try {
      backendSettings = await invoke<BackendSettings>('load_settings')
      settings.value.ssimThreshold = backendSettings.default_compressed_ssim_threshold
    } catch (error) {
      console.error('加载设置失败:', error)
    } finally {
      isLoading.value = false
    }
  }

  async function saveSettings() {
    isLoading.value = true
    try {
      backendSettings ??= await invoke<BackendSettings>('load_settings')
      backendSettings = {
        ...backendSettings,
        default_compressed_ssim_threshold: settings.value.ssimThreshold
      }
      await invoke('save_settings', { settings: backendSettings })
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
    ssimThresholdValue,
    loadSettings,
    saveSettings,
    updateSetting
  }
})
