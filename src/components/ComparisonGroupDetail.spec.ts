import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import { reactive } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ComparisonGroupDetail from './ComparisonGroupDetail.vue'

const apiMocks = vi.hoisted(() => ({
  getGroupSimilarityScores: vi.fn(async () => ([{
    left_image_id: 1,
    right_image_id: 2,
    ssim_score: 0.971947,
    error_message: null
  }])),
  batchRecycleImages: vi.fn()
}))

const clipboardWrite = vi.fn(async () => undefined)

const store = reactive({
  currentRunId: 'run-1',
  selectedGroup: null as any,
  selectedMemberId: 1 as number | null,
  checkedImageIds: [] as number[],
  qualitySelectionThreshold: 0.8,
  selectGroupMember: vi.fn(),
  setQualitySelectionThreshold: vi.fn((value: number) => {
    store.qualitySelectionThreshold = value
  }),
  refreshAnalysisData: vi.fn(async () => undefined)
})

vi.mock('@/api/comparison', () => ({
  getGroupSimilarityScores: apiMocks.getGroupSimilarityScores,
  batchRecycleImages: apiMocks.batchRecycleImages
}))

vi.mock('@/stores/comparisonStore', () => ({
  useComparisonStore: () => store
}))

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: vi.fn(async () => undefined)
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined)
}))

function member(id: number, overrides: Record<string, unknown> = {}) {
  return {
    image_id: id,
    file_path: `D:/images/${id}.png`,
    relative_path: `${id}.png`,
    file_size: id === 1 ? 4_000_000 : 120_000,
    width: id === 1 ? 1500 : 800,
    height: id === 1 ? 2400 : 1280,
    phash: `000000000000000${id}`,
    phash_distance_to_reference: id === 1 ? null : 1,
    role: id === 1 ? 'reference' : 'lower_quality',
    role_label: id === 1 ? '组内参考图' : '疑似低质量',
    reference_image_id: id === 1 ? null : 1,
    reference_relative_path: id === 1 ? null : '1.png',
    ssim_score: id === 1 ? 1 : 0.971947,
    ssim_cluster_key: '1',
    is_low_quality_suggestion: id !== 1,
    ...overrides
  }
}

describe('ComparisonGroupDetail threshold semantics', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: clipboardWrite }
    })
    store.selectedGroup = {
      group_index: 1,
      representative_image_id: 1,
      representative_file_name: '1.png',
      member_count: 2,
      has_low_quality_suggestion: true,
      members: [member(1), member(2)]
    }
    store.selectedMemberId = 1
    store.checkedImageIds = []
    store.qualitySelectionThreshold = 0.8
  })

  it('explains both decisions and labels candidate scores as SSIM against the current original', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    await wrapper.findAll('button')[0].trigger('click')
    await flushPromises()
    const pageText = document.body.textContent || ''
    const tooltipContents = wrapper
      .findAllComponents({ name: 'ElTooltip' })
      .map((tooltip) => tooltip.props('content'))
      .filter((content): content is string => typeof content === 'string')

    expect(pageText).toContain('原图拆分阈值')
    expect(pageText).toContain('调低会拆出更多原图，调高会减少原图数量')
    expect(pageText).toContain('自动勾选删除阈值')
    expect(pageText).toContain('这里只改变自动勾选，不改变图片归属')
    expect(pageText).toContain('两个阈值只改变判断规则，不改变标准 SSIM 算法或已计算数值')
    expect(wrapper.text()).toContain('与原图 SSIM')
    expect(tooltipContents).toContain(
      '候选图与当前所在行原图的标准 SSIM。该值用于自动勾选删除阈值，不用于决定原图拆分。'
    )

    wrapper.unmount()
  })

  it('does not label a stored score from another reference as SSIM against the current original', async () => {
    apiMocks.getGroupSimilarityScores.mockResolvedValueOnce([])
    store.selectedGroup = {
      group_index: 1,
      representative_image_id: 1,
      representative_file_name: '1.png',
      member_count: 2,
      has_low_quality_suggestion: true,
      members: [
        member(1),
        member(3, {
          reference_image_id: 999,
          reference_relative_path: 'missing-reference.png',
          ssim_score: 0.999
        })
      ]
    }

    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    const candidateTable = wrapper.get('.thumbnail-table')
    const tooltipContents = wrapper
      .findAllComponents({ name: 'ElTooltip' })
      .map((tooltip) => tooltip.props('content'))
    expect(candidateTable.text()).toContain('—')
    expect(candidateTable.text()).not.toContain('0.999000')
    expect(candidateTable.get('input[type="checkbox"]').attributes('disabled')).toBeDefined()
    expect(tooltipContents).toContain('缺少与当前原图的标准 SSIM，未自动勾选')

    wrapper.unmount()
  })

  it('copies checked delete file names with commas before the threshold action', async () => {
    apiMocks.getGroupSimilarityScores.mockResolvedValueOnce([
      { left_image_id: 1, right_image_id: 2, ssim_score: 0.971947, error_message: null },
      { left_image_id: 1, right_image_id: 3, ssim_score: 0.96, error_message: null }
    ])
    store.selectedGroup = {
      group_index: 1,
      representative_image_id: 1,
      representative_file_name: '1.png',
      member_count: 3,
      has_low_quality_suggestion: true,
      members: [member(1), member(2), member(3)]
    }

    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    const actionLabels = wrapper.findAll('.detail-actions button').map((button) => button.text())
    expect(actionLabels).toContain('复制已选文件名')
    expect(actionLabels.indexOf('复制已选文件名')).toBeLessThan(actionLabels.indexOf('判断阈值'))

    await wrapper.get('[data-test="copy-checked-names"]').trigger('click')
    await flushPromises()

    expect(clipboardWrite).toHaveBeenCalledWith('2.png,3.png')
    wrapper.unmount()
  })

  it('selects and clears every deletable candidate from the table header', async () => {
    apiMocks.getGroupSimilarityScores.mockResolvedValueOnce([
      { left_image_id: 1, right_image_id: 2, ssim_score: 0.971947, error_message: null },
      { left_image_id: 1, right_image_id: 3, ssim_score: 0.96, error_message: null }
    ])
    store.selectedGroup = {
      group_index: 1,
      representative_image_id: 1,
      representative_file_name: '1.png',
      member_count: 3,
      has_low_quality_suggestion: true,
      members: [member(1), member(2), member(3)]
    }

    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()
    store.checkedImageIds = []
    await wrapper.vm.$nextTick()

    const selectAll = wrapper.get('[data-test="select-all-delete"]')
    await selectAll.get('input').setValue(true)
    expect(store.checkedImageIds).toEqual([2, 3])

    await selectAll.get('input').setValue(false)
    expect(store.checkedImageIds).toEqual([])
    wrapper.unmount()
  })
})
