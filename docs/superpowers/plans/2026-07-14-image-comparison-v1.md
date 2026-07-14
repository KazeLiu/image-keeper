# ImageKeeper v1 Image Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the placeholder ImageKeeper workflow with the complete, fail-closed baseline-to-comparison analysis, review, recycle, recovery, and reporting product defined by `IMAGE_COMPARISON_WORKFLOW.md`.

**Architecture:** Keep the Tauri 2 and Vue 3 application shell, but replace the legacy scan/delete model with a run-scoped Rust domain core and a versioned SQLite schema. Tauri commands expose DTOs over services; every filesystem action is revalidated and journaled, while the Vue workbench consumes only those safe commands.

**Tech Stack:** Rust 2021, Tauri 2, rusqlite, image/image hashing/SSIM libraries, Tokio, Vue 3, TypeScript, Pinia, Element Plus, Vitest, Vue Test Utils.

---

## File Structure

### Backend

- Create `src-tauri/src/lib.rs`: application construction and exported modules for integration tests.
- Replace `src-tauri/src/main.rs`: thin desktop entry point.
- Replace `src-tauri/src/error.rs`: stable, privacy-safe application error codes.
- Replace `src-tauri/src/db/mod.rs`: connection factory, pragmas, and migration entry point.
- Create `src-tauri/src/db/migrations.rs`: schema v1 migration and legacy archival.
- Replace `src-tauri/src/db/models.rs`: persistence records separated by responsibility.
- Replace `src-tauri/src/db/repository.rs`: run-scoped transactional queries.
- Create `src-tauri/src/domain/mod.rs`: domain exports.
- Create `src-tauri/src/domain/types.rs`: enums and DTO-safe value types.
- Create `src-tauri/src/domain/profile.rs`: immutable algorithm/normalization profiles.
- Create `src-tauri/src/services/mod.rs`: service exports.
- Create `src-tauri/src/services/preflight.rs`: root canonicalization and safety checks.
- Create `src-tauri/src/services/manifest.rs`: exclusion-aware discovery and metadata extraction.
- Create `src-tauri/src/services/features.rs`: BLAKE3, pHash, decode policy, and cache validation.
- Create `src-tauri/src/services/index.rs`: exact hash map and pHash BK-tree neighbor lookup.
- Replace `src-tauri/src/core/ssim/compute.rs`: real windowed SSIM.
- Replace `src-tauri/src/core/ssim/resize.rs`: versioned downscale-only normalization.
- Create `src-tauri/src/services/analysis.rs`: pair evidence, stable ordering, and eight-way arbitration.
- Create `src-tauri/src/services/control.rs`: real pause/resume/cancel checkpoints.
- Create `src-tauri/src/services/runner.rs`: phase orchestration and progress events.
- Create `src-tauri/src/services/review.rs`: review transition rules and audit events.
- Replace `src-tauri/src/core/delete/mod.rs`: revalidation and journaled recycle lifecycle.
- Replace `src-tauri/src/core/delete/recycle.rs`: startup reconciliation.
- Replace `src-tauri/src/core/delete/export.rs`: atomic JSON/CSV/HTML reports.
- Replace `src-tauri/src/commands/*.rs`: run, query, review, action, report, and settings command adapters.
- Create `src-tauri/tests/fixtures.rs`: deterministic synthetic image and filesystem fixtures.
- Create `src-tauri/tests/workflow.rs`: end-to-end analysis tests.
- Create `src-tauri/tests/safety.rs`: path and action safety tests.
- Create `src-tauri/tests/reports.rs`: report isolation, privacy, and conservation tests.

### Frontend

- Update `package.json` and `vite.config.ts`: compatible Vue tooling, Vitest, jsdom, and test scripts.
- Replace `src/types/index.ts`: command DTOs and separated analysis/review/action states.
- Create `src/api/imageKeeper.ts`: typed Tauri command boundary.
- Replace `src/stores/scanStore.ts`: runs, progress, controls, and polling/events.
- Replace `src/stores/imageStore.ts`: run-scoped result filters and safe selections.
- Replace `src/stores/deleteStore.ts`: recycle entries and destructive confirmations.
- Replace `src/stores/settingsStore.ts`: future-run defaults only.
- Replace `src/App.vue` and `src/router/index.ts`: persistent workbench navigation.
- Create `src/components/AppSidebar.vue`: icon navigation and active-run identity.
- Create `src/components/RootSelector.vue`: explicit baseline/comparison/report roles.
- Create `src/components/RunProgress.vue`: stable phases and real controls.
- Create `src/components/ResultTable.vue`: virtualizable, filterable result table.
- Create `src/components/ComparisonViewer.vue`: side-by-side source/baseline candidate inspection.
- Create `src/components/ReviewActions.vue`: allowed keep/recycle decisions.
- Create `src/components/BatchApprovalDialog.vue`: grouped, comparison-only confirmation.
- Replace/create `src/views/RunsView.vue`, `ReviewView.vue`, `RecycleView.vue`, `ReportsView.vue`, and `SettingsView.vue`.
- Replace `src/style.css`: responsive, accessible workbench tokens and layout.
- Create `src/test/setup.ts`, `src/test/fakes.ts`, and focused `*.spec.ts` tests next to stores/components.

