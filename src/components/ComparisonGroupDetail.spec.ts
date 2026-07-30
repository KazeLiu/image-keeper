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
  batchRecycleImages: vi.fn(),
  startGroupSimilarityBackfill: vi.fn(async () => undefined)
}))

const clipboardWrite = vi.fn(async () => undefined)

const store = reactive({
  currentRunId: 'run-1',
  selectedGroup: null as any,
  selectedMemberId: 1 as number | null,
  checkedImageIds: [] as number[],
  appliedGroupingDistance: 10,
  isRefreshingGroups: false,
  groupingDataRevision: 0,
  selectGroupMember: vi.fn(),
  markGroupSimilarityStatus: vi.fn(),
  refreshAnalysisData: vi.fn(async () => undefined)
})

vi.mock('@/api/comparison', () => ({
  getGroupSimilarityScores: apiMocks.getGroupSimilarityScores,
  batchRecycleImages: apiMocks.batchRecycleImages,
  startGroupSimilarityBackfill: apiMocks.startGroupSimilarityBackfill
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
    store.appliedGroupingDistance = 10
    store.isRefreshingGroups = false
    store.groupingDataRevision = 0
    window.localStorage.setItem('imagekeeper:original-recognition-ssim', '0.985')
  })

  it('explains the original split decision without presenting checkbox selection as a threshold decision', async () => {
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
    expect(pageText).not.toContain('自动勾选')
    expect(pageText).not.toContain('勾选阈值')
    expect(wrapper.text()).toContain('与原图 SSIM')
    expect(tooltipContents).toContain(
      '候选图与当前所在行原图的标准 SSIM，用于确认它被归到哪张原图下。'
    )

    wrapper.unmount()
  })

  it('allows manually selecting a thumbnail even when SSIM against the current original is unavailable', async () => {
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
    expect(candidateTable.get('input[type="checkbox"]').attributes('disabled')).toBeUndefined()
    expect(tooltipContents).not.toContain('缺少与当前原图的标准 SSIM，未自动勾选')

    await candidateTable.get('input[type="checkbox"]').setValue(true)
    expect(store.checkedImageIds).toEqual([3])

    wrapper.unmount()
  })

  it('does not automatically select thumbnails after group comparison finishes', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    expect(store.checkedImageIds).toEqual([])
    wrapper.unmount()
  })

  it('drops a checked thumbnail when the judgment threshold reclassifies it as an original', async () => {
    apiMocks.getGroupSimilarityScores.mockResolvedValueOnce([])
    store.selectedGroup = {
      group_index: 1,
      representative_image_id: 1,
      representative_file_name: '1.png',
      member_count: 2,
      has_low_quality_suggestion: false,
      members: [
        member(1),
        member(3, {
          file_size: 3_000_000,
          width: 1450,
          height: 2320,
          ssim_score: 0.97,
          is_low_quality_suggestion: false
        })
      ]
    }
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    await wrapper.get('.thumbnail-table input[type="checkbox"]').setValue(true)
    expect(store.checkedImageIds).toEqual([3])

    wrapper.findAllComponents({ name: 'ElSlider' })[0].vm.$emit('input', 0)
    await flushPromises()

    expect(store.checkedImageIds).toEqual([])
    wrapper.unmount()
  })

  it('copies manually checked file names with commas before the threshold action', async () => {
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

    await wrapper.get('[data-test="select-all-delete"] input').setValue(true)
    await wrapper.get('[data-test="copy-checked-names"]').trigger('click')
    await flushPromises()

    expect(clipboardWrite).toHaveBeenCalledWith('2.png,3.png')
    wrapper.unmount()
  })

  it('selects and clears every thumbnail from the table header', async () => {
    apiMocks.getGroupSimilarityScores.mockResolvedValueOnce([
      { left_image_id: 1, right_image_id: 2, ssim_score: 0.971947, error_message: null },
      { left_image_id: 1, right_image_id: 3, ssim_score: 0.2, error_message: null }
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
    const selectAll = wrapper.get('[data-test="select-all-delete"]')
    await selectAll.get('input').setValue(true)
    expect(store.checkedImageIds).toEqual([2, 3])

    await selectAll.get('input').setValue(false)
    expect(store.checkedImageIds).toEqual([])
    wrapper.unmount()
  })

  it('starts background backfill after the current group scores are ready', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    expect(apiMocks.startGroupSimilarityBackfill).toHaveBeenCalledWith('run-1', 10, [1])
    wrapper.unmount()
  })

  it('revalidates current group scores through the backend before restarting backfill', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()
    apiMocks.getGroupSimilarityScores.mockClear()
    apiMocks.startGroupSimilarityBackfill.mockClear()

    const recalculateButton = wrapper
      .findAll('button')
      .find((button) => button.text().includes('重新计算图片归属'))
    expect(recalculateButton).toBeDefined()
    await recalculateButton!.trigger('click')
    await flushPromises()

    expect(apiMocks.getGroupSimilarityScores).toHaveBeenCalledTimes(1)
    expect(apiMocks.startGroupSimilarityBackfill).toHaveBeenCalledWith('run-1', 10, [1])
    wrapper.unmount()
  })

  it('starts backfill for the refreshed grouping distance only after refresh finishes', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()
    apiMocks.getGroupSimilarityScores.mockClear()
    apiMocks.startGroupSimilarityBackfill.mockClear()

    store.isRefreshingGroups = true
    store.appliedGroupingDistance = 14
    await flushPromises()
    expect(apiMocks.getGroupSimilarityScores).not.toHaveBeenCalled()

    store.groupingDataRevision += 1
    store.isRefreshingGroups = false
    await flushPromises()
    expect(apiMocks.getGroupSimilarityScores).toHaveBeenCalledTimes(1)
    expect(apiMocks.startGroupSimilarityBackfill).toHaveBeenCalledWith('run-1', 14, [1])
    wrapper.unmount()
  })

  it('does not let an old group request start backfill for a newly applied distance', async () => {
    let resolveOldRequest: ((scores: any[]) => void) | null = null
    apiMocks.getGroupSimilarityScores.mockImplementationOnce(() => new Promise((resolve) => {
      resolveOldRequest = resolve
    }))
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    store.isRefreshingGroups = true
    store.appliedGroupingDistance = 14
    await flushPromises()
    resolveOldRequest?.([])
    await flushPromises()
    expect(apiMocks.startGroupSimilarityBackfill).not.toHaveBeenCalled()

    store.groupingDataRevision += 1
    store.isRefreshingGroups = false
    await flushPromises()
    expect(apiMocks.getGroupSimilarityScores).toHaveBeenCalledTimes(2)
    expect(apiMocks.startGroupSimilarityBackfill).toHaveBeenCalledWith('run-1', 14, [1])
    wrapper.unmount()
  })

  it('does not start a new-distance backfill when grouping refresh fails', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()
    apiMocks.getGroupSimilarityScores.mockClear()
    apiMocks.startGroupSimilarityBackfill.mockClear()

    store.isRefreshingGroups = true
    store.appliedGroupingDistance = 14
    await flushPromises()
    store.isRefreshingGroups = false
    await flushPromises()

    expect(apiMocks.getGroupSimilarityScores).not.toHaveBeenCalled()
    expect(apiMocks.startGroupSimilarityBackfill).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('uses original source group indices after manual groups are renumbered', async () => {
    store.selectedGroup = {
      ...store.selectedGroup,
      group_index: 2,
      source_group_indices: [3]
    }

    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    expect(apiMocks.startGroupSimilarityBackfill).toHaveBeenCalledWith('run-1', 10, [3])
    wrapper.unmount()
  })

  it('starts background backfill when the current group needs no pair comparison', async () => {
    store.selectedGroup = {
      group_index: 1,
      representative_image_id: 1,
      representative_file_name: '1.png',
      member_count: 1,
      has_low_quality_suggestion: false,
      members: [member(1)]
    }

    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    expect(apiMocks.getGroupSimilarityScores).not.toHaveBeenCalled()
    expect(apiMocks.startGroupSimilarityBackfill).toHaveBeenCalledWith('run-1', 10, [1])
    wrapper.unmount()
  })

  it('shows a full-path viewer beside copy and open for originals and thumbnails', async () => {
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()

    const pathViewers = wrapper.findAll('[data-test="view-full-path"]')
    expect(pathViewers).toHaveLength(2)
    expect(pathViewers.every((viewer) => viewer.text() === '查看')).toBe(true)

    const tooltipContents = wrapper
      .findAllComponents({ name: 'ElTooltip' })
      .map((tooltip) => tooltip.props('content'))
    expect(tooltipContents).toContain('D:/images/1.png')
    expect(tooltipContents).toContain('D:/images/2.png')

    wrapper.unmount()
  })

  it('starts the newly selected group immediately while the previous group is still computing', async () => {
    let resolveFirstRequest: ((scores: any[]) => void) | null = null
    apiMocks.getGroupSimilarityScores.mockImplementationOnce(() => new Promise((resolve) => {
      resolveFirstRequest = resolve
    }))
    const wrapper = mount(ComparisonGroupDetail, {
      attachTo: document.body,
      global: { plugins: [ElementPlus] }
    })
    await flushPromises()
    expect(apiMocks.getGroupSimilarityScores).toHaveBeenCalledTimes(1)

    const nextGroup = {
      group_index: 2,
      representative_image_id: 3,
      representative_file_name: '3.png',
      member_count: 2,
      has_low_quality_suggestion: true,
      members: [member(3), member(4)]
    }
    store.selectedGroup = nextGroup
    await flushPromises()

    expect(apiMocks.getGroupSimilarityScores).toHaveBeenCalledTimes(2)
    expect(apiMocks.getGroupSimilarityScores).toHaveBeenLastCalledWith(
      'run-1',
      [3, 4],
      expect.stringMatching(/^group-/),
      10,
      2
    )
    expect(store.markGroupSimilarityStatus).toHaveBeenCalledWith(
      nextGroup,
      'running',
      '正在优先比对当前分组 SSIM'
    )

    resolveFirstRequest?.([])
    await flushPromises()
    wrapper.unmount()
  })
})
