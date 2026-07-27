export function formatSsim(value: number) {
  return value.toFixed(6)
}

export function qualitySliderValueToThreshold(sliderValue: number) {
  const tick = Math.min(138, Math.max(0, Math.round(sliderValue)))
  if (tick <= 18) return (8000 + tick * 100) / 10000
  if (tick <= 38) return (9800 + (tick - 18) * 5) / 10000
  return (9900 + tick - 38) / 10000
}

export function qualityThresholdToSliderValue(threshold: number) {
  const basisPoints = Math.round(Math.min(1, Math.max(0.8, threshold)) * 10000)
  if (basisPoints <= 9800) return Math.round((basisPoints - 8000) / 100)
  if (basisPoints <= 9900) return 18 + Math.round((basisPoints - 9800) / 5)
  return 38 + basisPoints - 9900
}

export function precisionSliderValueToThreshold(sliderValue: number) {
  const tick = Math.min(140, Math.max(0, Math.round(sliderValue)))
  if (tick <= 40) return (9500 + tick * 10) / 10000
  return (9900 + tick - 40) / 10000
}

export function precisionThresholdToSliderValue(threshold: number) {
  const basisPoints = Math.round(Math.min(1, Math.max(0.95, threshold)) * 10000)
  if (basisPoints <= 9900) return Math.round((basisPoints - 9500) / 10)
  return 40 + basisPoints - 9900
}

export function settingsSliderValueToThreshold(sliderValue: number) {
  const tick = Math.min(190, Math.max(0, Math.round(sliderValue)))
  if (tick <= 90) return (9000 + tick * 10) / 10000
  return (9900 + tick - 90) / 10000
}

export function settingsThresholdToSliderValue(threshold: number) {
  const basisPoints = Math.round(Math.min(1, Math.max(0.9, threshold)) * 10000)
  if (basisPoints <= 9900) return Math.round((basisPoints - 9000) / 10)
  return 90 + basisPoints - 9900
}

export function parseStoredRecognitionThreshold(
  currentRaw: string | null,
  legacyPercentRaw: string | null
) {
  if (currentRaw !== null && currentRaw.trim() !== '') {
    const current = Number(currentRaw)
    if (Number.isFinite(current)) {
      return Math.min(1, Math.max(0.95, Math.round(current * 10000) / 10000))
    }
  }
  if (legacyPercentRaw !== null && legacyPercentRaw.trim() !== '') {
    const legacyPercent = Number(legacyPercentRaw)
    if (Number.isFinite(legacyPercent)) {
      return Math.min(1, Math.max(0.95, Math.round(legacyPercent * 100) / 10000))
    }
  }
  return 0.985
}