### Documentation

- Update `README.md`: actual workflow, commands, privacy, and development steps.
- Create `docs/COMPLIANCE_MATRIX.md`: map every target checklist item to implementation and verification evidence.

## Task 1: Restore a Trustworthy Build and Test Harness

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `vite.config.ts`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src/test/setup.ts`

- [ ] **Step 1: Add a failing frontend smoke test**

Create `src/App.spec.ts` that mounts `App` with a memory router and asserts that the workbench landmark and navigation are present:

```ts
it('renders the ImageKeeper workbench shell', async () => {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/', component: { template: '<div />' } }]
  })
  await router.push('/')
  await router.isReady()
  const wrapper = mount(App, { global: { plugins: [router] } })
  expect(wrapper.get('[data-testid="app-shell"]').exists()).toBe(true)
  expect(wrapper.text()).toContain('ImageKeeper')
})
```

- [ ] **Step 2: Verify the frontend test/build baseline fails for the recorded reason**

Run: `npm test -- --run src/App.spec.ts`

Expected: FAIL because no test script/dependencies and no `data-testid="app-shell"` exist. Also retain the recorded `npm run build` failure from `vue-tsc@1.8` under Node 22.

- [ ] **Step 3: Upgrade only the compatible toolchain and add scripts**

Set scripts to `"test": "vitest"`, `"test:run": "vitest run"`, and keep `build` as `vue-tsc --noEmit && vite build`. Upgrade Vue, Vue Router, Pinia, Vite, Vue plugin, TypeScript, and `vue-tsc` to mutually compatible current major versions; add `vitest`, `@vue/test-utils`, `jsdom`, and `@pinia/testing`. Add Rust dependencies for UUID run IDs, EXIF orientation, proven perceptual hashing, CSV output, image comparison, structured tracing, and temporary test directories after confirming their `image` crate compatibility with `cargo tree -d`.

- [ ] **Step 4: Export the Rust application as a library**

Move module declarations and a `pub fn build_app()` constructor into `lib.rs`; keep `main.rs` as:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    imagekeeper_lib::run();
}
```

This makes integration tests compile the same services used by Tauri.

- [ ] **Step 5: Run the restored baselines**

Run: `npm install`, `npm run build`, `cargo check`, and `cargo test --no-run`.

Expected: dependency installation succeeds; web build and Rust compilation reach project code without the old vue-tsc crash or missing-field test compile error.

- [ ] **Step 6: Commit**

Run: `git add package.json package-lock.json vite.config.ts src/test src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/main.rs && git commit -m "chore: restore project test harness"`.

## Task 2: Define Fail-Closed Domain Types and Immutable Profiles

**Files:**
- Create: `src-tauri/src/domain/mod.rs`
- Create: `src-tauri/src/domain/types.rs`
- Create: `src-tauri/src/domain/profile.rs`
- Replace: `src-tauri/src/error.rs`
- Test: unit tests inside the new modules

- [ ] **Step 1: Write failing enum and transition tests**

Cover all exact serialized values and reject unknown input. The core transition assertions are:

```rust
assert!(ReviewStatus::NotRequired.can_transition_to(ReviewStatus::ApprovedForRecycle, AnalysisType::ExactDuplicate));
assert!(!ReviewStatus::Pending.can_transition_to(ReviewStatus::ApprovedForRecycle, AnalysisType::Error));
assert!(!AnalysisType::Inconclusive.is_bulk_approvable());
assert_eq!(ActionStatus::None.as_str(), "none");
assert_eq!(RunStatus::ReviewPending.as_str(), "review_pending");
```

