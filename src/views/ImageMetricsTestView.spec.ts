import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus, { ElMessage, ElMessageBox } from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ImageMetricsTestView from './ImageMetricsTestView.vue'

const apiMocks = vi.hoisted(() => ({
  load: vi.fn(),
  computePhash: vi.fn(),
  computeSsim: vi.fn(async () => ({ score: 0.95, durationMs: 10 })),
  computeDifference: vi.fn()
}))

const windowMocks = vi.hoisted(() => ({
  close: vi.fn(async () => undefined),
  closeHandler: undefined as undefined | ((
    event: { preventDefault: () => void }
  ) => void | Promise<void>)
}))

vi.mock('@/api/imageMetrics', () => ({
  loadTestImage: apiMocks.load,
  computeTestPhash: apiMocks.computePhash,
  computeTestSsim: apiMocks.computeSsim,
  computeTestDifferencePreview: apiMocks.computeDifference
}))

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: vi.fn()
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null)
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    close: windowMocks.close,
    onCloseRequested: vi.fn(async (handler) => {
      windowMocks.closeHandler = handler
      return () => undefined
    }),
    onDragDropEvent: vi.fn(async () => () => undefined)
  })
}))

function mountView() {
  return mount(ImageMetricsTestView, {
    attachTo: document.body,
    global: { plugins: [ElementPlus] }
  })
}

async function addPaths(wrapper: ReturnType<typeof mountView>, paths: string[]) {
  await (wrapper.vm as unknown as { addImagePathsForTest: (paths: string[]) => Promise<void> })
    .addImagePathsForTest(paths)
  await flushPromises()
}

