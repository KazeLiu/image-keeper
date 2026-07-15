import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus, { ElMessageBox } from 'element-plus'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ImageMetricsTestView from './ImageMetricsTestView.vue'

const apiMocks = vi.hoisted(() => ({
  computeHigh: vi.fn(async () => ({ score: 0.95, durationMs: 10 }))
}))

const windowMocks = vi.hoisted(() => ({
  close: vi.fn(async () => undefined),
  closeHandler: undefined as undefined | ((event: { preventDefault: () => void }) => Promise<void>)
}))

vi.mock('@/api/imageMetrics', () => ({
  loadTestImage: vi.fn(async (path: string) => ({
    path,
    fileName: `${path}.png`,
    fileSize: 100,
    width: 100,
    height: 100,
    modifiedAtMs: 1,
    phash: path,
    thumbnailDataUrl: `data:image/png;base64,${path}`
  })),
  computeTestLowPrecision: vi.fn(async () => ({
    phashDistance: 1,
    similarity: 0.9,
    durationMs: 2
  })),
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
  })
})