Also deserialize every run state from the target specification and assert unknown strings return an error instead of a fallback.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test domain:: -- --nocapture`.

Expected: FAIL because the domain modules and state types do not exist.

- [ ] **Step 3: Implement the domain model**

Define `RootRole`, eight-value `AnalysisType`, four-value `ReviewStatus`, ten-value `ActionStatus`, the full run lifecycle, file stage outcomes, operation kinds, and retryability. Use `serde(rename_all = "snake_case")`, explicit `TryFrom<&str>`, and exhaustive matches. `AnalysisType::is_executable()` must return false for inconclusive, not evaluated, error, and no-baseline-match; final authorization remains in review service.

- [ ] **Step 4: Implement immutable profiles**

Define `AlgorithmProfile::v1()` with hash `blake3`, pHash max distance `10` inclusive, Top-K `20`, compressed SSIM `0.995`, variant floor `0.75`, aspect tolerance `0.005`, tie delta `0.001`, normalization version `1`, Lanczos3 downscale, sRGB RGBA compositing, and first-frame animation policy. Serialize the complete profile into every run.

- [ ] **Step 5: Verify GREEN and formatting**

Run: `cargo fmt --check && cargo test domain::`.

Expected: all domain tests pass and unknown states fail closed.

- [ ] **Step 6: Commit**

Run: `git add src-tauri/src/domain src-tauri/src/error.rs && git commit -m "feat: define safe workflow domain model"`.

## Task 3: Replace the Legacy Schema with Versioned Run-Scoped Persistence

**Files:**
- Replace: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/migrations.rs`
- Replace: `src-tauri/src/db/models.rs`
- Replace: `src-tauri/src/db/repository.rs`
- Test: `src-tauri/tests/database.rs`

- [ ] **Step 1: Write failing migration tests**

Create an in-memory legacy schema, insert legacy settings and a pending duplicate, run migration, then assert:

```rust
assert_eq!(user_version(&conn), 1);
assert!(table_exists(&conn, "runs"));
assert!(table_exists(&conn, "manifest_entries"));
assert!(table_exists(&conn, "action_journal"));
assert_eq!(count_actionable_legacy_rows(&conn), 0);
assert_eq!(load_default_profile(&conn)?.compressed_ssim_threshold, 0.995);
```

Add foreign-key, unique `(run_id, root_id, relative_path)`, unique result-per-manifest, and run-scoped query tests.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test database`.

Expected: FAIL because the migration API and v1 tables do not exist.

- [ ] **Step 3: Implement migration v1**

Create the nine tables from the design with checks for every enum value, foreign keys, run-scoped indexes, uniqueness for idempotency, and immutable profile JSON. Rename legacy tables with a `legacy_` prefix inside a transaction, migrate only safe user defaults, force automatic recycle defaults off, set `PRAGMA user_version = 1`, and run `PRAGMA foreign_key_check` before commit.

- [ ] **Step 4: Implement repository transactions**

Expose focused methods: `create_run`, `insert_roots`, `upsert_manifest`, `save_candidate_batch`, `replace_analysis_result`, `append_review_event`, `prepare_action`, `finish_action`, `list_results(run_id, filter)`, `summary(run_id)`, and recycle/report queries that always require `run_id`. Do not expose a generic public connection from application services.

- [ ] **Step 5: Verify migration idempotency and constraints**

Run: `cargo test --test database && cargo test db::`.

Expected: migrating twice is a no-op; invalid enum strings, cross-run foreign keys, and duplicate result rows are rejected.

- [ ] **Step 6: Commit**

Run: `git add src-tauri/src/db src-tauri/tests/database.rs && git commit -m "feat: add run scoped workflow database"`.

## Task 4: Implement Directory Preflight and Manifest Discovery

**Files:**
- Create: `src-tauri/src/services/mod.rs`
- Create: `src-tauri/src/services/preflight.rs`
- Create: `src-tauri/src/services/manifest.rs`
- Test: `src-tauri/tests/preflight.rs`
- Test: `src-tauri/tests/manifest.rs`

- [ ] **Step 1: Write failing path safety tests**

Use temporary roots to test a valid A+B+C layout and reject: missing baseline, no comparison, same path, baseline nested in comparison, comparison nested in baseline, nested comparisons, duplicate canonical paths, report root inside a scanned tree without exclusion, and directory symlink aliases. Assert errors contain stable codes but no full absolute path.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test preflight`.

Expected: FAIL because `PreflightService` does not exist.

- [ ] **Step 3: Implement preflight**

Canonicalize every existing root, normalize Windows path comparisons case-insensitively, reject equality/nesting after real-path resolution, validate readability, probe report writability with a create-new temporary file, and verify each comparison root can host `.recycle`. Return explicit root records and exclusions; never infer roles from order.

