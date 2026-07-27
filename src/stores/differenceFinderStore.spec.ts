import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useDifferenceFinderStore } from './differenceFinderStore'
import type { DifferenceMatchItem, RenamePreviewItem } from '@/api/differenceFinder'

const apiMocks = vi.hoisted(() => ({
  previewDifferenceRename: vi.fn()
}))

vi.mock('@/api/differenceFinder', async importOriginal => ({
  ...await importOriginal<typeof import('@/api/differenceFinder')>(),
  previewDifferenceRename: apiMocks.previewDifferenceRename
}))

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}

function match(fileName: string, classification: DifferenceMatchItem['classification'] = 'variant'): DifferenceMatchItem {
  const filePath = `C:\\images\\${fileName}`
  return {
    filePath,
    fileName,
    sourceRoot: 'C:\\images',
    relativePath: fileName,
    fileSize: 1,
    modifiedAt: 1,
    width: 1,
    height: 1,
    format: 'jpg',
    blake3Hash: `hash-${fileName}`,
    classification,
    bestReferenceId: 'ref-1',
    relations: [{
      referenceId: 'ref-1', referencePath: 'C:\\ref.png', classification, phashDistance: 1, similarity: 0.9
    }]
  }
}

function preview(item: DifferenceMatchItem, proposedName: string): RenamePreviewItem {
  return {
    sourcePath: item.filePath,
    originalName: item.fileName,
    proposedName,
    targetPath: `C:\\images\\${proposedName}`,
    issues: [],
    blocking: false
  }
}

describe('differenceFinderStore rename preview consistency', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('keeps the newest preview when requests resolve out of order', async () => {
    const store = useDifferenceFinderStore()
    const item = match('first.jpg')
    store.references = [{ id: 'ref-1', name: 'ref.png', path: 'C:\\ref.png' }]
    store.matches = [item]
    store.orderedPaths = [item.filePath]
    store.selectedPaths = [item.filePath]
    const first = deferred<RenamePreviewItem[]>()
    const second = deferred<RenamePreviewItem[]>()
    apiMocks.previewDifferenceRename.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)

    const olderRequest = store.generateRenamePreview({ mode: 'simple', template: 'old-$name.$ext' })
    const newestRequest = store.generateRenamePreview({ mode: 'simple', template: 'new-$name.$ext' })
    second.resolve([preview(item, 'new-first.jpg')])
    await newestRequest
    first.resolve([preview(item, 'old-first.jpg')])
    await olderRequest

    expect(store.renamePreview[0].proposedName).toBe('new-first.jpg')
    expect(store.isPreviewing).toBe(false)
  })

  it('does not restore a pending preview after selection is cleared', async () => {
    const store = useDifferenceFinderStore()
    const item = match('first.jpg')
    store.references = [{ id: 'ref-1', name: 'ref.png', path: 'C:\\ref.png' }]
    store.matches = [item]
    store.orderedPaths = [item.filePath]
    store.selectedPaths = [item.filePath]
    const request = deferred<RenamePreviewItem[]>()
    apiMocks.previewDifferenceRename.mockReturnValueOnce(request.promise)

    const pending = store.generateRenamePreview({ mode: 'simple', template: '$name.$ext' })
    store.clearSelection()
    request.resolve([preview(item, 'stale.jpg')])
    await pending

    expect(store.renamePreview).toEqual([])
    expect(store.isPreviewing).toBe(false)
  })

  it('deselects only files in the current filter', () => {
    const store = useDifferenceFinderStore()
    const visible = match('visible.jpg', 'variant')
    const hidden = match('hidden.jpg', 'exact')
    store.matches = [visible, hidden]
    store.orderedPaths = [visible.filePath, hidden.filePath]
    store.selectedPaths = [visible.filePath, hidden.filePath]
    store.classificationFilter = 'variant'

    store.deselectFiltered()

    expect(store.selectedPaths).toEqual([hidden.filePath])
  })
})
