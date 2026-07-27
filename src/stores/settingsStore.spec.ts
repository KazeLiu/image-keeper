import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useSettingsStore } from './settingsStore'

const invoke = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const backendSettings = {
  default_compressed_ssim_threshold: 0.9825,
  default_variant_review_lower_bound: 0.75,
  default_phash_max_distance: 10,
  default_aspect_ratio_tolerance: 0.005,
  auto_preselect_exact_duplicates: false,
  max_candidate_per_image: 50
}

describe('settings store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invoke.mockReset()
  })

  it('loads the saved standard ssim threshold from the backend', async () => {
    invoke.mockResolvedValueOnce(backendSettings)
    const store = useSettingsStore()

    await store.loadSettings()

    expect(invoke).toHaveBeenCalledWith('load_settings')
    expect(store.settings.ssimThreshold).toBe(0.9825)
  })

  it('saves the edited threshold without overwriting other backend settings', async () => {
    invoke.mockResolvedValueOnce(backendSettings)
    invoke.mockResolvedValueOnce(undefined)
    const store = useSettingsStore()
    await store.loadSettings()
    store.settings.ssimThreshold = 0.9975

    await store.saveSettings()

    expect(invoke).toHaveBeenLastCalledWith('save_settings', {
      settings: {
        ...backendSettings,
        default_compressed_ssim_threshold: 0.9975
      }
    })
  })
})