- [ ] **Step 4: Write failing manifest tests**

Create supported images, a corrupt `.png`, text files, `.recycle/<run>/image.png`, report/cache/temp directories, and a directory symlink. Assert supported files become entries, corrupt images get a decode error entry, unsupported files are ignored, excluded folders never appear, GIF frame policy is `first_frame`, and links are not followed.

- [ ] **Step 5: Implement manifest discovery**

Use `walkdir` with `follow_links(false)` and `filter_entry` for permanent exclusions. Persist filesystem metadata and relative paths before decode. Keep per-file `discovery`, `decode`, `hash`, and `feature` outcomes separate; continue after individual errors.

- [ ] **Step 6: Verify and commit**

Run: `cargo test --test preflight --test manifest`.

Then: `git add src-tauri/src/services src-tauri/tests/preflight.rs src-tauri/tests/manifest.rs && git commit -m "feat: add safe root preflight and manifests"`.

## Task 5: Implement Versioned Features and Non-Cartesian Candidate Indexes

**Files:**
- Create: `src-tauri/src/services/features.rs`
- Create: `src-tauri/src/services/index.rs`
- Replace: `src-tauri/src/core/hash/*`
- Replace: `src-tauri/src/core/phash/*`
- Test: unit tests and `src-tauri/tests/features.rs`

- [ ] **Step 1: Write failing feature tests**

Generate a gradient fixture and assert BLAKE3 stability, a 64-bit pHash, inclusive distance behavior (`10` matches, `11` does not), first-frame GIF behavior, cache hits only when canonical identity + size + mtime + algorithm versions match, and recomputation after any key changes.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test features`.

Expected: FAIL because the versioned feature API and cache identity do not exist.

- [ ] **Step 3: Implement feature extraction**

Stream BLAKE3 from disk. Decode with orientation handling and a fixed first-frame policy, convert to the pHash library's expected image type, and store the 64-bit hash as an unsigned integer/16-character hex value with version. Return typed per-stage errors without absolute paths in display messages.

- [ ] **Step 4: Write failing index tests**

Insert known 64-bit hashes into the baseline index and assert exact lookup returns all stable-sorted baseline IDs, neighbor search returns only distance `<= max_distance`, stable distance/path sorting, correct Top-K truncation metadata, and query visit count below all-node traversal for a separated synthetic corpus.

- [ ] **Step 5: Implement indexes**

Build `HashMap<[u8; 32], Vec<BaselineRef>>` for exact matching and a BK-tree keyed by pHash/Hamming distance for neighbor lookup. Keep all exact matches; return `CandidateSearch { candidates, truncated, complete }` for approximate matches.

- [ ] **Step 6: Verify and commit**

Run: `cargo test --test features services::index core::hash core::phash`.

Then: `git add src-tauri/src/services/features.rs src-tauri/src/services/index.rs src-tauri/src/core/hash src-tauri/src/core/phash src-tauri/tests/features.rs && git commit -m "feat: add versioned image features and indexes"`.

## Task 6: Replace MSE Mapping with Real Normalization and SSIM

**Files:**
- Replace: `src-tauri/src/core/ssim/compute.rs`
- Replace: `src-tauri/src/core/ssim/resize.rs`
- Replace: `src-tauri/src/core/ssim/mod.rs`
- Test: `src-tauri/tests/similarity.rs`

- [ ] **Step 1: Write failing algorithm tests**

Use deterministic synthetic images and assert: identical images score `1.0` within epsilon; luminance and local-structure changes produce expected ordering; a larger image is downscaled to the smaller dimensions; a smaller image is never enlarged; aspect error exactly `0.005` is comparable and the next representable higher value is not; alpha compositing and EXIF orientation follow profile v1.

- [ ] **Step 2: Prove the old implementation is invalid**

Run: `cargo test --test similarity`.

Expected: FAIL because the old implementation computes `1 / (1 + MSE / 1000)` and lacks versioned normalization.

- [ ] **Step 3: Implement normalization**

Apply orientation, sRGB RGBA conversion, fixed alpha background, first-frame selection, and Lanczos3 downscale of only the higher-resolution side. Return `NotComparable` for aspect mismatch or semantic frame/alpha conflicts that the profile cannot safely align.

- [ ] **Step 4: Implement real SSIM**

Use a proven SSIM implementation where compatible; otherwise implement the standard 11x11 Gaussian-window formula with constants derived from 8-bit dynamic range, validated against a public reference vector. Never name an MSE transform SSIM. Persist normalization and SSIM versions with evidence.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --test similarity core::ssim`.

