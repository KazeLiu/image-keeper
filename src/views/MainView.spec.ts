import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import MainView from './MainView.vue'
import { openImageMetricsWindow } from '@/features/imageMetrics/window'

vi.mock('@/features/imageMetrics/window', () => ({
  openImageMetricsWindow: vi.fn(async () => undefined)
}))

describe('MainView image metrics entry', () => {
  beforeEach(() => vi.clearAllMocks())

  it('opens the independent window from a compact card below the two main cards', async () => {
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
    const compactEntry = wrapper.get('.task-cards + [data-test="open-image-metrics"]')
    expect(compactEntry.classes()).toContain('compact-task-card')

    await compactEntry.trigger('click')

    expect(openImageMetricsWindow).toHaveBeenCalledTimes(1)
  })
})
