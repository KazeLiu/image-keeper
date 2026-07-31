import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import { reactive } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ComparisonResults from './ComparisonResults.vue'

const statusByGroup = new Map<number, { status: string; message: string }>()

const store = reactive({
  stats: { comparison_total: 6 },
  groups: [] as any[],
  groupingDistance: 10,
  appliedGroupingDistance: 10,
  isRefreshingGroups: false,
  groupEditMode: false,
  selectedGroupIds: [] as number[],
  selectedGroupIndex: 1 as number | null,
  refreshAnalysisData: vi.fn(async () => undefined),
  setGroupingDistance: vi.fn(),
  selectGroup: vi.fn(),
  mergeSelectedGroups: vi.fn(),
  getGroupSimilarityStatus: vi.fn((group: { group_index: number }) =>
    statusByGroup.get(group.group_index) || {
      status: 'pending',
      message: '尚未比对，正在等待后台 SSIM 计算'
    }
  )
})

vi.mock('@/stores/comparisonStore', () => ({
  useComparisonStore: () => store
}))

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://${path}`
}))

function group(groupIndex: number) {
  return {
    group_index: groupIndex,
    representative_image_id: groupIndex,
    representative_file_name: `${groupIndex}.png`,
    member_count: 2,
    has_low_quality_suggestion: false,
    members: [
      {
        image_id: groupIndex,
        file_path: `D:/images/${groupIndex}.png`,
        relative_path: `${groupIndex}.png`,
        file_size: 1000,
        width: 100,
        height: 100,
        role: 'reference',
        role_label: '组内参考图',
        ssim_cluster_key: String(groupIndex),
        is_low_quality_suggestion: false
      }
    ]
  }
}

describe('ComparisonResults group SSIM status', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    store.groups = [group(1), group(2), group(3)]
    store.selectedGroupIndex = 1
    statusByGroup.clear()
    statusByGroup.set(1, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })
    statusByGroup.set(2, { status: 'running', message: '正在优先比对当前分组 SSIM' })
    statusByGroup.set(3, { status: 'pending', message: '尚未比对，正在等待后台 SSIM 计算' })
  })

  it('shows completed, running, and pending lights with readable hover descriptions', async () => {
    const wrapper = mount(ComparisonResults, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    const lights = wrapper.findAll('[data-test="group-ssim-status"]')
    expect(lights).toHaveLength(3)
    expect(lights.map((light) => light.classes())).toEqual([
      expect.arrayContaining(['is-completed']),
      expect.arrayContaining(['is-running']),
      expect.arrayContaining(['is-pending'])
    ])
    expect(lights.map((light) => light.attributes('aria-label'))).toEqual([
      'SSIM 状态：组内 SSIM 已比对完成并缓存',
      'SSIM 状态：正在优先比对当前分组 SSIM',
      'SSIM 状态：尚未比对，正在等待后台 SSIM 计算'
    ])

    const tooltipContents = wrapper
      .findAllComponents({ name: 'ElTooltip' })
      .map((tooltip) => tooltip.props('content'))
    expect(tooltipContents).toEqual(expect.arrayContaining([
      '组内 SSIM 已比对完成并缓存',
      '正在优先比对当前分组 SSIM',
      '尚未比对，正在等待后台 SSIM 计算'
    ]))

    wrapper.unmount()
  })

  it('renders at most one hundred groups at a time', async () => {
    store.groups = Array.from({ length: 250 }, (_, index) => group(index + 1))

    const wrapper = mount(ComparisonResults, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    expect(wrapper.findAll('[data-test="group-ssim-status"]')).toHaveLength(100)
    expect(wrapper.find('[data-test="group-pagination"]').exists()).toBe(true)

    store.selectedGroupIndex = 201
    await flushPromises()
    const visibleGroups = wrapper.findComponent({ name: 'ElTable' }).props('data') as any[]
    expect(visibleGroups[0].group_index).toBe(201)

    wrapper.unmount()
  })
})