Then: `git add src-tauri/src/core/ssim src-tauri/tests/similarity.rs && git commit -m "feat: implement versioned structural similarity"`.

## Task 7: Implement Deterministic Eight-Way Analysis

**Files:**
- Create: `src-tauri/src/services/analysis.rs`
- Test: `src-tauri/tests/analysis.rs`
- Test: `src-tauri/tests/workflow.rs`

- [ ] **Step 1: Write failing arbitration table tests**

Create table-driven cases for exact duplicate, every directional likely-compressed condition and its negation, same-resolution re-encode, variant, similar keep, no candidate, all scores below floor, truncated search, close-score tie at `0.001`, conflicting candidate classes, not evaluated, and error. Assert one and only one result per comparison manifest entry.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test analysis`.

Expected: FAIL because `AnalysisService` does not exist.

- [ ] **Step 3: Implement evidence classification**

Calculate relative aspect difference exactly as specified. `likely_compressed` requires source width, height, pixels, and file size all strictly lower, aspect `<= 0.005`, and SSIM `>= 0.995`. Same-size images can only be variant, similar keep, or inconclusive. Candidate result types must include every measured field and failure reason.

- [ ] **Step 4: Implement stable multi-candidate arbitration**

Sort by comparable first, descending SSIM, ascending pHash distance, descending baseline pixel count, and baseline relative path. Exact hashes bypass approximate scores but retain all matching evidence. Mark incomplete/truncated/conflicting/tied outcomes inconclusive. Never use database or traversal order.

- [ ] **Step 5: Add end-to-end A+B+C workflow tests**

Assert B and C each compare only to A; no B-to-C or within-root evidence exists. Include all eight categories and verify:

```rust
assert_eq!(summary.comparison_total, summary.analysis_total());
assert_eq!(summary.baseline_total, baseline_manifest_count);
```

- [ ] **Step 6: Verify and commit**

Run: `cargo test --test analysis --test workflow`.

Then: `git add src-tauri/src/services/analysis.rs src-tauri/tests/analysis.rs src-tauri/tests/workflow.rs && git commit -m "feat: classify comparison results conservatively"`.

## Task 8: Implement the Real Run Worker and Command Boundary

**Files:**
- Create: `src-tauri/src/services/control.rs`
- Create: `src-tauri/src/services/runner.rs`
- Replace: `src-tauri/src/commands/scan.rs`
- Replace: `src-tauri/src/commands/comparison.rs`
- Replace: `src-tauri/src/commands/directory.rs`
- Replace: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/runner.rs`

- [ ] **Step 1: Write failing lifecycle tests**

Run the worker against a fixture corpus and assert every stable phase transition. Pause during indexing and prove progress count does not change during a bounded observation window; resume and prove it continues; cancel and prove no later phase executes. Reissuing commands must be idempotent and preserve one result per manifest.

Inject a persistence failure and a simulated insufficient-space result before a feature batch. Assert the worker stops new feature/action work, persists its checkpoint, reports a run-level retryable error, and never reclassifies pending files as `no_baseline_match`.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test runner`.

Expected: FAIL because old pause/resume/cancel only mutate database status.

- [ ] **Step 3: Implement cooperative controls**

Store one `RunControl` per run with atomic requested state plus a condvar/notify for pause. Check controls before and after each bounded file/candidate batch. Persist checkpoints before entering pause/canceled states and restore from the last idempotent stage.

- [ ] **Step 4: Implement runner orchestration**

Open worker-local database connections. Execute preflight, indexing, matching, scoring, resolving, review-pending, report generation, and completion transitions. Use bounded channels for parallel feature/score results and a single persistence consumer. Emit events with run ID, stable phase ID, completed/total counts, root alias, and privacy-safe relative path.

- [ ] **Step 5: Replace Tauri commands**

Expose `preflight_run`, `start_run`, `list_runs`, `get_run`, `pause_run`, `resume_run`, `cancel_run`, `get_run_summary`, `list_results`, and `get_result_detail`. Remove `within` mode and old single-directory scan commands from `generate_handler!`.

- [ ] **Step 6: Verify and commit**

Run: `cargo test --test runner --test workflow && cargo check`.

Then: `git add src-tauri/src/services/control.rs src-tauri/src/services/runner.rs src-tauri/src/commands src-tauri/src/lib.rs src-tauri/tests/runner.rs && git commit -m "feat: run comparison pipeline with real controls"`.

## Task 9: Implement Review Decisions and Safe Batch Authorization

**Files:**
- Create: `src-tauri/src/services/review.rs`
- Create: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Test: `src-tauri/tests/review.rs`

- [ ] **Step 1: Write failing review tests**

Assert exact duplicates may be explicitly approved, likely compressed starts pending, inconclusive requires an individual evidence view and cannot use batch API, error/not-evaluated/stale cannot be approved, rejected keep records reason/note/time, and repeated identical requests do not duplicate current state or events.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test review`.

