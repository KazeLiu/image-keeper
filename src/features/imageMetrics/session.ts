import { computed, ref } from 'vue'
import type {
  TestImageInfo,
  TestImagePhashResult,
  TestSsimResult
} from '@/api/imageMetrics'

export type MetricState<T> =
  | { status: 'idle' | 'queued' | 'loading'; value?: undefined; error?: undefined }
  | { status: 'done'; value: T; error?: undefined }
  | { status: 'error'; value?: undefined; error: string }
  | { status: 'baseline'; value?: undefined; error?: undefined }

export interface TestImageItem extends TestImageInfo {
  importOrder: number
  loadState: 'loading' | 'ready'
  phash: string
  phashState: 'idle' | 'loading' | 'ready' | 'error'
  phashDistance: number | null
  ssim: MetricState<TestSsimResult>
}

export interface ImageMetricsDependencies {
  loadImage(path: string): Promise<TestImageInfo>
  computePhash(image: TestImageInfo): Promise<TestImagePhashResult>
  computeSsim(baseline: TestImageInfo, candidate: TestImageInfo): Promise<TestSsimResult>
}

const ALGORITHM_CONCURRENCY = 4

function createLimiter(concurrency: number) {
  let active = 0
  const waiters: Array<() => void> = []

  async function acquire() {
    if (active < concurrency) {
      active += 1
      return
    }
    await new Promise<void>((resolve) => waiters.push(resolve))
  }

  function release() {
    const next = waiters.shift()
    if (next) next()
    else active = Math.max(0, active - 1)
  }

  return { acquire, release }
}

const importLimiter = createLimiter(ALGORITHM_CONCURRENCY)
const phashLimiter = createLimiter(ALGORITHM_CONCURRENCY)
const ssimLimiter = createLimiter(ALGORITHM_CONCURRENCY)

