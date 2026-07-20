import { ref } from 'vue'
import type { TestDifferencePreviewResult, TestImageInfo } from '@/api/imageMetrics'

type DifferencePreviewLoader = (
  baseline: TestImageInfo,
  candidate: TestImageInfo,
  sensitivity: number
) => Promise<TestDifferencePreviewResult>

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}

export function createDifferencePreview(loadPreview: DifferencePreviewLoader) {
  const visible = ref(false)
  const loading = ref(false)
  const error = ref('')
  const result = ref<TestDifferencePreviewResult | null>(null)
  const baseline = ref<TestImageInfo | null>(null)
  const candidate = ref<TestImageInfo | null>(null)
  const sensitivity = ref(50)
  let generation = 0

  async function open(nextBaseline: TestImageInfo, nextCandidate: TestImageInfo) {
    baseline.value = nextBaseline
    candidate.value = nextCandidate
    sensitivity.value = 50
    result.value = null
    error.value = ''
    visible.value = true
    await refresh()
  }

  async function refresh() {
    const currentBaseline = baseline.value
    const currentCandidate = candidate.value
    if (!currentBaseline || !currentCandidate) return

    const currentGeneration = ++generation
    loading.value = true
    error.value = ''
    try {
      const nextResult = await loadPreview(
        currentBaseline,
        currentCandidate,
        sensitivity.value
      )
      if (currentGeneration === generation) {
        result.value = nextResult
      }
    } catch (cause) {
      if (currentGeneration === generation) {
        error.value = errorMessage(cause)
      }
    } finally {
      if (currentGeneration === generation) {
        loading.value = false
      }
    }
  }

  function close() {
    generation += 1
    visible.value = false
    loading.value = false
    error.value = ''
    result.value = null
    baseline.value = null
    candidate.value = null
  }

  return {
    visible,
    loading,
    error,
    result,
    baseline,
    candidate,
    sensitivity,
    open,
    refresh,
    retry: refresh,
    close
  }
}