Expected: FAIL because no independent review service exists.

- [ ] **Step 3: Implement review service and commands**

Implement `review_result`, `batch_review_exact_duplicates`, and `review_history`. Validate run/result identity, analysis eligibility, freshness, and transition rules in the service transaction. Persist actor `local_user`, timestamp, required reason code, and optional note.

- [ ] **Step 4: Verify and commit**

Run: `cargo test --test review`.

Then: `git add src-tauri/src/services/review.rs src-tauri/src/commands/review.rs src-tauri/src/commands/mod.rs src-tauri/tests/review.rs && git commit -m "feat: add explicit review decisions"`.

## Task 10: Implement Journaled Recycle, Restore, Delete, and Reconciliation

**Files:**
- Replace: `src-tauri/src/core/delete/mod.rs`
- Replace: `src-tauri/src/core/delete/recycle.rs`
- Create: `src-tauri/src/commands/actions.rs`
- Test: `src-tauri/tests/safety.rs`
- Test: `src-tauri/tests/reconciliation.rs`

- [ ] **Step 1: Write failing action safety tests**

Cover baseline target rejection, non-approved results, source canonical escape, changed source size/mtime/hash, missing or changed baseline evidence, collision-safe recycle targets, no overwrite, destination hash verification, and `.recycle/<runId>` isolation. Every failure must leave the source in place and set stale/failed without a recycled event.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test safety`.

Expected: FAIL because the old delete manager moves directly and does not revalidate run evidence.

- [ ] **Step 3: Implement prepared-journal recycling**

For each approved result: transition to validating; canonical-boundary check source and baseline; restat and BLAKE3 both sides; choose a create-new collision-free target; insert prepared journal; create parents; move; verify target; persist recycled event and entry. On uncertainty set reconciliation-required instead of guessing.

- [ ] **Step 4: Write failing restore/delete/reconcile tests**

Assert restore refuses an occupied origin, supports an explicitly supplied alternative inside the comparison root, records restored state, and is idempotent. Permanent delete must accept only recorded recycled entries, require an audit-export acknowledgement token and exact-count confirmation, remain inside recycle root, and record permanently-deleted state. Simulate prepared journal states with source-only, target-only, both, and neither.

- [ ] **Step 5: Implement restore, permanent delete, and startup reconciliation**

Resolve source-only as not moved/failed, target-only as moved then verify and finalize, both as reconciliation-required, and neither as reconciliation-required. Never overwrite or silently delete. Register reconciliation during app startup and expose unresolved entries to the UI.

- [ ] **Step 6: Verify and commit**

Run: `cargo test --test safety --test reconciliation`.

Then: `git add src-tauri/src/core/delete src-tauri/src/commands/actions.rs src-tauri/tests/safety.rs src-tauri/tests/reconciliation.rs && git commit -m "feat: add journaled recycle lifecycle"`.

## Task 11: Implement Atomic Run-Scoped Reports

**Files:**
- Replace: `src-tauri/src/core/delete/export.rs`
- Create: `src-tauri/src/commands/reports.rs`
- Test: `src-tauri/tests/reports.rs`

- [ ] **Step 1: Write failing report tests**

Create two runs and assert each JSON/CSV/HTML export contains only its run. Verify JSON schema/profile/evidence/review/action fields, CSV run/analysis/review/action columns, HTML no embedded images or absolute paths, filename includes run ID, existing files are not overwritten, and temp files are removed after successful atomic rename.

- [ ] **Step 2: Run and confirm RED**

Run: `cargo test --test reports`.

Expected: FAIL because old CSV queries the entire recycle table and JSON/HTML do not exist.

- [ ] **Step 3: Implement canonical JSON and derived exports**

Build one run-scoped report model, validate the conservation equation before writing, serialize JSON, derive CSV and escaped HTML from the same model, write with create-new temp files, flush/sync, validate, and rename to a non-existing final path. Report aliases/relative paths and shortened fingerprints by default.

- [ ] **Step 4: Gate run completion on report success**

Analysis data may remain after report failure, but the run becomes `completed_with_errors` and stores a retryable report error instead of `analysis_complete`/`action_complete` delivery.

- [ ] **Step 5: Verify and commit**

Run: `cargo test --test reports --test workflow`.

Then: `git add src-tauri/src/core/delete/export.rs src-tauri/src/commands/reports.rs src-tauri/tests/reports.rs && git commit -m "feat: export private run scoped reports"`.

## Task 12: Build the Typed Frontend Data Layer

**Files:**
- Replace: `src/types/index.ts`
- Create: `src/api/imageKeeper.ts`
- Replace: `src/stores/scanStore.ts`
- Replace: `src/stores/imageStore.ts`
- Replace: `src/stores/deleteStore.ts`
- Replace: `src/stores/settingsStore.ts`
- Test: adjacent `*.spec.ts` files

- [ ] **Step 1: Write failing API/store tests**

Stub only the Tauri invoke boundary. Assert command payload names match Rust DTOs, every query carries run ID, progress events ignore other runs, summary conservation is checked, unsafe result types cannot enter batch selection, settings edits affect only future-run defaults, and command errors surface stable codes.

- [ ] **Step 2: Run and confirm RED**

Run: `npm run test:run -- src/api src/stores`.

Expected: FAIL because the typed API and v1 stores do not exist.

- [ ] **Step 3: Implement exact TypeScript DTOs and API wrapper**

Mirror Rust snake/camel serialization deliberately in one boundary module. Provide typed methods for all run, result, review, action, recycle, report, and settings commands. Do not call `invoke` directly from components or stores.

- [ ] **Step 4: Implement stores**

Keep active run identity in the run store, normalized result pages/filters in image store, and recycle lifecycle in delete store. Derive selection eligibility from analysis/review/action state, clear stale selections after refresh, and expose loading/error/empty states for every async operation.

- [ ] **Step 5: Verify and commit**

Run: `npm run test:run -- src/api src/stores && npm run build`.

Then: `git add src/types src/api src/stores && git commit -m "feat: connect typed frontend workflow state"`.

## Task 13: Build the Desktop Workbench Shell and Run Setup

**Files:**
- Replace: `src/App.vue`
- Replace: `src/router/index.ts`
- Replace: `src/style.css`
- Create: `src/components/AppSidebar.vue`
- Create: `src/components/RootSelector.vue`
- Create: `src/components/RunProgress.vue`
- Replace: `src/views/MainView.vue` with `src/views/RunsView.vue`
- Test: component/view specs

- [ ] **Step 1: Apply the UI design skill before visual implementation**

Read `ui-ux-pro-max/SKILL.md`, use its accessibility and responsive guidance, and keep the approved quiet operational workbench direction. Record any material design choice in the implementation notes, without changing product semantics.

- [ ] **Step 2: Write failing shell/setup tests**

Assert persistent icon navigation, protected baseline label, dynamic comparison root add/remove, minimum one comparison, report selector, inline preflight errors, start disabled until valid, stable progress phase IDs, privacy-safe current path, and real pause/resume/cancel controls.

- [ ] **Step 3: Run and confirm RED**

Run: `npm run test:run -- src/App.spec.ts src/components/RootSelector.spec.ts src/views/RunsView.spec.ts`.

Expected: FAIL because the workbench components do not exist.

- [ ] **Step 4: Implement responsive workbench setup/progress**

Use a fixed navigation rail on desktop and compact navigation on narrow windows, full-width unframed page sections, 8px-or-less radii, Element Plus/Lucide-equivalent existing icon buttons with tooltips, explicit A/B/C role chips, and stable responsive grid constraints. Avoid nested cards and decorative gradients.

- [ ] **Step 5: Verify and commit**

Run: `npm run test:run -- src/App.spec.ts src/components/RootSelector.spec.ts src/views/RunsView.spec.ts && npm run build`.

Then: `git add src/App.vue src/router src/style.css src/components/AppSidebar.vue src/components/RootSelector.vue src/components/RunProgress.vue src/views && git commit -m "feat: build comparison run workbench"`.

## Task 14: Build Review, Recycle, Reports, and Settings Views

**Files:**
- Create: `src/components/ResultTable.vue`
- Create: `src/components/ComparisonViewer.vue`
- Create: `src/components/ReviewActions.vue`
- Create: `src/components/BatchApprovalDialog.vue`
- Create: `src/views/ReviewView.vue`
- Create: `src/views/RecycleView.vue`
- Create: `src/views/ReportsView.vue`
- Replace: `src/views/SettingsView.vue`
- Remove obsolete placeholder views/components after routes no longer use them
- Test: adjacent specs

- [ ] **Step 1: Write failing review UI tests**

Assert filters cover all eight categories, A and source are side-by-side, all candidates are selectable, evidence shows resolution/format/size/short hashes/pHash/SSIM/normalization, baseline never has a delete checkbox, likely compressed is not preselected, unsafe categories cannot batch approve, and grouped confirmation explicitly says only comparison roots are affected.

- [ ] **Step 2: Implement review UI**

Use a dense result table plus an unframed comparison workspace with stable image panes, zoom, overlay/difference modes, candidate strip, evidence panel, reason selector, note, keep, and approve commands. Provide keyboard-independent controls, focus states, tooltips, loading/error/empty/stale states, and no image/log uploads.

- [ ] **Step 3: Write failing recycle/report/settings tests**

Assert restore conflict choices never overwrite, permanent delete is a separate page flow with exact count and second confirmation, unresolved reconcile entries are visible, exports are per run/format, settings state says it applies to future runs, and no automatic recycle/delete toggles exist.

- [ ] **Step 4: Implement recycle/report/settings views**

Add run filters, audit status, restore destination handling, report export history/actions, and bounded numeric controls for next-run thresholds. Keep permanent delete disabled until audit export acknowledgement and exact confirmation are complete.

- [ ] **Step 5: Verify and commit**

Run: `npm run test:run && npm run build`.

Then: `git add src/components src/views src/router && git commit -m "feat: complete review and recycle workflow"`.

## Task 15: Documentation, Visual QA, and Requirement Audit

**Files:**
- Modify: `README.md`
- Create: `docs/COMPLIANCE_MATRIX.md`
- Modify: `.gitignore`
- Update tests/fixtures only when audit evidence identifies a gap

- [ ] **Step 1: Write the compliance matrix from the target checklist**

For every checkbox in section 10 and every test class in section 9, record implementation file, exact automated test, manual evidence where required, and status. No row may rely only on “build passes.”

- [ ] **Step 2: Update documentation and ignore rules**

Document the A-to-comparison model, supported formats, local-only privacy behavior, run lifecycle, review/recycle safety, report formats, development/test commands, and known v1 algorithm limits. Ignore private fixtures, generated reports, local databases/caches, recycle content, screenshots, and temporary output.

- [ ] **Step 3: Verify log privacy and performance evidence**

Run integration tests with captured tracing output and assert no pixel data, thumbnail data URLs, complete BLAKE3 values, or unapproved absolute paths occur. Add an ignored benchmark harness that reports cold/warm cache separately with format mix, mean resolution, storage location, thread count, candidate counts, truncation counts, and SSIM duration; run it on the synthetic corpus and record results without making a per-image product promise.

- [ ] **Step 4: Run fresh backend verification**

Run: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`, and `cargo build --release` from `src-tauri`.

