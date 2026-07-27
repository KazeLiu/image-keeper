import { defineComponent } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus, { ElMessageBox } from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import DifferenceFinderView from './DifferenceFinderView.vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

const ReferenceImageStripStub = defineComponent({
  name: 'ReferenceImageStrip',
  template: '<div data-test="reference-setup">参考图片设置</div>'
})

const SearchSetupPanelStub = defineComponent({
  name: 'SearchSetupPanel',
  emits: ['searchComplete'],
  template: '<button data-test="complete-search" @click="$emit(\'searchComplete\')">完成查找</button>'
})

const BatchRenameGridStub = defineComponent({
  name: 'BatchRenameGrid',
  template: '<div data-test="file-table">文件表格<input data-test="rename-draft" /></div>'
})

function mountView() {
  const pinia = createPinia()
  const wrapper = mount(DifferenceFinderView, {
    global: {
      plugins: [pinia, ElementPlus],
      stubs: {
        ReferenceImageStrip: ReferenceImageStripStub,
        SearchSetupPanel: SearchSetupPanelStub,
        DifferenceResultList: true,
        BatchRenameGrid: BatchRenameGridStub
      }
    }
  })
  return { wrapper, store: useDifferenceFinderStore(pinia) }
}

describe('DifferenceFinderView two-step workflow', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('shows only search setup before a search is completed', () => {
    const { wrapper } = mountView()

    expect(wrapper.get('.finder-header').classes()).toContain('finder-card')
    expect(wrapper.get('.setup-grid').classes()).toContain('stacked')
    expect(wrapper.get('[data-test="reference-setup"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="complete-search"]').exists()).toBe(true)
    expect(wrapper.get('.results-stage').attributes('style')).toContain('display: none')
    expect(wrapper.get('[data-test="step-setup"]').attributes('aria-current')).toBe('step')
  })

  it('moves to the file table after search and can return to setup', async () => {
    const { wrapper, store } = mountView()
    store.matches = [{ filePath: 'C:\\result.jpg' } as any]
    store.selectedPaths = ['C:\\result.jpg']
    const confirm = vi.spyOn(ElMessageBox, 'confirm').mockResolvedValue('confirm')

    await wrapper.get('[data-test="complete-search"]').trigger('click')

    expect(wrapper.get('.setup-stage').attributes('style')).toContain('display: none')
    expect(wrapper.get('[data-test="file-table"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="step-results"]').attributes('aria-current')).toBe('step')

    await wrapper.get('[data-test="edit-search"]').trigger('click')
    await flushPromises()

    expect(confirm).toHaveBeenCalledWith(
      '返回后需要重新查找图片，是否继续？',
      '重新选择',
      expect.objectContaining({ confirmButtonText: '继续返回' })
    )
    expect(wrapper.get('[data-test="reference-setup"]').exists()).toBe(true)
    expect(wrapper.get('.results-stage').attributes('style')).toContain('display: none')
    expect(store.matches).toEqual([])
    expect(store.selectedPaths).toEqual([])
  })

  it('stays on the result table when returning is cancelled', async () => {
    const { wrapper } = mountView()
    vi.spyOn(ElMessageBox, 'confirm').mockRejectedValue('cancel')

    await wrapper.get('[data-test="complete-search"]').trigger('click')
    await wrapper.get('[data-test="edit-search"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="step-results"]').attributes('aria-current')).toBe('step')
    expect(wrapper.get('.setup-stage').attributes('style')).toContain('display: none')
  })

  it('preserves rename drafts while returning to edit search inputs', async () => {
    const { wrapper } = mountView()
    vi.spyOn(ElMessageBox, 'confirm').mockResolvedValue('confirm')

    await wrapper.get('[data-test="complete-search"]').trigger('click')
    await wrapper.get('[data-test="rename-draft"]').setValue('kept-name.jpg')
    await wrapper.get('[data-test="edit-search"]').trigger('click')
    await flushPromises()
    await wrapper.get('[data-test="complete-search"]').trigger('click')

    expect((wrapper.get('[data-test="rename-draft"]').element as HTMLInputElement).value).toBe('kept-name.jpg')
  })
})
