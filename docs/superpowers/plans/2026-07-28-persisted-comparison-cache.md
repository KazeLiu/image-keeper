# Persisted Comparison Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist formal comparison grouping and group SSIM results, then backfill unfinished groups at low priority so completed history opens without repeating expensive image analysis.

**Architecture:** SQLite stores one immutable grouping snapshot per run, algorithm profile, and grouping distance, plus group SSIM snapshots keyed by the sorted member fingerprints. Foreground group review reads the persistent cache before computing and starts a deduplicated one-worker backfill after it completes; difference finder and image-metrics commands remain on their existing real-time path and never query these tables.

**Tech Stack:** Rust 2021, Tauri 2, rusqlite, Rayon, Vue 3, TypeScript, Vitest

---

### Task 1: Add derived-result cache tables

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Test: `src-tauri/src/db/schema.rs`

- [x] **Step 1: Write a failing schema test**

Assert initialization creates `comparison_group_cache` and `comparison_group_similarity_cache`, both linked to `runs` with cascading deletion.

- [x] **Step 2: Run the schema test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::schema::tests::test_initialize_database -- --exact`

Expected: FAIL because the two cache tables do not exist.

- [x] **Step 3: Add the tables and indexes**

The grouping table uses a unique `(run_id, algorithm_profile_id, grouping_distance)` key. The similarity table uses a unique `(run_id, algorithm_profile_id, member_signature)` key. Both reference `runs` with cascading deletion.

- [x] **Step 4: Run the schema test and verify GREEN**

Run the command from Step 2 and expect one passing test.

### Task 2: Persist and reuse grouping snapshots

**Files:**
- Modify: `src-tauri/src/commands/comparison.rs`
- Test: `src-tauri/src/commands/comparison.rs`

- [x] **Step 1: Write failing cache behavior tests**

Cover snapshot creation, reuse without repeating pHash grouping, isolation by grouping distance, and invalidation after the run's operation revision changes.

- [x] **Step 2: Run the focused grouping tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::comparison::tests::comparison_group_cache -- --nocapture`

Expected: FAIL because no persistent grouping cache exists.

- [x] **Step 3: Implement read-through grouping persistence**

`read_comparison_groups` first validates and decodes a persisted snapshot, then builds and persists the existing response shape on a miss. Snapshots are validated with the eligible image count and latest operation-log revision, so a hit is O(1) SQLite work plus JSON decoding.

- [x] **Step 4: Run grouping tests and verify GREEN**

Run the command from Step 2 and expect all focused tests to pass.

### Task 3: Persist group similarity snapshots safely

**Files:**
- Modify: `src-tauri/src/commands/comparison.rs`
- Test: `src-tauri/src/commands/comparison.rs`

- [x] **Step 1: Write failing SSIM persistence tests**

Verify member order does not change the key, a current-file fingerprint change misses the cache, valid scores round-trip, and error results are not persisted.

- [x] **Step 2: Run focused tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::comparison::tests::persisted_group_similarity -- --nocapture`

Expected: FAIL because the persistent SSIM helpers do not exist.

- [x] **Step 3: Add read-through persistent SSIM caching**

The read-through key includes run ID, algorithm profile, canonical path, size, nanosecond mtime, dimensions, and pHash. Only successful score sets are persisted. Both image-metrics and difference-finder commands remain outside this path.

- [x] **Step 4: Run focused tests and verify GREEN**

Run the command from Step 2 and expect all focused tests to pass.

### Task 4: Backfill remaining groups at low priority

**Files:**
- Modify: `src-tauri/src/core/algorithm_profile.rs`
- Modify: `src-tauri/src/commands/comparison.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/src/core/algorithm_profile.rs`
- Test: `src-tauri/src/commands/comparison.rs`

- [x] **Step 1: Write failing scheduler tests**

Verify the background pool has one worker and groups are ordered after the foreground group before wrapping to earlier unfinished groups.

- [x] **Step 2: Run focused scheduler tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml background_algorithm_pool -- --nocapture`

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::comparison::tests::background_group_order -- --nocapture`

Expected: FAIL because the pool and ordering helper are absent.

- [x] **Step 3: Implement the deduplicated background command**

`start_group_similarity_backfill` maintains one active job per run/distance, processes groups sequentially using a dedicated one-worker Rayon pool, reuses persistent snapshots, and releases the repository mutex during image decoding and SSIM work.

- [x] **Step 4: Precompute the default grouping after new runs**

After a successful formal workflow, build and store grouping distance 10 before emitting completion. Old histories build once on first post-upgrade access and then reuse the snapshot.

- [x] **Step 5: Run scheduler tests and verify GREEN**

Run the focused tests from Step 2 and expect them to pass.

### Task 5: Trigger backfill from group review

**Files:**
- Modify: `src/api/comparison.ts`
- Modify: `src/components/ComparisonGroupDetail.vue`
- Modify: `src/components/ComparisonGroupDetail.spec.ts`
- Modify: `src/stores/comparisonStore.ts`

- [x] **Step 1: Write a failing component test**

Assert successful foreground loading invokes `startGroupSimilarityBackfill(runId, appliedGroupingDistance, sourceGroupIndices)`. Repeated review goes through the backend so current file fingerprints are always revalidated before a persistent-cache hit is returned.

- [x] **Step 2: Run the component test and verify RED**

Run: `npm test -- src/components/ComparisonGroupDetail.spec.ts`

Expected: FAIL because the API and trigger are absent.

- [x] **Step 3: Add the API wrapper and non-blocking trigger**

```ts
export function startGroupSimilarityBackfill(runId: string, groupingDistance: number, afterGroupIndices: number[]) {
  return invoke<void>('start_group_similarity_backfill', { runId, groupingDistance, afterGroupIndices })
}
```

Invoke it only after current group scores are available; backend deduplication makes repeated group visits harmless.

The component passes original source-group indices so manually merged display groups do not confuse backend ordering. A successful-grouping revision triggers the new distance only after fresh groups arrive; failed refreshes keep the previous group and do not start a mismatched backfill.

- [x] **Step 4: Run the component test and verify GREEN**

Run the command from Step 2 and expect all component tests to pass.

### Task 6: Verify the complete change

**Files:**
- Modify: `README.md`

- [x] **Step 1: Document formal-task cache semantics**

State that formal history persists grouping and group cross-check results, while both temporary tools always compute current files in real time.

- [x] **Step 2: Run format and complete test suites**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Run: `npm test`

Run: `npm run build`

Expected: all commands exit 0 with no failed tests.

- [x] **Step 3: Inspect the final diff and requirement coverage**

Confirm no temporary-tool API calls reference the new cache tables, cache keys include algorithm and file fingerprints, history grouping is read-through persistent, and the backfill begins only after foreground group loading.
