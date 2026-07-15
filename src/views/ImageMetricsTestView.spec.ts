import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus, { ElMessage, ElMessageBox } from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ImageMetricsTestView from './ImageMetricsTestView.vue'

const apiMocks = vi.hoisted(() => ({
  load: vi.fn(),
  computeLow: vi.fn(),
  computeHigh: vi.fn(async () => ({ score: 0.95, durationMs: 10 }))
}))

const windowMocks = vi.hoisted(() => ({
  close: vi.fn(async () => undefined),
  closeHandler: undefined as undefined | ((event: { preventDefault: () => void }) => Promise<void>)
}))

vi.mock('@/api/imageMetrics', () => ({
  loadTestImage: apiMocks.load,
  computeTestLowPrecision: apiMocks.computeLow,
  computeTestStandardSsim: apiMocks.computeHigh
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
      phash: path === 'candidate' ? '0000000000000001' : '0000000000000000',
      thumbnailDataUrl: `data:image/png;base64,${path}`
    }))
    apiMocks.computeLow.mockResolvedValue({
      similarity: 0.9,
      durationMs: 2
    })
    apiMocks.computeHigh.mockResolvedValue({ score: 0.95, durationMs: 10 })
  })

  it('closes an empty window without confirmation', async () => {
    const confirm = vi.spyOn(ElMessageBox, 'confirm')
    const wrapper = mountView()

    await wrapper.get('[data-test="close-window"]').trigger('click')
    await flushPromises()

    expect(confirm).not.toHaveBeenCalled()
    expect(windowMocks.close).toHaveBeenCalledTimes(1)
  })

  it('keeps a non-empty window when discard confirmation is canceled', async () => {
    vi.spyOn(ElMessageBox, 'confirm').mockRejectedValue('cancel')
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])

    await wrapper.get('[data-test="close-window"]').trigger('click')
    await flushPromises()

    expect(windowMocks.close).not.toHaveBeenCalled()
  })

  it('keeps the session when the native close call fails', async () => {
    vi.spyOn(ElMessageBox, 'confirm').mockResolvedValue('confirm')
    windowMocks.close.mockRejectedValueOnce(new Error('关闭失败'))
    const wrapper = mountView()
    await addPaths(wrapper, ['a'])

    await wrapper.get('[data-test="close-window"]').trigger('click')
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

  it('computes standard ssim only after the card action is clicked', async () => {
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()
    expect(apiMocks.computeHigh).not.toHaveBeenCalled()

    await wrapper.get('[data-test="high-1"]').trigger('click')
    await flushPromises()

    expect(apiMocks.computeHigh).toHaveBeenCalledTimes(1)
    expect(wrapper.text()).toContain('0.950000 · 10 ms')
  })

  it('offers a retry action when low precision calculation fails', async () => {
    apiMocks.computeLow.mockRejectedValueOnce(new Error('临时失败'))
    const wrapper = mountView()
    await addPaths(wrapper, ['base', 'candidate'])

    await wrapper.get('[data-test="card-0"]').trigger('click')
    await flushPromises()
    apiMocks.computeLow.mockResolvedValueOnce({
      similarity: 0.88,
      durationMs: 3
    })
    await wrapper.get('[data-test="low-1"]').trigger('click')
    await flushPromises()

    expect(apiMocks.computeLow).toHaveBeenCalledTimes(2)
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

    expect(wrapper.find('[data-test="loading-card"]').exists()).toBe(true)
    resolveLoad({
      path: 'slow',
      fileName: 'slow.png',
      fileSize: 100,
      width: 100,
      height: 100,
      modifiedAtMs: 1,
      phash: '0000000000000000',
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
