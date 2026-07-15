<template>
  <div class="comparison-progress">
    <el-card v-if="store.isRunning || store.stats || store.errorMessage" shadow="never" class="progress-card">
      <el-alert
        v-if="store.errorMessage"
        :title="store.errorMessage"
        type="error"
        :closable="false"
        show-icon
        class="status-alert"
      />

      <!-- 进度显示 -->
      <div v-if="store.isRunning" class="progress-section" :style="{ '--phase-color': progressColor }">
        <div class="progress-header">
          <div>
            <span class="phase-name">{{ phaseName }}</span>
            <span v-if="phaseStepText" class="phase-step">{{ phaseStepText }}</span>
          </div>
          <span v-if="hasMeasurableProgress" class="progress-text">{{ progressText }}</span>
          <span v-else class="progress-text">正在工作</span>
        </div>

        <div class="status-runner" role="status" aria-live="polite">
          <span class="runner-orb" aria-hidden="true" />
          <div class="runner-copy">
            <div class="shimmer-text">{{ currentActionText }}</div>
            <div class="phase-description">{{ phaseDescription }}</div>
          </div>
        </div>

        <div v-if="hasMeasurableProgress" class="progress-meta">
          <span>{{ processedSummary }}</span>
        </div>
      </div>

      <!-- 统计结果 -->
      <div v-if="store.stats" class="stats-section">
        <div class="stats-header">
          <h3 class="stats-title">对比结果统计</h3>
          <el-button size="small" @click="store.refreshAnalysisData" :icon="Refresh">刷新</el-button>
        </div>

        <el-alert
          v-if="store.isSingleInternalComparison"
          type="info"
          :closable="false"
          show-icon
          class="mode-explanation"
        >
          <template #title>
            单目录内部互比：左边表示这个文件夹共扫描了多少张图，右边表示有多少张图参与了内部分析。
          </template>
        </el-alert>

        <!-- 总览 -->
        <div class="stats-overview">
          <div class="stat-item">
            <span class="stat-label">{{ store.isSingleInternalComparison ? '文件夹图片' : '基准图片' }}</span>
            <span class="stat-value">{{ store.stats.baseline_total }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">{{ store.isSingleInternalComparison ? '参与分析' : '对比图片' }}</span>
            <span class="stat-value">{{ store.stats.comparison_total }}</span>
          </div>
        </div>

        <!-- 分类统计 -->
        <div class="category-stats">
          <div
            v-for="category in store.categoryStats"
            :key="category.type"
            class="category-item"
            :class="{ 'has-count': category.count > 0 }"
          >
            <div class="category-info">
              <span class="category-title" :style="{ color: category.color }">
                <span class="category-label">{{ category.label }}</span>
                <el-tooltip placement="top" effect="dark">
                  <template #content>
                    <div class="category-tip">{{ category.description }}</div>
                  </template>
                  <el-icon class="category-tip-icon" :aria-label="`${category.label}说明`">
                    <QuestionFilled />
                  </el-icon>
                </el-tooltip>
              </span>
              <span class="category-count">{{ category.count }}</span>
            </div>
            <el-progress
              :percentage="getPercentage(category.count)"
              :color="category.color"
              :show-text="false"
              class="category-progress"
            />
          </div>
        </div>

        <!-- 守恒验证 -->
        <div class="conservation-check" :class="{ valid: store.conservationCheck.valid }">
          <el-icon v-if="store.conservationCheck.valid" class="check-icon"><CircleCheck /></el-icon>
          <el-icon v-else class="check-icon"><Warning /></el-icon>
          <span class="check-message">{{ store.conservationCheck.message }}</span>
        </div>

        <!-- 审核统计 -->
        <div v-if="store.stats.pending_review > 0" class="review-stats">
          <el-alert type="info" :closable="false" show-icon>
            <template #title>
              待复核：{{ store.stats.pending_review }} 个变体需要人工审核
            </template>
          </el-alert>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Refresh, CircleCheck, Warning, QuestionFilled } from '@element-plus/icons-vue'
import { useComparisonStore } from '@/stores/comparisonStore'

const store = useComparisonStore()

