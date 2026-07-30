import { describe, expect, it } from 'vitest'
import {
  formatSsim,
  parseStoredRecognitionThreshold,
  precisionSliderValueToThreshold,
  precisionThresholdToSliderValue,
  settingsSliderValueToThreshold,
  settingsThresholdToSliderValue
} from './similarity'

describe('similarity presentation', () => {
  it('shows the direct ssim value instead of a percentage', () => {
    expect(formatSsim(0.9963142)).toBe('0.996314')
  })

  it('keeps 0.001 recognition steps below 0.99 and uses 0.0001 above it', () => {
    expect(precisionSliderValueToThreshold(0)).toBe(0.95)
    expect(precisionSliderValueToThreshold(1)).toBe(0.951)
    expect(precisionSliderValueToThreshold(39)).toBe(0.989)
    expect(precisionSliderValueToThreshold(40)).toBe(0.99)
    expect(precisionSliderValueToThreshold(41)).toBe(0.9901)
    expect(precisionSliderValueToThreshold(139)).toBe(0.9999)
    expect(precisionSliderValueToThreshold(140)).toBe(1)
    expect(precisionThresholdToSliderValue(0.989)).toBe(39)
    expect(precisionThresholdToSliderValue(0.9901)).toBe(41)
    expect(precisionThresholdToSliderValue(0.9999)).toBe(139)
  })

  it('keeps the settings range at 0.9–1 with finer steps only above 0.99', () => {
    expect(settingsSliderValueToThreshold(0)).toBe(0.9)
    expect(settingsSliderValueToThreshold(1)).toBe(0.901)
    expect(settingsSliderValueToThreshold(89)).toBe(0.989)
    expect(settingsSliderValueToThreshold(90)).toBe(0.99)
    expect(settingsSliderValueToThreshold(91)).toBe(0.9901)
    expect(settingsSliderValueToThreshold(189)).toBe(0.9999)
    expect(settingsSliderValueToThreshold(190)).toBe(1)
    expect(settingsThresholdToSliderValue(0.989)).toBe(89)
    expect(settingsThresholdToSliderValue(0.9901)).toBe(91)
    expect(settingsThresholdToSliderValue(0.9999)).toBe(189)
  })

  it('migrates recognition thresholds without treating a missing key as zero', () => {
    expect(parseStoredRecognitionThreshold(null, null)).toBe(0.985)
    expect(parseStoredRecognitionThreshold('0.9973', '98.5')).toBe(0.9973)
    expect(parseStoredRecognitionThreshold(null, '98.5')).toBe(0.985)
  })
})
