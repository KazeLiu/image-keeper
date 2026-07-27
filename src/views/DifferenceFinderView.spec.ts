import { defineComponent } from 'vue'
import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import { describe, expect, it } from 'vitest'
import DifferenceFinderView from './DifferenceFinderView.vue'

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
  return mount(DifferenceFinderView, {
    global: {
      plugins: [createPinia(), ElementPlus],
      stubs: {
        ReferenceImageStrip: ReferenceImageStripStub,
        SearchSetupPanel: SearchSetupPanelStub,
        DifferenceResultList: true,
        BatchRenameGrid: BatchRenameGridStub
      }
    }
  })
}

describe('DifferenceFinderView two-step workflow', () => {
  it('shows only search setup before a search is completed', () => {
    const wrapper = mountView()

    expect(wrapper.get('[data-test="reference-setup"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="complete-search"]').exists()).toBe(true)
    expect(wrapper.get('.results-stage').attributes('style')).toContain('display: none')
    expect(wrapper.get('[data-test="step-setup"]').attributes('aria-current')).toBe('step')
  })

  it('moves to the file table after search and can return to setup', async () => {
    const wrapper = mountView()

    await wrapper.get('[data-test="complete-search"]').trigger('click')

    expect(wrapper.get('.setup-stage').attributes('style')).toContain('display: none')
    expect(wrapper.get('[data-test="file-table"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="step-results"]').attributes('aria-current')).toBe('step')

    await wrapper.get('[data-test="edit-search"]').trigger('click')

    expect(wrapper.get('[data-test="reference-setup"]').exists()).toBe(true)
    expect(wrapper.get('.results-stage').attributes('style')).toContain('display: none')
  })

  it('preserves rename drafts while returning to edit search inputs', async () => {
    const wrapper = mountView()

    await wrapper.get('[data-test="complete-search"]').trigger('click')
    await wrapper.get('[data-test="rename-draft"]').setValue('kept-name.jpg')
    await wrapper.get('[data-test="edit-search"]').trigger('click')
    await wrapper.get('[data-test="complete-search"]').trigger('click')

    expect((wrapper.get('[data-test="rename-draft"]').element as HTMLInputElement).value).toBe('kept-name.jpg')
  })
})
