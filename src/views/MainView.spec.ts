import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import MainView from './MainView.vue'
import { openImageMetricsWindow } from '@/features/imageMetrics/window'

const webviewWindowMocks = vi.hoisted(() => ({
  getByLabel: vi.fn(async () => null)
}))

vi.mock('@/features/imageMetrics/window', () => ({
  openImageMetricsWindow: vi.fn(async () => undefined)
}))

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: class {
    static getByLabel = webviewWindowMocks.getByLabel
  }
}))

describe('MainView image metrics entry', () => {
  beforeEach(() => vi.clearAllMocks())

  it('keeps both tool entries and opens the image metrics window', async () => {
    const wrapper = mount(MainView, {
      global: {
        plugins: [createPinia(), ElementPlus],
        stubs: {
          ComparisonDirectorySelector: true,
          ComparisonProgress: true,
          ComparisonResults: true,
          ComparisonGroupDetail: true
        }
      }
    })

    expect(wrapper.findAll('.task-cards .task-card')).toHaveLength(2)
    expect(wrapper.text()).toContain('找差分图')
    const compactEntry = wrapper.get('[data-test="open-image-metrics"]')
    expect(compactEntry.classes()).toContain('compact-task-card')

    await compactEntry.trigger('click')

    expect(openImageMetricsWindow).toHaveBeenCalledTimes(1)
  })
})
