import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import { describe, expect, it, vi } from 'vitest'
import SearchSetupPanel from './SearchSetupPanel.vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

describe('SearchSetupPanel', () => {
  it('emits completion only after the search resolves', async () => {
    const pinia = createPinia()
    const wrapper = mount(SearchSetupPanel, {
      global: { plugins: [pinia, ElementPlus] }
    })
    const store = useDifferenceFinderStore(pinia)
    store.references = [{ id: 'ref-1', name: 'ref.png', path: 'C:\\ref.png' }]
    store.targetRoots = ['C:\\images']
    vi.spyOn(store, 'search').mockResolvedValue(undefined)
    await wrapper.vm.$nextTick()

    await wrapper.get('[data-test="start-search"]').trigger('click')
    await flushPromises()

    expect(store.search).toHaveBeenCalledTimes(1)
    expect(wrapper.emitted('searchComplete')).toHaveLength(1)
  })
})
