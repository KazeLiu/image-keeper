import { computed, ref } from 'vue'
import type {
  TestImageInfo,
  TestLowPrecisionResult,
  TestStandardSsimResult
} from '@/api/imageMetrics'

export type MetricState<T> =
  | { status: 'idle' | 'queued' | 'loading'; value?: undefined; error?: undefined }
  | { status: 'done'; value: T; error?: undefined }
  | { status: 'error'; value?: undefined; error: string }
  | { status: 'baseline'; value?: undefined; error?: undefined }

export interface TestImageItem extends TestImageInfo {
  phashDistance: number | null
  low: MetricState<TestLowPrecisionResult>
  high: MetricState<TestStandardSsimResult>
}

export interface ImageMetricsDependencies {
  loadImage(path: string): Promise<TestImageInfo>
  computeLow(
    baseline: TestImageInfo,
    candidate: TestImageInfo
  ): Promise<TestLowPrecisionResult>
  computeHigh(
    baseline: TestImageInfo,
    candidate: TestImageInfo
  ): Promise<TestStandardSsimResult>
}

function idle<T>(): MetricState<T> {
  return { status: 'idle' }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

function highPrecisionCacheKey(left: TestImageInfo, right: TestImageInfo) {
  return [left, right]
    .map((item) => `${item.path}|${item.fileSize}|${item.modifiedAtMs}`)
    .sort()
    .join('::')
}

function phashDistance(left: string, right: string) {
  if (!/^[0-9a-f]{16}$/i.test(left) || !/^[0-9a-f]{16}$/i.test(right)) return null
  let difference = BigInt(`0x${left}`) ^ BigInt(`0x${right}`)
  let distance = 0
  while (difference > 0n) {
    distance += Number(difference & 1n)
    difference >>= 1n
  }
  return distance
}

export function createImageMetricsSession(deps: ImageMetricsDependencies) {
  const items = ref<TestImageItem[]>([])
  const baselinePath = ref<string | null>(null)
  const loadingCount = ref(0)
  const lowPrecisionCount = ref(0)
  const highPrecisionCount = ref(0)
  const importErrors = ref<string[]>([])
  const duplicateCount = ref(0)
  const highCache = new Map<string, TestStandardSsimResult>()
  let lifecycleGeneration = 0
  let comparisonGeneration = 0
  let lowQueue = Promise.resolve()

  const highPrecisionBusy = computed(() => highPrecisionCount.value > 0)
  const hasRunningTasks = computed(() =>
    loadingCount.value > 0 || lowPrecisionCount.value > 0 || highPrecisionCount.value > 0
  )
  const hasContent = computed(() =>
    items.value.length > 0
    || loadingCount.value > 0
    || lowPrecisionCount.value > 0
    || highPrecisionCount.value > 0
  )

  async function addPaths(paths: string[]) {
    const lifecycle = lifecycleGeneration
    for (const path of paths) {
      loadingCount.value += 1
      try {
        const loaded = await deps.loadImage(path)
        if (lifecycle !== lifecycleGeneration) return
        const duplicate = items.value.some(
          (item) => item.path.toLocaleLowerCase() === loaded.path.toLocaleLowerCase()
        )
        if (!duplicate) {
          items.value.push({
            ...loaded,
            phashDistance: null,
            low: idle<TestLowPrecisionResult>(),
            high: idle<TestStandardSsimResult>()
          })
        } else {
          duplicateCount.value += 1
        }
      } catch (error) {
        if (lifecycle === lifecycleGeneration) {
          importErrors.value.push(`${path}：${errorMessage(error)}`)
        }
      } finally {
        loadingCount.value = Math.max(0, loadingCount.value - 1)
      }
    }
  }

  async function computeLowPrecision(
    baseline: TestImageItem,
    candidate: TestImageItem,
    comparison: number
  ) {
    candidate.low = { status: 'queued' }
    lowPrecisionCount.value += 1
    const run = lowQueue.then(async () => {
      if (
        comparison !== comparisonGeneration
        || baselinePath.value !== baseline.path
        || !items.value.some((item) => item.path === baseline.path)
        || !items.value.some((item) => item.path === candidate.path)
      ) {
        return
      }

      candidate.low = { status: 'loading' }
      try {
        const value = await deps.computeLow(baseline, candidate)
        if (comparison === comparisonGeneration && baselinePath.value === baseline.path) {
          candidate.low = { status: 'done', value }
        }
      } catch (error) {
        if (comparison === comparisonGeneration && baselinePath.value === baseline.path) {
          candidate.low = { status: 'error', error: errorMessage(error) }
        }
      }
    })
    lowQueue = run.catch(() => undefined)
    try {
      await run
    } finally {
      lowPrecisionCount.value = Math.max(0, lowPrecisionCount.value - 1)
    }
  }

  async function setBaseline(path: string) {
    const baseline = items.value.find((item) => item.path === path)
    if (!baseline) return

    if (baselinePath.value === path) {
      const newCandidates = items.value.filter(
        (item) => item.path !== path && item.low.status === 'idle'
      )
      for (const item of newCandidates) {
        item.phashDistance = phashDistance(baseline.phash, item.phash)
        item.low = { status: 'queued' }
      }
      for (const item of newCandidates) {
        await computeLowPrecision(baseline, item, comparisonGeneration)
      }
      return
    }

    comparisonGeneration += 1
    const comparison = comparisonGeneration
    baselinePath.value = path
    for (const item of items.value) {
      item.phashDistance = item.path === path ? null : phashDistance(baseline.phash, item.phash)
      item.low = item.path === path ? { status: 'baseline' } : { status: 'queued' }
      item.high = item.path === path
        ? { status: 'baseline' }
        : idle<TestStandardSsimResult>()
    }

    for (const item of items.value) {
      if (item.path === path || comparison !== comparisonGeneration) continue
      await computeLowPrecision(baseline, item, comparison)
    }
  }

  async function retryLowPrecision(path: string) {
    const baseline = items.value.find((item) => item.path === baselinePath.value)
    const candidate = items.value.find((item) => item.path === path)
    if (
      !baseline
      || !candidate
      || baseline.path === candidate.path
      || candidate.low.status === 'queued'
      || candidate.low.status === 'loading'
    ) {
      return false
    }
    await computeLowPrecision(baseline, candidate, comparisonGeneration)
    return true
  }

  async function computeHighPrecision(path: string) {
    const baseline = items.value.find((item) => item.path === baselinePath.value)
    const candidate = items.value.find((item) => item.path === path)
    if (!baseline || !candidate || baseline.path === candidate.path || highPrecisionBusy.value) {
      return false
    }

    const cacheKey = highPrecisionCacheKey(baseline, candidate)
    const cached = highCache.get(cacheKey)
    if (cached) {
      candidate.high = { status: 'done', value: cached }
      return true
    }

    const comparison = comparisonGeneration
    highPrecisionCount.value += 1
    candidate.high = { status: 'loading' }
    try {
      const value = await deps.computeHigh(baseline, candidate)
      if (comparison === comparisonGeneration && baselinePath.value === baseline.path) {
        highCache.set(cacheKey, value)
        candidate.high = { status: 'done', value }
      }
    } catch (error) {
      if (comparison === comparisonGeneration && baselinePath.value === baseline.path) {
        candidate.high = { status: 'error', error: errorMessage(error) }
      }
    } finally {
      highPrecisionCount.value = Math.max(0, highPrecisionCount.value - 1)
    }
    return true
  }

  function remove(path: string) {
    const removingBaseline = baselinePath.value === path
    items.value = items.value.filter((item) => item.path !== path)
    if (removingBaseline) {
      comparisonGeneration += 1
      baselinePath.value = null
      for (const item of items.value) {
        item.phashDistance = null
        item.low = idle<TestLowPrecisionResult>()
        item.high = idle<TestStandardSsimResult>()
      }
    }
  }

  function clearImportErrors() {
    importErrors.value = []
  }

  function clearDuplicateCount() {
    duplicateCount.value = 0
  }

  function reset() {
    lifecycleGeneration += 1
    comparisonGeneration += 1
    items.value = []
    baselinePath.value = null
    importErrors.value = []
    duplicateCount.value = 0
    highCache.clear()
  }

  return {
    items,
    baselinePath,
    loadingCount,
    highPrecisionBusy,
    importErrors,
    duplicateCount,
    hasRunningTasks,
    hasContent,
    addPaths,
    setBaseline,
    retryLowPrecision,
    computeHighPrecision,
    remove,
    clearImportErrors,
    clearDuplicateCount,
    reset
  }
}
