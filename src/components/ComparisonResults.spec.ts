import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import { reactive } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ComparisonResults from './ComparisonResults.vue'

const statusByGroup = reactive(new Map<number, { status: string; message: string }>())

const store = reactive({
  stats: { comparison_total: 6 },
  groups: [] as any[],
  groupingDistance: 10,
  appliedGroupingDistance: 10,
  isRefreshingGroups: false,
  groupEditMode: false,
  originalRecognitionThreshold: 0.985,
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

function group(groupIndex: number, hasThumbnail = true) {
  const secondImageId = groupIndex + 10_000
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
        file_size: 4_000_000,
        width: 1500,
        height: 2400,
        role: 'reference',
        role_label: '组内参考图',
        ssim_score: 1,
        ssim_cluster_key: String(groupIndex),
        is_low_quality_suggestion: false
      },
      {
        image_id: secondImageId,
        file_path: `D:/images/${secondImageId}.png`,
        relative_path: `${secondImageId}.png`,
        file_size: hasThumbnail ? 120_000 : 3_000_000,
        width: hasThumbnail ? 800 : 1450,
        height: hasThumbnail ? 1280 : 2320,
        role: hasThumbnail ? 'lower_quality' : 'similar_keep',
        role_label: hasThumbnail ? '疑似低质量' : '相似保留',
        reference_image_id: groupIndex,
        reference_relative_path: `${groupIndex}.png`,
        ssim_score: hasThumbnail ? 0.97 : 0.99,
        ssim_cluster_key: String(groupIndex),
        is_low_quality_suggestion: hasThumbnail
      }
    ]
  }
}

describe('ComparisonResults group SSIM status', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    store.groups = [group(1), group(2), group(3)]
    store.selectedGroupIndex = 1
    store.groupEditMode = false
    store.originalRecognitionThreshold = 0.985
    store.selectedGroupIds = []
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

  it('shows whether each completed SSIM group has thumbnails without opening it', async () => {
    store.groups = [group(7, true), group(11, false), group(15, true)]
    statusByGroup.clear()
    statusByGroup.set(7, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })
    statusByGroup.set(11, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })
    statusByGroup.set(15, { status: 'running', message: '正在后台比对本组 SSIM' })

    const wrapper = mount(ComparisonResults, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    const states = wrapper.findAll('[data-test="group-thumbnail-status"]')
    expect(states.map((state) => state.text())).toEqual(['有缩略图', '无缩略图', '比对中'])
    wrapper.unmount()
  })

  it('adds newly completed thumbnail groups to the live filter without renumbering', async () => {
    store.groups = [group(7, true), group(11, false), group(15, true)]
    statusByGroup.clear()
    statusByGroup.set(7, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })
    statusByGroup.set(11, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })
    statusByGroup.set(15, { status: 'pending', message: '尚未比对，正在等待后台 SSIM 计算' })

    const wrapper = mount(ComparisonResults, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    await wrapper.get('[data-test="thumbnail-group-filter"] input').setValue(true)
    let visibleGroups = wrapper.findComponent({ name: 'ElTable' }).props('data') as any[]
    expect(visibleGroups.map((item) => item.group_index)).toEqual([7])

    statusByGroup.set(15, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })
    await flushPromises()
    visibleGroups = wrapper.findComponent({ name: 'ElTable' }).props('data') as any[]
    expect(visibleGroups.map((item) => item.group_index)).toEqual([7, 15])
    wrapper.unmount()
  })

  it('updates the live thumbnail filter when the original recognition threshold changes', async () => {
    store.groups = [group(11, false)]
    statusByGroup.clear()
    statusByGroup.set(11, { status: 'completed', message: '组内 SSIM 已比对完成并缓存' })

    const wrapper = mount(ComparisonResults, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    await wrapper.get('[data-test="thumbnail-group-filter"] input').setValue(true)
    expect(wrapper.findComponent({ name: 'ElTable' }).exists()).toBe(false)

    store.originalRecognitionThreshold = 0.995
    await flushPromises()

    const visibleGroups = wrapper.findComponent({ name: 'ElTable' }).props('data') as any[]
    expect(visibleGroups.map((item) => item.group_index)).toEqual([11])
    wrapper.unmount()
  })
})
