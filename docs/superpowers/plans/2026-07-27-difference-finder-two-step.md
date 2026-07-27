# Difference Finder Two-Step Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the four-panel difference finder with a two-step flow where search setup comes first and matching files are selected and renamed in one table.

**Architecture:** `DifferenceFinderView.vue` owns the visible workflow step and switches after `SearchSetupPanel` emits search completion. `BatchRenameGrid.vue` becomes the only results workspace: it renders every filtered match, owns row selection and inline rename editing, and keeps the existing preview, conflict checking, transfer, rename, and undo APIs.

**Tech Stack:** Vue 3, Pinia, Element Plus, Vitest, Vue Test Utils, Tauri APIs

---

### Task 1: Two-step page shell

**Files:**
- Create: `src/views/DifferenceFinderView.spec.ts`
- Modify: `src/views/DifferenceFinderView.vue`
- Modify: `src/components/difference-finder/SearchSetupPanel.vue`

- [x] **Step 1: Write the failing view test**

Mount the view with stubs, assert that setup is visible initially, emit `search-complete`, then assert that the result table replaces setup and that `data-test="edit-search"` returns to setup.

- [x] **Step 2: Run test to verify it fails**

Run: `npm test -- src/views/DifferenceFinderView.spec.ts`

Expected: FAIL because the current view renders all four panels simultaneously.

- [x] **Step 3: Implement the workflow state**

Use a local `ref<'setup' | 'results'>('setup')`, render a compact two-step indicator, show setup components only in step one, and show `BatchRenameGrid` only in step two. Emit after `await store.search()`:

```ts
const emit = defineEmits<{ searchComplete: [] }>()
await store.search()
emit('searchComplete')
```

- [x] **Step 4: Run test to verify it passes**

Run: `npm test -- src/views/DifferenceFinderView.spec.ts`

Expected: PASS.

### Task 2: Unified selection and rename table

**Files:**
- Create: `src/components/difference-finder/BatchRenameGrid.spec.ts`
- Modify: `src/components/difference-finder/BatchRenameGrid.vue`

- [x] **Step 1: Write the failing table test**

Seed two store matches, mount the component, assert both rows are visible before selection, check one row, and assert `store.selectedPaths` contains only that file.

- [x] **Step 2: Run test to verify it fails**

Run: `npm test -- src/components/difference-finder/BatchRenameGrid.spec.ts`

Expected: FAIL because the current grid renders only selected files and delegates selection to a separate result list.

- [x] **Step 3: Implement the single-table workspace**

Render `store.filteredMatches` as stable table rows with checkbox, thumbnail, original metadata, inline new-name input, match classification, and validation status. Keep batch rules in a collapsed secondary section and keep one primary action, `执行重命名`, for the checked rows.

- [x] **Step 4: Run focused tests**

Run: `npm test -- src/views/DifferenceFinderView.spec.ts src/components/difference-finder/BatchRenameGrid.spec.ts`

Expected: PASS.

### Task 3: Verification and visual QA

**Files:**
- Modify only if verification exposes an issue.

- [x] **Step 1: Run the complete frontend suite**

Run: `npm test`

Expected: all tests pass.

- [x] **Step 2: Run the production frontend build**

Run: `npm run build`

Expected: TypeScript and Vite build exit successfully.

- [x] **Step 3: Inspect the actual desktop-sized page**

Start the Vite server, capture the `/difference-finder` route at 1920x1080 and a narrower desktop viewport, and confirm there is no simultaneous four-panel layout, overlapping text, clipped actions, or nested scrolling.
