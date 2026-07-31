import { createPinia, setActivePinia } from 'pinia'
import { flushPromises } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { RunStatus } from '@/types'
import { useComparisonStore } from './comparisonStore'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((done, fail) => {
    resolve = done
    reject = fail
  })
  return { promise, resolve, reject }
}

function comparisonStats(runId = 'run-1') {
  return {
    run_id: runId,
    baseline_total: 1,
    comparison_total: 1,
    exact_duplicate: 0,
    likely_compressed: 0,
    variant: 0,
    similar_keep: 0,
    no_baseline_match: 1,
    inconclusive: 0,
    not_evaluated: 0,
    error: 0,
    pending_review: 0,
    approved_for_recycle: 0,
    rejected_keep: 0,
    recycled: 0,
    restored: 0,
    permanently_deleted: 0
  }
}

const apiMocks = vi.hoisted(() => ({
  getComparisonStats: vi.fn(),
  getComparisonResults: vi.fn(),
  getComparisonGroups: vi.fn(),
  getGroupSimilarityStatuses: vi.fn(),
  getRunStatus: vi.fn(),
  listComparisonRuns: vi.fn(),
  startMultiCompare: vi.fn(),
  deleteComparisonRun: vi.fn()
}))

vi.mock('@/api/comparison', () => apiMocks)

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => vi.fn())
}))

describe('comparisonStore history loading', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    window.localStorage.clear()

    apiMocks.getRunStatus.mockResolvedValue({
      run_id: 'run-1',
      status: RunStatus.ReviewPending,
      completed_at: 1
    })
    apiMocks.getComparisonStats.mockResolvedValue(comparisonStats())
    apiMocks.getComparisonResults.mockResolvedValue([])
    apiMocks.getComparisonGroups.mockResolvedValue([])
    apiMocks.getGroupSimilarityStatuses.mockReturnValue(new Promise(() => undefined))
  })

  it('finishes loading the history body without waiting for SSIM status hydration', async () => {
    const store = useComparisonStore()
    const loading = store.loadHistoryRun('run-1')

    await flushPromises()

    expect(store.currentRunId).toBe('run-1')
    expect(store.stats?.run_id).toBe('run-1')
    expect(store.isLoadingHistory).toBe(false)
    await loading
  })

  it('does not let an older failed history request clear the newer run', async () => {
    const oldStatus = deferred<{ run_id: string; status: RunStatus; completed_at: number }>()
    apiMocks.getRunStatus.mockImplementation((runId: string) => {
      if (runId === 'run-a') return oldStatus.promise
      return Promise.resolve({
        run_id: 'run-b',
        status: RunStatus.ReviewPending,
        completed_at: 2
      })
    })
    apiMocks.getComparisonStats.mockResolvedValue(comparisonStats('run-b'))

    const store = useComparisonStore()
    const oldLoading = store.loadHistoryRun('run-a')
    const currentLoading = store.loadHistoryRun('run-b')
    await currentLoading
    oldStatus.reject(new Error('旧任务读取失败'))

    await expect(oldLoading).resolves.toBeUndefined()
    expect(store.currentRunId).toBe('run-b')
    expect(store.stats?.run_id).toBe('run-b')
    expect(store.isLoadingHistory).toBe(false)
  })

  it('keeps the group loading flag until the newest refresh finishes', async () => {
    const firstGroups = deferred<any[]>()
    const secondGroups = deferred<any[]>()
    apiMocks.getComparisonGroups
      .mockReturnValueOnce(firstGroups.promise)
      .mockReturnValueOnce(secondGroups.promise)
    const store = useComparisonStore()
    store.currentRunId = 'run-1'

    const firstRefresh = store.refreshGroups()
    const secondRefresh = store.refreshGroups()
    firstGroups.resolve([{ group_index: 1, members: [] }])
    await firstRefresh

    expect(store.isRefreshingGroups).toBe(true)
    expect(store.groups).toEqual([])

    secondGroups.resolve([{ group_index: 2, members: [] }])
    await secondRefresh
    expect(store.isRefreshingGroups).toBe(false)
    expect(store.groups[0]?.group_index).toBe(2)
  })
})