describe('ImageMetricsTestView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    windowMocks.closeHandler = undefined
    apiMocks.load.mockImplementation(async (path: string) => ({
      path,
      fileName: `${path}.png`,
      fileSize: 100,
      width: 100,
      height: 100,
      modifiedAtMs: 1,
      thumbnailDataUrl: `data:image/png;base64,${path}`
    }))
    apiMocks.computePhash.mockImplementation(async ({ path }: { path: string }) => ({
      phash: path === 'candidate' ? '0000000000000001' : '0000000000000000'
    }))
    apiMocks.computeSsim.mockResolvedValue({ score: 0.95, durationMs: 10 })
    apiMocks.computeDifference.mockResolvedValue({
      baselineDataUrl: 'data:image/png;base64,baseline',
      candidateDataUrl: 'data:image/png;base64,candidate',
      highlightDataUrl: 'data:image/png;base64,highlight',
      width: 100,
      height: 100,
      changedPixelRatio: 0.12,
      regionCount: 2
    })
  })

  it('closes an empty window without confirmation', async () => {
    const wrapper = mountView()
    await flushPromises()
    const preventDefault = vi.fn()

    await windowMocks.closeHandler?.({ preventDefault })
    await flushPromises()

    expect(preventDefault).not.toHaveBeenCalled()
    expect(windowMocks.close).not.toHaveBeenCalled()
  })

  it('keeps a non-empty window when discard confirmation is canceled', async () => {
    vi.spyOn(ElMessageBox, 'confirm').mockRejectedValue('cancel')
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])
    const preventDefault = vi.fn()
    await windowMocks.closeHandler?.({ preventDefault })
    await flushPromises()

    expect(preventDefault).toHaveBeenCalledTimes(1)
    expect(windowMocks.close).not.toHaveBeenCalled()
  })

  it('keeps the session when the native close call fails', async () => {
    vi.spyOn(ElMessageBox, 'confirm').mockResolvedValue('confirm')
    windowMocks.close.mockRejectedValueOnce(new Error('关闭失败'))
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])
    const preventDefault = vi.fn()
    await windowMocks.closeHandler?.({ preventDefault })
    await flushPromises()

    expect(wrapper.find('[data-test="card-0"]').exists()).toBe(true)
  })

  it('intercepts the native close request for a non-empty session', async () => {
    vi.spyOn(ElMessageBox, 'confirm').mockRejectedValue('cancel')
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])
    await flushPromises()
    const preventDefault = vi.fn()

    await windowMocks.closeHandler?.({ preventDefault })

    expect(preventDefault).toHaveBeenCalledTimes(1)
    expect(windowMocks.close).not.toHaveBeenCalled()
  })

  it('does not await discard confirmation inside the native close handler', async () => {
    let resolveConfirm!: (value: string) => void
    vi.spyOn(ElMessageBox, 'confirm').mockReturnValue(new Promise((resolve) => {
      resolveConfirm = resolve
    }))
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])
    await flushPromises()
    const preventDefault = vi.fn()

    const result = windowMocks.closeHandler?.({ preventDefault })

    expect(result).toBeUndefined()
    expect(preventDefault).toHaveBeenCalledTimes(1)
    expect(windowMocks.close).not.toHaveBeenCalled()
    resolveConfirm('confirm')
    await flushPromises()
    expect(windowMocks.close).toHaveBeenCalledTimes(1)
  })

  it('allows the follow-up native close request after discard is confirmed', async () => {
    const confirm = vi.spyOn(ElMessageBox, 'confirm').mockResolvedValue('confirm')
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])
    const initialPreventDefault = vi.fn()
    const followUpPreventDefault = vi.fn()
    windowMocks.close.mockImplementationOnce(async () => {
      await windowMocks.closeHandler?.({ preventDefault: followUpPreventDefault })
    })

    await windowMocks.closeHandler?.({ preventDefault: initialPreventDefault })
    await flushPromises()

    expect(initialPreventDefault).toHaveBeenCalledTimes(1)
    expect(followUpPreventDefault).not.toHaveBeenCalled()
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(windowMocks.close).toHaveBeenCalledTimes(1)
  })

  it('keeps only one discard confirmation open across repeated close requests', async () => {
    let cancelConfirmation!: () => void
    const confirm = vi.spyOn(ElMessageBox, 'confirm').mockReturnValue(new Promise((_, reject) => {
      cancelConfirmation = () => reject('cancel')
    }))
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])
    const firstPreventDefault = vi.fn()
    const secondPreventDefault = vi.fn()

    windowMocks.closeHandler?.({ preventDefault: firstPreventDefault })
    windowMocks.closeHandler?.({ preventDefault: secondPreventDefault })

    expect(firstPreventDefault).toHaveBeenCalledTimes(1)
    expect(secondPreventDefault).toHaveBeenCalledTimes(1)
    expect(confirm).toHaveBeenCalledTimes(1)

    cancelConfirmation()
    await flushPromises()
    expect(windowMocks.close).not.toHaveBeenCalled()
  })

  it('automatically computes the unified standard ssim after selecting a baseline', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()

    expect(apiMocks.computeSsim).toHaveBeenCalledTimes(1)
    expect(wrapper.text()).toContain('0.950000 · 10 ms')
  })

  it('shows a pending hash distance while the selected baseline hash is still running', async () => {
    const phashResolvers = new Map<string, (value: { phash: string }) => void>()
    apiMocks.computePhash.mockImplementation(({ path }: { path: string }) => (
      new Promise<{ phash: string }>((resolve) => phashResolvers.set(path, resolve))
    ))
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])
    await vi.waitFor(() => expect(apiMocks.computePhash).toHaveBeenCalledTimes(2))
    phashResolvers.get('candidate')?.({ phash: '0000000000000001' })
    await flushPromises()

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()
    const transientMetrics = wrapper.get('[data-test="card-1"] .metrics-inline').text()

    phashResolvers.get('base')?.({ phash: '0000000000000000' })
    await vi.waitFor(() => expect(wrapper.text()).toContain('感知哈希距离：1'))
    expect(transientMetrics).toContain('感知哈希距离：计算中…')
    expect(transientMetrics).not.toContain('感知哈希距离：失败')
  })

  it('toggles difference highlighting inline from the file metadata row', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])
    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="difference-0"]').exists()).toBe(false)
    const candidateCard = wrapper.get('[data-test="card-1"]')
    const toggle = candidateCard.get('.file-meta [data-test="difference-1"]')
    expect(toggle.attributes('aria-pressed')).toBe('false')

    await toggle.trigger('click')
    await flushPromises()

    expect(apiMocks.computeDifference).toHaveBeenCalledWith(
      expect.objectContaining({ path: 'base' }),
      expect.objectContaining({ path: 'candidate' }),
      50
    )
    expect(candidateCard.get('.metrics-image .el-image__inner').attributes('src'))
      .toBe('data:image/png;base64,highlight')
    expect(toggle.attributes('aria-pressed')).toBe('true')
    expect(document.body.querySelector('.difference-dialog')).toBeNull()

    await toggle.trigger('click')
    await flushPromises()

    expect(candidateCard.get('.metrics-image .el-image__inner').attributes('src'))
      .toBe('data:image/png;base64,candidate')
    expect(toggle.attributes('aria-pressed')).toBe('false')
  })

  it('keeps difference preview errors in the candidate card and offers retry', async () => {
    apiMocks.computeDifference.mockRejectedValueOnce(new Error('生成失败'))
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])
    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()

    await wrapper.get('[data-test="difference-1"]').trigger('click')
    await flushPromises()

    const candidateCard = wrapper.get('[data-test="card-1"]')
    expect(candidateCard.text()).toContain('差异高亮生成失败：生成失败')
    expect(candidateCard.find('[data-test="difference-retry-1"]').exists()).toBe(true)
    expect(document.body.querySelector('.difference-dialog')).toBeNull()

    apiMocks.computeDifference.mockResolvedValueOnce({
      baselineDataUrl: 'data:image/png;base64,baseline',
      candidateDataUrl: 'data:image/png;base64,candidate',
      highlightDataUrl: 'data:image/png;base64,retried-highlight',
      width: 100,
      height: 100,
      changedPixelRatio: 0.12,
      regionCount: 2
    })
    await candidateCard.get('[data-test="difference-retry-1"]').trigger('click')
    await flushPromises()

    expect(candidateCard.get('.metrics-image .el-image__inner').attributes('src'))
      .toBe('data:image/png;base64,retried-highlight')
  })

  it('renders candidate metrics in one compact line', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('感知哈希距离：1'))

    const metrics = wrapper.get('[data-test="card-1"] .metrics-inline')
    expect(metrics.text()).toContain('感知哈希距离：1')
    expect(metrics.text()).not.toContain('/ 64')
    expect(metrics.text()).toContain('标准 SSIM：0.950000 · 10 ms')
    expect(metrics.text()).not.toContain('低精度')
  })

  it('renders baseline and candidate hashes on separate tooltip lines', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])
    await wrapper.get('[data-test="card-0"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.text()).toContain('感知哈希距离：1'))

    await wrapper.get('[data-test="card-1"] .metrics-inline .metric-chip').trigger('mouseenter')

    await vi.waitFor(() => {
      expect(document.body.querySelectorAll('.phash-tooltip-line')).toHaveLength(2)
    })
    expect(
      Array.from(document.body.querySelectorAll('.phash-tooltip-line'))
        .map(element => element.textContent?.trim())
    ).toEqual([
      '标准图感知哈希：0000000000000000',
      '当前图片感知哈希：0000000000000001'
    ])
  })

  it('keeps the baseline instruction in the header instead of a disappearing banner', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])

    expect(wrapper.find('.window-header').text()).toContain('点击一张图片卡片，将它设为标准图')
    expect(wrapper.find('.baseline-hint').exists()).toBe(false)

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('.window-header').text()).toContain('点击一张图片卡片，将它设为标准图')
    expect(wrapper.find('.baseline-hint').exists()).toBe(false)
  })

  it('explains how to interpret and combine both metrics without overclaiming', async () => {
    const wrapper = mountView()
    const guideButton = wrapper.findAll('button').find(button => button.text().includes('指标说明'))
    expect(guideButton).toBeDefined()

    await guideButton!.trigger('click')
    await vi.waitFor(() => {
      expect(document.body.textContent).toContain('快速粗筛')
    })

    const guideText = document.body.textContent || ''
    expect(guideText).toContain('只负责快速找出可能相似的候选')
    expect(guideText).toContain('不能单独用来判断重复或删除')
    expect(guideText).toContain('精细对比')
    expect(guideText).toContain('直接显示原始值，不转换成百分比')
    expect(guideText).toContain('不代表当前图一定是原图、压缩图或低质量图')
    expect(guideText).toContain('本页只展示算法证据，不给出删除结论')
    expect(guideText).toContain('与主程序、组内交叉比较和找差分图共用同一套实现')
    expect(guideText).toContain('最多 4 路共享并行计算')
  })

  it('offers a retry action when ssim calculation fails', async () => {
    apiMocks.computeSsim.mockRejectedValueOnce(new Error('临时失败'))
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await vi.waitFor(() => expect(wrapper.find('[data-test="ssim-1"]').exists()).toBe(true))
    apiMocks.computeSsim.mockResolvedValueOnce({ score: 0.88, durationMs: 3 })
    await wrapper.get('[data-test="ssim-1"]').trigger('click')
    await flushPromises()

    expect(apiMocks.computeSsim).toHaveBeenCalledTimes(2)
    expect(wrapper.text()).toContain('0.880000')
  })

  it('does not print the original resolution in image cards', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['large'])

    expect(wrapper.text()).not.toContain('100 × 100')
  })

  it('shows a card placeholder while an image is loading', async () => {
    let resolveLoad!: (value: Awaited<ReturnType<typeof apiMocks.load>>) => void
    apiMocks.load.mockReturnValueOnce(new Promise((resolve) => { resolveLoad = resolve }))
    const wrapper = mountView()

    const importing = (wrapper.vm as unknown as {
      addImagePathsForTest: (paths: string[]) => Promise<void>
    }).addImagePathsForTest(['slow'])
    await wrapper.vm.$nextTick()

    expect(wrapper.find('[data-test="card-0"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="card-0"]').text()).toContain('正在读取图片')
    resolveLoad({
      path: 'slow',
      fileName: 'slow.png',
      fileSize: 100,
      width: 100,
      height: 100,
      modifiedAtMs: 1,
      thumbnailDataUrl: 'data:image/png;base64,slow'
    })
    await importing
  })

  it('notifies when duplicate images are skipped', async () => {
    const info = vi.spyOn(ElMessage, 'info')
    const wrapper = mountView()

    await addPaths(wrapper, ['same', 'same'])

    expect(info).toHaveBeenCalledWith('已跳过 1 张重复图片')
  })
})
