import type { ComparisonGroup, ComparisonGroupMember } from '@/types'

/** 返回自动判定为原图的图片 ID，供详情与分组列表共用。 */
export function getAutomaticOriginalImageIds(
  group: ComparisonGroup,
  recognitionThreshold: number
): Set<number> {
  if (group.members.length === 0) return new Set()

  const maxPixels = Math.max(...group.members.map(imagePixels))
  const referenceAspect = imageAspect(getHighestQualityMember(group.members))
  return new Set(
    group.members
      .filter((member) => {
        if (member.image_id === group.representative_image_id || member.role === 'reference') {
          return true
        }

        const highResolution = imagePixels(member) >= maxPixels * 0.9
        const sameShape = aspectDifference(member, referenceAspect) <= 0.02
        const similarEnough = (
          typeof member.ssim_score !== 'number'
          || member.ssim_score >= recognitionThreshold
        )
        return highResolution && sameShape && similarEnough
      })
      .map((member) => member.image_id)
  )
}

/** 判断一个分组在自动归属下是否至少包含一张缩略图。 */
export function groupHasThumbnailCandidates(
  group: ComparisonGroup,
  recognitionThreshold: number
): boolean {
  if (group.members.length < 2) return false
  const automaticOriginalIds = getAutomaticOriginalImageIds(group, recognitionThreshold)
  const originalCount = automaticOriginalIds.size || 1
  return originalCount < group.members.length
}

/** 选出与详情表默认原图一致的最高质量图片。 */
function getHighestQualityMember(members: ComparisonGroupMember[]): ComparisonGroupMember {
  return [...members].sort((left, right) => (
    imagePixels(right) - imagePixels(left)
    || right.file_size - left.file_size
    || left.relative_path.localeCompare(right.relative_path)
  ))[0]
}

/** 计算图片总像素数。 */
function imagePixels(member: ComparisonGroupMember): number {
  return member.width * member.height
}

/** 计算图片宽高比，异常高度按零处理。 */
function imageAspect(member: ComparisonGroupMember): number {
  return member.height === 0 ? 0 : member.width / member.height
}

/** 计算图片相对参考图的宽高比差异。 */
function aspectDifference(member: ComparisonGroupMember, referenceAspect: number): number {
  if (referenceAspect === 0) return 0
  return Math.abs(imageAspect(member) - referenceAspect) / referenceAspect
}
