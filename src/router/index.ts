import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'Main',
    component: () => import('@/views/MainView.vue')
  },
  {
    path: '/settings',
    name: 'Settings',
    component: () => import('@/views/SettingsView.vue')
  },
  {
    path: '/export',
    name: 'Export',
    component: () => import('@/views/ExportView.vue')
  },
  {
    path: '/difference-finder',
    name: 'DifferenceFinder',
    component: () => import('@/views/DifferenceFinderView.vue')
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router