const phaseDetails: Record<string, { description: string; action: string }> = {
  pending: {
    description: '准备创建本次对比任务。',
    action: '正在准备任务...'
  },
  preflight: {
    description: '检查目录是否可读、路径是否重复，以及任务边界是否成立。',
    action: '正在检查目录...'
  },
  indexing: {
    description: '扫描图片文件，读取基础信息，并建立后续分析需要的索引。',
    action: '正在扫描图片并提取特征...'
  },
  matching: {
    description: '使用文件哈希查找完全重复的图片。',
    action: '正在查找完全重复图片...'
  },
  candidate_search: {
    description: '粗筛内容相近的图片，把可能相似的图先聚到一起。',
    action: '正在筛选相似候选图片...'
  },
  scoring: {
    description: '计算组内图片相似程度，为低质量图判断提供依据。',
    action: '正在计算图片相似度...'
  },
  resolving: {
    description: '整理分组、分类与统计结果，准备展示给你复核。',
    action: '正在整理结果...'
  },
  complete: {
    description: '对比任务已经完成。',
    action: '结果整理完成'
  },
  paused: {
    description: '对比任务已暂停。',
    action: '等待继续...'
  },
  canceled: {
    description: '对比任务已取消。',
    action: '任务已取消'
  },
  failed: {
    description: '对比任务执行失败。',
    action: '任务失败'
  }
}

const phaseOrder = ['preflight', 'indexing', 'matching', 'candidate_search', 'scoring', 'resolving']

const activePhase = computed(() => {
  return store.currentPhase || store.progressModel.phase || 'pending'
})

const phaseName = computed(() => {
  return store.getPhaseName(activePhase.value)
})

const progressColor = computed(() => {
  const phase = activePhase.value
  if (phase === 'preflight') return '#909399'
  if (phase === 'indexing') return '#409eff'
  if (phase === 'matching') return '#e6a23c'
  if (phase === 'candidate_search') return '#8b5cf6'
  if (phase === 'scoring') return '#67c23a'
  if (phase === 'resolving') return '#409eff'
  return '#409eff'
})

const phaseDescription = computed(() => {
  return phaseDetails[activePhase.value]?.description || '正在执行当前阶段。'
})

const currentActionText = computed(() => {
  return store.progressModel.currentFile || phaseDetails[activePhase.value]?.action || '正在处理...'
})

const hasMeasurableProgress = computed(() => {
  return store.progressModel.totalFiles > 0
})

const phaseStepText = computed(() => {
  const index = phaseOrder.indexOf(activePhase.value)
  if (index < 0) return ''
  return `${index + 1} / ${phaseOrder.length}`
})

const progressText = computed(() => {
  if (!hasMeasurableProgress.value) return ''
  return `${store.progressModel.processedFiles} / ${store.progressModel.totalFiles}`
})

const processedSummary = computed(() => {
  if (!hasMeasurableProgress.value) return ''
  return `已处理 ${store.progressModel.processedFiles} / 共 ${store.progressModel.totalFiles}`
})

function getPercentage(count: number): number {
  if (!store.stats || store.stats.comparison_total === 0) return 0
  return Math.round((count / store.stats.comparison_total) * 100)
}
</script>

<style scoped lang="scss">
.comparison-progress {
  width: 100%;
}

.progress-card {
  border-radius: 8px;
}

.status-alert {
  margin-bottom: 16px;
}

