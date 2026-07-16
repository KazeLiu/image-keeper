import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

export const IMAGE_METRICS_WINDOW_LABEL = 'image-metrics-test'

let pendingWindow: Promise<WebviewWindow> | null = null

async function focusWindow(window: WebviewWindow) {
  await window.show()
  await window.unminimize()
  await window.setFocus()
  return window
}

async function openOrCreateImageMetricsWindow() {
  const existing = await WebviewWindow.getByLabel(IMAGE_METRICS_WINDOW_LABEL)
  if (existing) {
    return focusWindow(existing)
  }

  const window = new WebviewWindow(IMAGE_METRICS_WINDOW_LABEL, {
    url: '/image-metrics-test',
    title: 'ImageKeeper - 图片指标测试',
    width: 1180,
    height: 820,
    minWidth: 840,
    minHeight: 600,
    resizable: true,
    center: true
  })

  return new Promise<WebviewWindow>((resolve, reject) => {
    let settled = false
    const finish = (result: { window: WebviewWindow } | { error: unknown }) => {
      if (settled) return
      settled = true
      if ('window' in result) resolve(result.window)
      else reject(result.error instanceof Error ? result.error : new Error(String(result.error)))
    }

    void Promise.all([
      window.once('tauri://created', () => finish({ window })),
      window.once<string>('tauri://error', (event) => {
        finish({ error: event.payload || '创建图片指标测试窗口失败' })
      })
    ]).catch((error) => finish({ error }))
  })
}

export function openImageMetricsWindow() {
  if (pendingWindow) return pendingWindow
  pendingWindow = openOrCreateImageMetricsWindow()
  void pendingWindow.finally(() => {
    pendingWindow = null
  }).catch(() => undefined)
  return pendingWindow
}
