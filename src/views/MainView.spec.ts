import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import MainView from './MainView.vue'
import { openImageMetricsWindow } from '@/features/imageMetrics/window'
import { useComparisonStore } from '@/stores/comparisonStore'

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
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
  })

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
    expect(compactEntry.text()).toContain('感知哈希与标准 SSIM')
    expect(compactEntry.text()).not.toContain('当前相似度')

    await compactEntry.trigger('click')

    expect(openImageMetricsWindow).toHaveBeenCalledTimes(1)
  })

  it('slides between the menu page and the history page', async () => {
    const pinia = createPinia()
    const wrapper = mount(MainView, {
      global: {
        plugins: [pinia, ElementPlus],
        stubs: {
          ComparisonDirectorySelector: true,
          ComparisonProgress: true,
          ComparisonResults: true,
          ComparisonGroupDetail: true
        }
      }
    })
    const store = useComparisonStore(pinia)
    vi.spyOn(store, 'refreshHistory').mockResolvedValue(undefined)

    const entryViewport = wrapper.find('.entry-view > .entry-viewport')
    expect(entryViewport.exists()).toBe(true)
    expect(entryViewport.find(':scope > .entry-slider').exists()).toBe(true)

    await wrapper.findAll('.task-card')[1].trigger('click')
    await flushPromises()

    expect(wrapper.get('.entry-slider').classes()).toContain('is-history')
    expect(wrapper.get('[data-test="history-back"]').text()).toContain('返回')

    await wrapper.get('[data-test="history-back"]').trigger('click')
    expect(wrapper.get('.entry-slider').classes()).not.toContain('is-history')
  })

  it('shows category counts and both tool shortcuts in the collapsed sidebar', async () => {
    localStorage.setItem('imagekeeper:workspace-stats-panel-collapsed', 'true')
    const pinia = createPinia()
    const wrapper = mount(MainView, {
      global: {
        plugins: [pinia, ElementPlus],
        stubs: {
          ComparisonDirectorySelector: true,
          ComparisonProgress: true,
          ComparisonResults: true,
          ComparisonGroupDetail: true
        }
      }
    })

    await wrapper.findAll('.task-card')[0].trigger('click')
    useComparisonStore(pinia).$patch({
      stats: {
        run_id: 'run-1',
        baseline_total: 100,
        comparison_total: 132,
        exact_duplicate: 2,
        likely_compressed: 3,
        variant: 4,
        similar_keep: 5,
        no_baseline_match: 6,
        inconclusive: 7,
        not_evaluated: 0,
        error: 105,
        pending_review: 0,
        approved_for_recycle: 0,
        rejected_keep: 0,
        recycled: 0,
        restored: 0,
        permanently_deleted: 0
      }
    })
    await wrapper.vm.$nextTick()

    const badges = wrapper.findAll('[data-test^="collapsed-stat-"]')
    expect(badges).toHaveLength(8)
    expect(badges.map(badge => badge.text())).toEqual(['2', '3', '4', '5', '6', '7', '0', '99+'])
    expect(badges[0].attributes('aria-label')).toBe('完全重复：2 张图片')
    expect(badges[6].classes()).toContain('is-zero')
    expect(badges[7].attributes('aria-label')).toBe('错误：105 张图片')
    expect(wrapper.find('[data-test="collapsed-tools-divider"]').exists()).toBe(true)

    await wrapper.get('[data-test="collapsed-open-difference-finder"]').trigger('click')
    await wrapper.get('[data-test="collapsed-open-image-metrics"]').trigger('click')
    await flushPromises()

    expect(webviewWindowMocks.getByLabel).toHaveBeenCalledWith('difference-finder')
    expect(openImageMetricsWindow).toHaveBeenCalledTimes(1)

    await wrapper.get('[aria-label="展开左栏"]').trigger('click')
    expect(wrapper.find('[data-test="collapsed-stats"]').exists()).toBe(false)
  })
})
