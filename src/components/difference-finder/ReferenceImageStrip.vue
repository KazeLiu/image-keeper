<template>
  <section class="panel reference-panel" aria-labelledby="reference-title">
    <div class="panel-heading">
      <div>
        <h2 id="reference-title">参考图片</h2>
        <p>可一次添加多张，用来查找相同图片和差分图。</p>
      </div>
      <el-button type="primary" plain :icon="Plus" @click="chooseReferences">添加图片</el-button>
    </div>

    <div v-if="store.references.length" class="reference-strip">
      <article
        v-for="reference in store.references"
        :key="reference.id"
        class="reference-card image-card"
      >
        <img :src="convertFileSrc(reference.path)" :alt="reference.name" />
        <span :title="reference.name">{{ reference.name }}</span>
        <el-button
          class="remove-reference"
          :icon="Close"
          circle
          text
          aria-label="移除参考图"
          @click.stop="store.removeReference(reference.id)"
        />
      </article>
    </div>

    <button v-else type="button" class="drop-empty" @click="chooseReferences">
      <el-icon><PictureRounded /></el-icon>
      <span>点击选择，或把参考图片拖进窗口</span>
      <small>支持 JPG、PNG、WebP、BMP、GIF</small>
    </button>
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { Close, PictureRounded, Plus } from '@element-plus/icons-vue'
import { useDifferenceFinderStore } from '@/stores/differenceFinderStore'

const store = useDifferenceFinderStore()
let unlistenDrop: (() => void) | null = null

async function chooseReferences() {
  const result = await open({
    multiple: true,
    directory: false,
    title: '选择参考图片',
    filters: [{ name: '图片', extensions: ['jpg', 'jpeg', 'png', 'webp', 'bmp', 'gif'] }]
  })
  if (!result) return
  store.addReferences(Array.isArray(result) ? result : [result])
}

onMounted(async () => {
  if (!('__TAURI_INTERNALS__' in window)) return
  unlistenDrop = await getCurrentWebview().onDragDropEvent(event => {
    if (event.payload.type !== 'drop') return
    const imagePaths = event.payload.paths.filter(path => /\.(jpe?g|png|webp|bmp|gif)$/i.test(path))
    if (imagePaths.length) store.addReferences(imagePaths)
  })
})

onBeforeUnmount(() => unlistenDrop?.())
</script>

<style scoped>
.panel {
  border: 1px solid #dcdfe6;
  border-radius: 10px;
  background: #fff;
}

.reference-panel { padding: 18px; }

.panel-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}

.panel-heading h2 { margin: 0; font-size: 17px; }
.panel-heading p { margin: 5px 0 0; color: #606266; font-size: 12px; }

.reference-strip { display: flex; gap: 10px; overflow-x: auto; padding: 2px 2px 6px; }
.reference-card { position: relative; width: 112px; height: 116px; flex: 0 0 auto; border: 1px solid #dcdfe6; border-radius: 8px; background: #fff; overflow: hidden; }
.reference-card > img { width: 100%; height: 82px; display: block; object-fit: cover; background: #f2f3f5; }
.reference-card > span { display: block; padding: 8px 7px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.remove-reference { position: absolute; top: 3px; right: 3px; width: 28px; height: 28px; background: rgba(255,255,255,.92); }
.drop-empty { width: 100%; min-height: 112px; border: 1px dashed #c0c4cc; border-radius: 8px; background: #fafcff; color: #606266; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 5px; cursor: pointer; }
.drop-empty:hover { border-color: #409eff; color: #409eff; }
.drop-empty .el-icon { font-size: 28px; }
.drop-empty small { color: #909399; }
</style>