Expected: zero failures and zero warnings.

- [ ] **Step 5: Run fresh frontend verification**

Run: `npm run test:run` and `npm run build`.

Expected: zero test failures, successful strict TypeScript check, and successful Vite production build.

- [ ] **Step 6: Start the app and perform visual/runtime QA**

Start `npm run tauri:dev` on an available port. Use Playwright/browser tooling where the Tauri webview can be exercised, or the Vite web shell with a deterministic fake API mode for visual-only QA. Capture desktop and narrow screenshots; verify no blank views, text overflow, overlapping controls, inaccessible focus states, or unintended baseline actions. Exercise demo fixtures through run creation, review, recycle, restore, and report export without permanently deleting source fixtures.

- [ ] **Step 7: Audit every target requirement against current evidence**

Re-read `IMAGE_COMPARISON_WORKFLOW.md`, resolve every missing/weak matrix row, scan for old placeholder/auto-recycle/within-mode code, and rerun the affected full verification command. Completion is allowed only when every required row is proven.

- [ ] **Step 8: Commit final documentation and audit fixes**

Run: `git add README.md docs/COMPLIANCE_MATRIX.md .gitignore src src-tauri package.json package-lock.json && git commit -m "docs: document verified image comparison workflow"`.

## Execution Notes

- Keep this checklist current by marking each completed step immediately.
- Follow TDD for every behavior change: record RED output before production edits, then GREEN output after.
- Preserve unrelated user changes if they appear; never reset the worktree.
- Never test permanent deletion against user-owned or demo source files. Use isolated temporary fixtures only.
- Do not mark the goal complete until Task 15's compliance audit proves every target requirement.
