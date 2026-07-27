import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import BatchRenameGrid from './BatchRenameGrid.vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'
import type { DifferenceMatchItem } from '@/api/differenceFinder'

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => path,
  invoke: vi.fn()
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

const apiMocks = vi.hoisted(() => ({
  previewDifferenceRename: vi.fn(async (items: Array<{ sourcePath: string }>) => items.map(item => ({
    sourcePath: item.sourcePath,
    originalName: item.sourcePath.split('\\').pop() || item.sourcePath,
    proposedName: item.sourcePath.split('\\').pop() || item.sourcePath,
    targetPath: item.sourcePath,
    issues: [],
    blocking: false
  })))
}))

vi.mock('@/api/differenceFinder', async importOriginal => ({
  ...await importOriginal<typeof import('@/api/differenceFinder')>(),
  previewDifferenceRename: apiMocks.previewDifferenceRename
}))

function match(fileName: string): DifferenceMatchItem {
  const filePath = `C:\\images\\${fileName}`
  return {
    filePath,
    fileName,
    sourceRoot: 'C:\\images',
    relativePath: fileName,
    fileSize: 1024,
    modifiedAt: 1,
    width: 800,
    height: 1200,
    format: 'jpg',
    blake3Hash: `hash-${fileName}`,
    classification: 'variant',
    bestReferenceId: 'ref-1',
    relations: [{
      referenceId: 'ref-1',
      referencePath: 'C:\\ref.png',
      classification: 'variant',
      phashDistance: 2,
      similarity: 0.97
    }]
  }
}

function renamePreview(item: DifferenceMatchItem) {
  return {
    sourcePath: item.filePath,
    originalName: item.fileName,
    proposedName: item.fileName,
    targetPath: item.filePath,
    issues: [],
    blocking: false
  }
}

describe('BatchRenameGrid unified file table', () => {
  beforeEach(() => vi.clearAllMocks())

  it('shows all matching files and limits rename preparation to checked rows', async () => {
    const pinia = createPinia()
    const store = useDifferenceFinderStore(pinia)
    store.references = [{ id: 'ref-1', name: 'ref.png', path: 'C:\\ref.png' }]
    store.matches = [match('first.jpg'), match('second.jpg')]
    store.orderedPaths = store.matches.map(item => item.filePath)

    const wrapper = mount(BatchRenameGrid, {
      global: { plugins: [pinia, ElementPlus] }
    })
    await flushPromises()

    expect(wrapper.findAll('[data-test="file-row"]')).toHaveLength(2)
    expect(store.selectedPaths).toEqual([])

    await wrapper.findAll('[data-test="file-select"]')[0].get('input').setValue(true)
    await flushPromises()

    expect(store.selectedPaths).toEqual(['C:\\images\\first.jpg'])
    expect(apiMocks.previewDifferenceRename).toHaveBeenCalledTimes(1)
    expect(apiMocks.previewDifferenceRename.mock.calls[0][0]).toHaveLength(1)
  })

  it('shows the relation for the active reference filter', async () => {
    const pinia = createPinia()
    const store = useDifferenceFinderStore(pinia)
    const item = match('first.jpg')
    item.relations.push({
      referenceId: 'ref-2',
      referencePath: 'C:\\other.png',
      classification: 'exact',
      phashDistance: 0,
      similarity: 0.88
    })
    store.references = [
      { id: 'ref-1', name: 'primary.png', path: 'C:\\ref.png' },
      { id: 'ref-2', name: 'other.png', path: 'C:\\other.png' }
    ]
    store.matches = [item]
    store.orderedPaths = [item.filePath]
    store.activeReferenceId = 'ref-2'

    const wrapper = mount(BatchRenameGrid, {
      global: { plugins: [pinia, ElementPlus] }
    })
    await flushPromises()

    expect(wrapper.get('.match-cell').text()).toContain('other · 0.88')
    expect(wrapper.get('.match-cell').text()).not.toContain('primary')
  })

  it('refreshes selected rename previews when the reference filter changes', async () => {
    const pinia = createPinia()
    const store = useDifferenceFinderStore(pinia)
    const item = match('first.jpg')
    item.relations.push({
      referenceId: 'ref-2', referencePath: 'C:\\other.png', classification: 'variant', phashDistance: 3, similarity: 0.9
    })
    store.references = [
      { id: 'ref-1', name: 'primary.png', path: 'C:\\ref.png' },
      { id: 'ref-2', name: 'other.png', path: 'C:\\other.png' }
    ]
    store.matches = [item]
    store.orderedPaths = [item.filePath]
    store.selectedPaths = [item.filePath]

    mount(BatchRenameGrid, { global: { plugins: [pinia, ElementPlus] } })
    await flushPromises()
    expect(apiMocks.previewDifferenceRename).toHaveBeenCalledTimes(1)

    store.activeReferenceId = 'ref-2'
    await flushPromises()

    expect(apiMocks.previewDifferenceRename).toHaveBeenCalledTimes(2)
  })

  it('disables execution while the latest rename preview is pending', async () => {
    const pinia = createPinia()
    const store = useDifferenceFinderStore(pinia)
    const item = match('first.jpg')
    store.references = [{ id: 'ref-1', name: 'ref.png', path: 'C:\\ref.png' }]
    store.matches = [item]
    store.orderedPaths = [item.filePath]
    store.selectedPaths = [item.filePath]
    store.renamePreview = [renamePreview(item)]

    const wrapper = mount(BatchRenameGrid, { global: { plugins: [pinia, ElementPlus] } })
    await flushPromises()
    store.isPreviewing = true
    await wrapper.vm.$nextTick()

    expect(wrapper.get('[data-test="execute-rename"]').attributes('disabled')).toBeDefined()
  })
})
