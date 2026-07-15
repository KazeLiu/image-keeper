import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  cancelDifferenceSearch,
  listenDifferenceSearchProgress,
  previewDifferenceRename,
  startDifferenceSearch,
  type DifferenceMatchItem,
  type DifferenceReferenceInput,
  type DifferenceSearchProgress,
  type MatchClassification,
  type OperationEntry,
  type RenamePreviewItem,
  type RenameRule,
  type SearchFileError
} from '@/api/differenceFinder'

export interface FinderReference extends DifferenceReferenceInput {
  name: string
}

export const useDifferenceFinderStore = defineStore('difference-finder', () => {
  const references = ref<FinderReference[]>([])
  const targetRoots = ref<string[]>([])
  const recursive = ref(true)
  const activeReferenceId = ref<string | null>(null)
  const classificationFilter = ref<MatchClassification | 'all'>('all')
  const matches = ref<DifferenceMatchItem[]>([])
  const errors = ref<SearchFileError[]>([])
  const selectedPaths = ref<string[]>([])
  const orderedPaths = ref<string[]>([])
  const progress = ref<DifferenceSearchProgress | null>(null)
  const isRunning = ref(false)
  const currentSessionId = ref<string | null>(null)
  const renamePreview = ref<RenamePreviewItem[]>([])
  const isPreviewing = ref(false)

  const filteredMatches = computed(() => matches.value.filter(item => {
    const referenceMatches = !activeReferenceId.value || item.relations.some(
      relation => relation.referenceId === activeReferenceId.value
    )
    const classificationMatches = classificationFilter.value === 'all'
      || classificationForItem(item) === classificationFilter.value
    return referenceMatches && classificationMatches
  }))

  const orderedSelectedMatches = computed(() => {
    const byPath = new Map(matches.value.map(item => [normalizePath(item.filePath), item]))
    return orderedPaths.value
      .map(path => byPath.get(normalizePath(path)))
      .filter((item): item is DifferenceMatchItem => Boolean(item))
      .filter(item => selectedPaths.value.some(path => normalizePath(path) === normalizePath(item.filePath)))
  })

  const canSearch = computed(() => references.value.length > 0 && targetRoots.value.length > 0 && !isRunning.value)
  const hasBlockingRenameIssues = computed(() => renamePreview.value.some(item => item.blocking))

  function addReferences(paths: string[]) {
    const existing = new Set(references.value.map(item => normalizePath(item.path)))
    for (const path of paths) {
      if (existing.has(normalizePath(path))) continue
      references.value.push({ id: crypto.randomUUID(), path, name: fileName(path) })
      existing.add(normalizePath(path))
    }
    if (!activeReferenceId.value) activeReferenceId.value = references.value[0]?.id || null
  }

  function removeReference(id: string) {
    references.value = references.value.filter(item => item.id !== id)
    if (activeReferenceId.value === id) activeReferenceId.value = references.value[0]?.id || null
  }

  function addTargetRoots(paths: string[]) {
    const existing = new Set(targetRoots.value.map(normalizePath))
    for (const path of paths) {
      if (!existing.has(normalizePath(path))) targetRoots.value.push(path)
      existing.add(normalizePath(path))
    }
  }

  async function search() {
    if (!canSearch.value) return
    const sessionId = crypto.randomUUID()
    currentSessionId.value = sessionId
    isRunning.value = true
    errors.value = []
    progress.value = null
    selectedPaths.value = []
    renamePreview.value = []
    let unlisten: (() => void) | null = null
    try {
      unlisten = await listenDifferenceSearchProgress(value => {
        if (value.sessionId === sessionId) progress.value = value
      })
      const result = await startDifferenceSearch({
        sessionId,
        references: references.value.map(({ id, path }) => ({ id, path })),
        targetRoots: targetRoots.value,
        recursive: recursive.value
      })
      matches.value = result.matches
      errors.value = result.errors
      selectedPaths.value = []
      orderedPaths.value = result.matches.map(item => item.filePath)
      renamePreview.value = []
    } finally {
      unlisten?.()
      isRunning.value = false
    }
  }

  async function cancel() {
    if (currentSessionId.value) await cancelDifferenceSearch(currentSessionId.value)
  }

  function toggleSelected(path: string, selected?: boolean) {
    const key = normalizePath(path)
    const exists = selectedPaths.value.some(item => normalizePath(item) === key)
    const shouldSelect = selected ?? !exists
    selectedPaths.value = shouldSelect
      ? exists ? selectedPaths.value : [...selectedPaths.value, path]
      : selectedPaths.value.filter(item => normalizePath(item) !== key)
  }

  function selectFiltered() {
    const merged = new Map(selectedPaths.value.map(path => [normalizePath(path), path]))
    filteredMatches.value.forEach(item => merged.set(normalizePath(item.filePath), item.filePath))
    selectedPaths.value = [...merged.values()]
  }

  function clearSelection() {
    selectedPaths.value = []
    renamePreview.value = []
  }

  function reorderSelected(from: number, to: number) {
    const selected = orderedSelectedMatches.value.map(item => item.filePath)
    const [moved] = selected.splice(from, 1)
    if (!moved) return
    selected.splice(to, 0, moved)
    const selectedKeys = new Set(selected.map(normalizePath))
    orderedPaths.value = [
      ...selected,
      ...orderedPaths.value.filter(path => !selectedKeys.has(normalizePath(path)))
    ]
  }

  async function generateRenamePreview(rule: RenameRule) {
    if (orderedSelectedMatches.value.length === 0) return
    isPreviewing.value = true
    try {
      renamePreview.value = await previewDifferenceRename(
        orderedSelectedMatches.value.map((item, index) => {
          const referenceId = activeReferenceId.value
            && item.relations.some(relation => relation.referenceId === activeReferenceId.value)
            ? activeReferenceId.value
            : item.bestReferenceId
          return {
            sourcePath: item.filePath,
            referenceName: referenceStem(referenceId),
            groupIndex: Math.max(1, references.value.findIndex(ref => ref.id === referenceId) + 1),
            order: index + 1,
            expectedFingerprint: fingerprintOf(item)
          }
        }),
        rule
      )
    } finally {
      isPreviewing.value = false
    }
  }

  function referenceStem(id: string) {
    const name = references.value.find(item => item.id === id)?.name || '参考图'
    return name.replace(/\.[^.]+$/, '')
  }

  function classificationForItem(item: DifferenceMatchItem): MatchClassification {
    if (!activeReferenceId.value) return item.classification
    return item.relations.find(relation => relation.referenceId === activeReferenceId.value)?.classification
      || item.classification
  }

  function applyOperationPaths(entries: OperationEntry[]) {
    const changed = new Map(
      entries
        .filter(entry => entry.status === 'succeeded')
        .map(entry => [normalizePath(entry.sourcePath), entry])
    )
    if (changed.size === 0) return
    matches.value = matches.value.map(item => {
      const entry = changed.get(normalizePath(item.filePath))
      return entry
        ? {
            ...item,
            filePath: entry.targetPath,
            fileName: fileName(entry.targetPath),
            blake3Hash: entry.targetFingerprint?.blake3Hash || item.blake3Hash,
            fileSize: entry.targetFingerprint?.fileSize ?? item.fileSize,
            modifiedAt: entry.targetFingerprint?.modifiedAt ?? item.modifiedAt
          }
        : item
    })
    selectedPaths.value = selectedPaths.value.map(path => changed.get(normalizePath(path))?.targetPath || path)
    orderedPaths.value = orderedPaths.value.map(path => changed.get(normalizePath(path))?.targetPath || path)
  }

  return {
    references,
    targetRoots,
    recursive,
    activeReferenceId,
    classificationFilter,
    matches,
    errors,
    selectedPaths,
    orderedPaths,
    progress,
    isRunning,
    renamePreview,
    isPreviewing,
    filteredMatches,
    orderedSelectedMatches,
    canSearch,
    hasBlockingRenameIssues,
    addReferences,
    removeReference,
    addTargetRoots,
    search,
    cancel,
    toggleSelected,
    selectFiltered,
    clearSelection,
    reorderSelected,
    generateRenamePreview,
    referenceStem,
    classificationForItem,
    applyOperationPaths
  }
})

function normalizePath(path: string) {
  return path.replace(/\//g, '\\').toLowerCase()
}

function fileName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() || path
}

function fingerprintOf(item: DifferenceMatchItem) {
  return {
    blake3Hash: item.blake3Hash,
    fileSize: item.fileSize,
    modifiedAt: item.modifiedAt
  }
}
