# Thumbnail Group Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show whether each SSIM-completed group has thumbnails and provide a live filter that only lists completed groups with thumbnails while preserving original group numbers.

**Architecture:** Extract the automatic original-image classification into a pure frontend feature so the detail table and group list use the same rule. The group list combines that result with the existing reactive SSIM status; pending/running groups stay unconfirmed, and completed groups become either `has` or `empty`. Pagination and empty states consume the filtered list without rewriting `group_index`.

**Tech Stack:** Vue 3, Pinia, Element Plus, TypeScript, Vitest, Vue Test Utils.

---

### Task 1: Share Automatic Thumbnail Classification

**Files:**
- Create: `src/features/groupThumbnails.ts`
- Create: `src/features/groupThumbnails.spec.ts`
- Modify: `src/components/ComparisonGroupDetail.vue`

- [x] **Step 1: Write the failing pure-function tests**

Test one group whose second image is a lower-resolution candidate and another whose images all satisfy the automatic-original rule. Assert that only the first group reports thumbnails.

- [x] **Step 2: Run the focused test and verify RED**

Run: `npx vitest run src/features/groupThumbnails.spec.ts`

Expected: FAIL because `groupHasThumbnailCandidates` does not exist.

- [x] **Step 3: Implement the shared automatic-original rule**

Export `getAutomaticOriginalImageIds(group, threshold)` and `groupHasThumbnailCandidates(group, threshold)`. Match the existing detail behavior: representative/reference images are originals; other images require at least 90% of maximum pixels, at most 2% aspect-ratio difference, and an absent or threshold-passing task SSIM.

- [x] **Step 4: Make the detail table consume the shared IDs**

Replace the local automatic-original calculation with the shared result while retaining manual original/thumbnail overrides and the existing fallback behavior.

- [x] **Step 5: Run the helper and detail tests**

Run: `npx vitest run src/features/groupThumbnails.spec.ts src/components/ComparisonGroupDetail.spec.ts`

Expected: PASS.

### Task 2: Add Live Group Status and Filter

**Files:**
- Modify: `src/components/ComparisonResults.vue`
- Modify: `src/components/ComparisonResults.spec.ts`

- [x] **Step 1: Write failing component tests**

Assert that completed groups visibly show `有缩略图` or `无缩略图`. Turn on `只看缩略图`, assert only completed groups with thumbnails remain, then change a pending group's status to completed and assert it appears without renumbering.

- [x] **Step 2: Run the focused test and verify RED**

Run: `npx vitest run src/components/ComparisonResults.spec.ts`

Expected: FAIL because the status labels and filter do not exist.

- [x] **Step 3: Implement reactive filtering**

Add a local `showOnlyThumbnailGroups` switch beside the mode switch. Derive `displayedGroups` from `store.groups`, `store.getGroupSimilarityStatus(group)`, and `groupHasThumbnailCandidates`. Use `displayedGroups` for the table, pagination, selected-page synchronization, total count, and the filtered empty state.

- [x] **Step 4: Render visible group-level thumbnail status**

Add a compact table column: pending/running groups show `待 SSIM` or `比对中`; completed groups show `有缩略图` or `无缩略图`.

- [x] **Step 5: Run the focused component tests**

Run: `npx vitest run src/components/ComparisonResults.spec.ts`

Expected: PASS.

### Task 3: Verify the Complete Frontend

**Files:**
- Verify all modified frontend files.

- [x] **Step 1: Run all frontend tests**

Run: `npm test`

Expected: all tests pass.

- [x] **Step 2: Run the production build**

Run: `npm run build`

Expected: TypeScript checking and Vite build exit successfully.

- [x] **Step 3: Check the final diff**

Run: `git diff --check`

Expected: no whitespace errors; existing `outputs/` and `work/` remain untouched and untracked.
