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

export function createImageMetricsSession(deps: ImageMetricsDependencies) {
  const items = ref<TestImageItem[]>([])
  const baselinePath = ref<string | null>(null)
  const loadingCount = ref(0)
  const highPrecisionBusy = ref(false)
  const importErrors = ref<string[]>([])
  const highCache = new Map<string, TestStandardSsimResult>()
  let lifecycleGeneration = 0
  let comparisonGeneration = 0

  const hasContent = computed(() => items.value.length > 0 || loadingCount.value > 0)

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
            low: idle<TestLowPrecisionResult>(),
            high: idle<TestStandardSsimResult>()
          })
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

  async function setBaseline(path: string) {
    const baseline = items.value.find((item) => item.path === path)
    if (!baseline) return

    comparisonGeneration += 1
    const comparison = comparisonGeneration
    baselinePath.value = path
    for (const item of items.value) {
      item.low = item.path === path ? { status: 'baseline' } : { status: 'queued' }
      item.high = item.path === path
        ? { status: 'baseline' }
        : idle<TestStandardSsimResult>()
    }

    for (const item of items.value) {
      if (item.path === path || comparison !== comparisonGeneration) continue
      item.low = { status: 'loading' }
      try {
        const value = await deps.computeLow(baseline, item)
        if (comparison === comparisonGeneration && baselinePath.value === path) {
          item.low = { status: 'done', value }
        }
      } catch (error) {
        if (comparison === comparisonGeneration && baselinePath.value === path) {
          item.low = { status: 'error', error: errorMessage(error) }
        }
      }
    }
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
    highPrecisionBusy.value = true
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
      highPrecisionBusy.value = false
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
        item.low = idle<TestLowPrecisionResult>()
        item.high = idle<TestStandardSsimResult>()
      }
    }
  }

  function clearImportErrors() {
    importErrors.value = []
  }

  function reset() {
    lifecycleGeneration += 1
    comparisonGeneration += 1
    items.value = []
    baselinePath.value = null
    loadingCount.value = 0
    highPrecisionBusy.value = false
    importErrors.value = []
    highCache.clear()
  }

  return {
    items,
    baselinePath,
    loadingCount,
    highPrecisionBusy,
    importErrors,
    hasContent,
    addPaths,
    setBaseline,
    computeHighPrecision,
    remove,
    clearImportErrors,
    reset
  }
}
