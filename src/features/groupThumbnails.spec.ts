import { describe, expect, it } from 'vitest'
import type { ComparisonGroup, ComparisonGroupMember } from '@/types'
import {
  getAutomaticOriginalImageIds,
  groupHasThumbnailCandidates
} from './groupThumbnails'

function member(id: number, overrides: Partial<ComparisonGroupMember> = {}): ComparisonGroupMember {
  return {
    image_id: id,
    file_path: `D:/images/${id}.png`,
    relative_path: `${id}.png`,
    file_size: id === 1 ? 4_000_000 : 120_000,
    width: id === 1 ? 1500 : 800,
    height: id === 1 ? 2400 : 1280,
    role: id === 1 ? 'reference' : 'lower_quality',
    role_label: id === 1 ? '组内参考图' : '疑似低质量',
    reference_image_id: id === 1 ? null : 1,
    reference_relative_path: id === 1 ? null : '1.png',
    ssim_score: id === 1 ? 1 : 0.97,
    ssim_cluster_key: '1',
    is_low_quality_suggestion: id !== 1,
    ...overrides
  }
}

function group(members: ComparisonGroupMember[]): ComparisonGroup {
  return {
    group_index: 7,
    representative_image_id: 1,
    representative_file_name: '1.png',
    member_count: members.length,
    has_low_quality_suggestion: false,
    members
  }
}

describe('group thumbnail classification', () => {
  it('reports a lower-resolution non-original as a thumbnail candidate', () => {
    const currentGroup = group([member(1), member(2)])

    expect([...getAutomaticOriginalImageIds(currentGroup, 0.985)]).toEqual([1])
    expect(groupHasThumbnailCandidates(currentGroup, 0.985)).toBe(true)
  })

  it('reports no thumbnails when every member satisfies the automatic-original rule', () => {
    const currentGroup = group([
      member(1),
      member(3, {
        file_size: 3_000_000,
        width: 1450,
        height: 2320,
        role: 'similar_keep',
        role_label: '相似保留',
        ssim_score: 0.99,
        is_low_quality_suggestion: false
      })
    ])

    expect([...getAutomaticOriginalImageIds(currentGroup, 0.985)]).toEqual([1, 3])
    expect(groupHasThumbnailCandidates(currentGroup, 0.985)).toBe(false)
  })
})