.progress-section {
  --phase-color: #409eff;
  padding: 14px;
  border: 1px solid color-mix(in srgb, var(--phase-color) 22%, #ebeef5);
  border-radius: 10px;
  background:
    radial-gradient(circle at 18% 12%, color-mix(in srgb, var(--phase-color) 12%, transparent), transparent 28%),
    linear-gradient(135deg, #ffffff 0%, #f8fbff 100%);

  .progress-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;

    .phase-name {
      font-weight: 600;
      font-size: 16px;
      color: #303133;
    }

    .phase-step {
      display: inline-flex;
      align-items: center;
      height: 20px;
      margin-left: 8px;
      padding: 0 8px;
      border-radius: 999px;
      background-color: color-mix(in srgb, var(--phase-color) 12%, #ffffff);
      color: var(--phase-color);
      font-size: 12px;
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }

    .progress-text {
      font-size: 14px;
      color: var(--phase-color);
      font-weight: 600;
      font-variant-numeric: tabular-nums;
    }
  }

  .status-runner {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.76);
    box-shadow: inset 0 0 0 1px rgba(64, 158, 255, 0.08);

    .runner-orb {
      flex-shrink: 0;
      width: 10px;
      height: 10px;
      margin-top: 5px;
      border-radius: 999px;
      background-color: var(--phase-color);
      box-shadow: 0 0 0 6px color-mix(in srgb, var(--phase-color) 14%, transparent);
      animation: runner-pulse 1.4s ease-in-out infinite;
    }

    .runner-copy {
      min-width: 0;
      flex: 1;
    }

    .shimmer-text {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 13px;
      font-weight: 650;
      line-height: 20px;
      color: #303133;
      background: linear-gradient(
        110deg,
        #303133 0%,
        #303133 34%,
        #ffffff 47%,
        #303133 60%,
        #303133 100%
      );
      background-size: 240% 100%;
      background-clip: text;
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      animation: shimmer-sweep 1.85s ease-in-out infinite;
    }

    .phase-description {
      margin-top: 2px;
      color: #606266;
      font-size: 12px;
      line-height: 18px;
    }
  }

  .progress-meta {
    margin-top: 10px;
    color: #909399;
    font-size: 12px;
    line-height: 18px;

    span {
      color: color-mix(in srgb, var(--phase-color) 82%, #303133);
      font-weight: 500;
    }
  }
}

@keyframes shimmer-sweep {
  0% {
    background-position: 130% 0;
  }

  100% {
    background-position: -130% 0;
  }
}

@keyframes runner-pulse {
  0%,
  100% {
    transform: scale(0.92);
    opacity: 0.72;
  }

  50% {
    transform: scale(1);
    opacity: 1;
  }
}

@media (prefers-reduced-motion: reduce) {
  .progress-section {
    .status-runner {
      .runner-orb,
      .shimmer-text {
        animation: none;
      }

      .shimmer-text {
        background: none;
        -webkit-text-fill-color: #303133;
      }
    }
  }
}

.stats-section {
  margin-top: 24px;

  .stats-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;

    .stats-title {
      font-size: 16px;
      font-weight: 600;
      margin: 0;
    }
  }

  .stats-overview {
    display: flex;
    gap: 16px;
    margin-bottom: 20px;

    .stat-item {
      flex: 1;
      padding: 12px;
      background-color: #f5f7fa;
      border-radius: 6px;
      display: flex;
      flex-direction: column;
      gap: 4px;

      .stat-label {
        font-size: 12px;
        color: #909399;
      }

      .stat-value {
        font-size: 20px;
        font-weight: 600;
        color: #303133;
      }
    }
  }

  .mode-explanation {
    margin-bottom: 16px;
  }

  .category-stats {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 16px;

    .category-item {
      opacity: 0.5;
      transition: opacity 0.2s;

      &.has-count {
        opacity: 1;
      }

      .category-info {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 6px;

        .category-title {
          display: inline-flex;
          align-items: center;
          gap: 4px;
          font-size: 12px;
          font-weight: 600;
        }

        .category-tip-icon {
          font-size: 13px;
          opacity: 0.72;
          cursor: help;
          transition: opacity 0.2s ease;

          &:hover {
            opacity: 1;
          }
        }

        .category-count {
          font-weight: 600;
          font-size: 14px;
        }
      }

      .category-progress {
        width: 100%;
      }
    }
  }

  .conservation-check {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 4px;
    background-color: #f56c6c20;
    color: #f56c6c;
    font-size: 13px;
    margin-bottom: 16px;

    &.valid {
      background-color: #67c23a20;
      color: #67c23a;
    }

    .check-icon {
      flex-shrink: 0;
      font-size: 16px;
    }

    .check-message {
      font-weight: 500;
    }
  }

  .review-stats {
    margin-top: 12px;
  }
}

.category-tip {
  max-width: 260px;
  line-height: 1.5;
}
</style>