function idle<T>(): MetricState<T> {
  return { status: 'idle' }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
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

function displayNameFromPath(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() || path || '图片加载中'
}

function loadingItem(path: string, importOrder: number): TestImageItem {
  return {
    path,
    fileName: displayNameFromPath(path),
    fileSize: 0,
    width: 0,
    height: 0,
    modifiedAtMs: 0,
    thumbnailDataUrl: '',
    importOrder,
    loadState: 'loading',
    phash: '',
    phashState: 'idle',
    phashDistance: null,
    ssim: idle<TestSsimResult>()
  }
}

export function createImageMetricsSession(deps: ImageMetricsDependencies) {
  const items = ref<TestImageItem[]>([])
  const baselinePath = ref<string | null>(null)
  const loadingCount = ref(0)
  const phashCount = ref(0)
  const ssimCount = ref(0)
  const importErrors = ref<string[]>([])
  const duplicateCount = ref(0)
  let lifecycleGeneration = 0
  let comparisonGeneration = 0
  let nextImportOrder = 0

  const hasRunningTasks = computed(() =>
    loadingCount.value > 0 || phashCount.value > 0 || ssimCount.value > 0
  )
  const hasContent = computed(() => items.value.length > 0 || hasRunningTasks.value)

  async function addPaths(paths: string[]) {
    const lifecycle = lifecycleGeneration
    const importBaseOrder = nextImportOrder
    nextImportOrder += paths.length
    const placeholders = paths.map((path, index) => loadingItem(path, importBaseOrder + index))
    loadingCount.value += placeholders.length
    items.value.push(...placeholders)
    items.value.sort((left, right) => left.importOrder - right.importOrder)

    await Promise.all(placeholders.map(async (placeholder) => {
      await importLimiter.acquire()
      try {
        if (lifecycle !== lifecycleGeneration) return
        const loaded = await deps.loadImage(placeholder.path)
        if (lifecycle !== lifecycleGeneration) return
        const duplicate = items.value.some((item) =>
          item.importOrder !== placeholder.importOrder
          && item.loadState === 'ready'
          && item.path.toLocaleLowerCase() === loaded.path.toLocaleLowerCase()
        )
        if (duplicate) {
          items.value = items.value.filter((item) => item.importOrder !== placeholder.importOrder)
          duplicateCount.value += 1
          return
        }

        const index = items.value.findIndex((item) => item.importOrder === placeholder.importOrder)
        if (index < 0) return
        const readyItem: TestImageItem = {
          ...loaded,
          importOrder: placeholder.importOrder,
          loadState: 'ready',
          phash: '',
          phashState: 'loading',
          phashDistance: null,
          ssim: idle<TestSsimResult>()
        }
        items.value[index] = readyItem
        items.value.sort((left, right) => left.importOrder - right.importOrder)
        void computeImagePhash(readyItem, lifecycle)
      } catch (error) {
        if (lifecycle === lifecycleGeneration) {
          items.value = items.value.filter((item) => item.importOrder !== placeholder.importOrder)
          importErrors.value.push(`${placeholder.path}：${errorMessage(error)}`)
        }
      } finally {
        importLimiter.release()
        loadingCount.value = Math.max(0, loadingCount.value - 1)
      }
    }))
  }

  async function computeImagePhash(item: TestImageItem, lifecycle: number) {
    phashCount.value += 1
    await phashLimiter.acquire()
    try {
      if (lifecycle !== lifecycleGeneration) return
      const queuedItem = items.value.find((candidate) =>
        candidate.importOrder === item.importOrder && candidate.loadState === 'ready'
      )
      if (!queuedItem) return
      const result = await deps.computePhash(item)
      if (lifecycle !== lifecycleGeneration) return
      const current = items.value.find((candidate) => candidate.importOrder === item.importOrder)
      if (!current || current.loadState !== 'ready') return
      current.phash = result.phash
      current.phashState = 'ready'
      await refreshComparisonFor(current)
    } catch (error) {
      if (lifecycle !== lifecycleGeneration) return
      const current = items.value.find((candidate) => candidate.importOrder === item.importOrder)
      if (!current) return
      current.phashState = 'error'
      importErrors.value.push(`${current.path}：${errorMessage(error)}`)
    } finally {
      phashLimiter.release()
      phashCount.value = Math.max(0, phashCount.value - 1)
    }
  }

  async function refreshComparisonFor(item: TestImageItem) {
    const baseline = items.value.find((candidate) =>
      candidate.path === baselinePath.value
      && candidate.loadState === 'ready'
      && candidate.phashState === 'ready'
    )
    if (!baseline) return

    if (item.path === baseline.path) {
      for (const candidate of items.value) {
        if (
          candidate.path !== baseline.path
          && candidate.loadState === 'ready'
          && candidate.phashState === 'ready'
          && candidate.ssim.status === 'idle'
        ) {
          candidate.phashDistance = phashDistance(baseline.phash, candidate.phash)
          void computeSsim(baseline, candidate, comparisonGeneration)
        }
      }
      return
    }

    if (item.phashState !== 'ready') return
    item.phashDistance = phashDistance(baseline.phash, item.phash)
    if (item.ssim.status === 'idle') {
      void computeSsim(baseline, item, comparisonGeneration)
    }
  }

  async function computeSsim(
    baseline: TestImageItem,
    candidate: TestImageItem,
    comparison: number
  ) {
    candidate.ssim = { status: 'queued' }
    ssimCount.value += 1
    await ssimLimiter.acquire()
    try {
      if (
        comparison !== comparisonGeneration
        || baselinePath.value !== baseline.path
        || !items.value.some((item) => item.path === baseline.path)
        || !items.value.some((item) => item.path === candidate.path)
      ) return

      candidate.ssim = { status: 'loading' }
      try {
        const value = await deps.computeSsim(baseline, candidate)
        if (comparison === comparisonGeneration && baselinePath.value === baseline.path) {
          candidate.ssim = { status: 'done', value }
        }
      } catch (error) {
        if (comparison === comparisonGeneration && baselinePath.value === baseline.path) {
          candidate.ssim = { status: 'error', error: errorMessage(error) }
        }
      }
    } finally {
      ssimLimiter.release()
      ssimCount.value = Math.max(0, ssimCount.value - 1)
    }
  }

  async function setBaseline(path: string) {
    const baseline = items.value.find((item) => item.path === path && item.loadState === 'ready')
    if (!baseline) return

    if (baselinePath.value !== path) {
      comparisonGeneration += 1
      baselinePath.value = path
      for (const item of items.value) {
        if (item.loadState !== 'ready') continue
        item.phashDistance = item.path === path || item.phashState !== 'ready'
          ? null
          : phashDistance(baseline.phash, item.phash)
        item.ssim = item.path === path
          ? { status: 'baseline' }
          : idle<TestSsimResult>()
      }
    }

    const comparison = comparisonGeneration
    for (const item of items.value) {
      if (
        item.loadState === 'ready'
        && item.phashState === 'ready'
        && baseline.phashState === 'ready'
        && item.path !== path
        && item.ssim.status === 'idle'
      ) {
        void computeSsim(baseline, item, comparison)
      }
    }
  }

  async function retrySsim(path: string) {
    const baseline = items.value.find((item) =>
      item.path === baselinePath.value && item.loadState === 'ready'
    )
    const candidate = items.value.find((item) => item.path === path && item.loadState === 'ready')
    if (
      !baseline || !candidate || baseline.path === candidate.path
      || candidate.ssim.status === 'queued' || candidate.ssim.status === 'loading'
    ) return false
    await computeSsim(baseline, candidate, comparisonGeneration)
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
        item.ssim = idle<TestSsimResult>()
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
  }

  return {
    items,
    baselinePath,
    loadingCount,
    importErrors,
    duplicateCount,
    hasRunningTasks,
    hasContent,
    addPaths,
    setBaseline,
    retrySsim,
    remove,
    clearImportErrors,
    clearDuplicateCount,
    reset
  }
}
