import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

export const IMAGE_METRICS_WINDOW_LABEL = 'image-metrics-test'

export async function openImageMetricsWindow() {
  const existing = await WebviewWindow.getByLabel(IMAGE_METRICS_WINDOW_LABEL)
  if (existing) {
    await existing.show()
    await existing.unminimize()
    await existing.setFocus()
    return existing
  }

  return new WebviewWindow(IMAGE_METRICS_WINDOW_LABEL, {
    url: '/image-metrics-test',
    title: 'ImageKeeper - 图片指标测试',
    width: 1180,
    height: 820,
    minWidth: 840,
    minHeight: 600,
    resizable: true,
    center: true
  })
}
