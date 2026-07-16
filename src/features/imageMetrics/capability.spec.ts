// @vitest-environment node
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('image metrics window capability', () => {
  it('grants only the window commands used by the standalone tool', () => {
    const capability = JSON.parse(
      readFileSync(resolve(process.cwd(), 'src-tauri/capabilities/default.json'), 'utf8')
    ) as { permissions: string[] }

    expect(capability.permissions).toEqual(expect.arrayContaining([
      'core:webview:allow-create-webview-window',
      'core:window:allow-show',
      'core:window:allow-unminimize',
      'core:window:allow-set-focus',
      'core:window:allow-close'
    ]))
  })
})
