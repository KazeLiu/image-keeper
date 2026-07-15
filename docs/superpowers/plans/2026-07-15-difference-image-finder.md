Exit code: 0
Wall time: 0.4 seconds
Output:
# Difference Image Finder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an independent multi-reference “找差分图” window that scans target folders once, aggregates reference-centered matches, previews safe batch rename rules, and performs rename/move/copy operations without silent overwrite.

**Architecture:** Add a transient Rust search service with reusable image-feature extraction and direct reference-to-candidate scoring. Expose search, rename preview, rename/move/copy, cancel, and undo through focused Tauri commands; keep UI session state in a dedicated Pinia store and open the tool through a separate `/difference-finder` webview window.

**Tech Stack:** Rust 2021, Tauri 2, `image`, `blake3`, `rayon`, Vue 3, TypeScript, Pinia, Element Plus, native HTML drag-and-drop.

---

## File map

- Create `src-tauri/src/core/image_features.rs`: reusable BLAKE3, metadata, and hexadecimal pHash extraction.
- Create `src-tauri/src/core/difference_finder.rs`: target scanning, reference-centered matching, classification, aggregation, cancellation, and progress types.
- Create `src-tauri/src/core/file_operations.rs`: rename grammar, validation, temporary-name execution, move/copy, and undo records.
- Create `src-tauri/src/commands/difference_finder.rs`: Tauri boundary and transient session/undo state.
- Modify `src-tauri/src/core/scanner.rs`: use the shared feature functions instead of private duplicates.
- Modify `src-tauri/src/core/mod.rs`, `src-tauri/src/commands/mod.rs`, and `src-tauri/src/main.rs`: register modules, state, and commands.
- Create `src/api/differenceFinder.ts`: typed Tauri API.
- Create `src/stores/differenceFinderStore.ts`: isolated tool state and preview transformations.
- Create `src/views/DifferenceFinderView.vue`: independent tool shell.
- Create `src/components/difference-finder/ReferenceImageStrip.vue`: reference input/filter.
- Create `src/components/difference-finder/SearchSetupPanel.vue`: target directories and progress.
- Create `src/components/difference-finder/DifferenceResultList.vue`: result filtering and selection.
- Create `src/components/difference-finder/BatchRenameGrid.vue`: drag sorting, templates, help, validation, and file actions.
- Modify `src/router/index.ts`, `src/views/MainView.vue`, and `src-tauri/capabilities/default.json`: route, small entry card, independent window permission.
- Modify `README.md`: document the new small-tool workflow and safety boundary.

### Task 1: Reusable image features

**Files:**
- Create: `src-tauri/src/core/image_features.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/core/scanner.rs`

- [ ] **Step 1: Write feature extraction tests**

Add tests that create two identical PNG files and one visibly different PNG, then assert identical BLAKE3/pHash and a nonzero pHash distance for the different image:

```rust
#[test]
fn extracts_stable_features() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.png");
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(32, 32, image::Rgb([20, 40, 80])))
        .save(&first).unwrap();
    std::fs::copy(&first, &second).unwrap();
    let a = extract_image_features(&first).unwrap();
    let b = extract_image_features(&second).unwrap();
    assert_eq!(a.blake3_hash, b.blake3_hash);
    assert_eq!(a.phash, b.phash);
}
```

- [ ] **Step 2: Run the focused test and confirm failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml image_features -- --nocapture`  
Expected: FAIL because `core::image_features` does not exist.

- [ ] **Step 3: Implement the shared extractor**

Define the stable boundary and move the scanner’s current DCT pHash algorithm into it:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFeatures {
    pub file_path: String,
    pub file_size: u64,
    pub modified_at: i64,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub blake3_hash: String,
    pub phash: String,
}

pub fn phash_distance(left: &str, right: &str) -> Option<u32> {
    Some((u64::from_str_radix(left, 16).ok()? ^ u64::from_str_radix(right, 16).ok()?).count_ones())
}
```

Implement `extract_image_features(path: &Path) -> Result<ImageFeatures>` by reading metadata, decoding with `image::open`, calling a buffered `compute_blake3`, and calling the moved 32×32 DCT `compute_phash`. Update `ScanEngine::scan_file` to call that function and remove its duplicate private BLAKE3/pHash/DCT functions; the moved math must remain byte-for-byte equivalent so existing hashes do not change.

- [ ] **Step 4: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml image_features -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit the unit**

```bash
git add src-tauri/src/core/image_features.rs src-tauri/src/core/mod.rs src-tauri/src/core/scanner.rs
git commit -m "refactor: share image feature extraction"
```

### Task 2: Multi-reference search engine

**Files:**
- Create: `src-tauri/src/core/difference_finder.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write direct-match and aggregation tests**

Use synthetic `SearchImage` values to verify that a candidate directly matching two references produces one aggregate item with two relations, while transitive-only similarity is not inserted:

```rust
#[test]
fn aggregates_same_candidate_once_across_references() {
    let relations = vec![relation("ref-a", "candidate.png", 0.98), relation("ref-b", "candidate.png", 0.91)];
    let items = aggregate_relations(relations);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].relations.len(), 2);
    assert_eq!(items[0].best_reference_id, "ref-a");
}
```

- [ ] **Step 2: Confirm the focused test fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml difference_finder -- --nocapture`  
Expected: FAIL because the module and types do not exist.

- [ ] **Step 3: Implement request, progress, relation, and result types**

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceSearchRequest {
    pub session_id: String,
    pub reference_paths: Vec<String>,
    pub target_roots: Vec<String>,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceMatchItem {
    pub file_path: String,
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub format: String,
    pub classification: MatchClassification,
    pub best_reference_id: String,
    pub relations: Vec<ReferenceRelation>,
}
```

Implement canonical target-root deduplication, recursive supported-image discovery, cancellation checks, parallel target feature extraction, exact-hash detection, pHash candidate recall, normalized score calculation through `SsimComputer::compute_from_files`, conservative classification, and normalized-path aggregation. Emit `DifferenceSearchProgress` after each phase.

- [ ] **Step 4: Add classification boundary tests**

Cover exact hash, compressed/re-encoded, variant, related group, weak candidate, and rejected pHash distance. Assert that every accepted relation was calculated directly against its reference.

- [ ] **Step 5: Run the search-engine tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml difference_finder -- --nocapture`  
Expected: PASS.

- [ ] **Step 6: Commit the unit**

```bash
git add src-tauri/src/core/difference_finder.rs src-tauri/src/core/mod.rs
git commit -m "feat: add multi-reference difference search"
```

### Task 3: Rename grammar and safe file plans

**Files:**
- Create: `src-tauri/src/core/file_operations.rs`
- Modify: `src-tauri/src/core/mod.rs`

- [ ] **Step 1: Write rename grammar tests**

Cover simple variables, zero-padded order, wildcard captures, unmatched rows, and quick rename:

```rust
#[test]
fn renders_simple_and_capture_templates() {
    let ctx = RenameContext::new("表情_角色.png", "三月七", 2, 1);
    assert_eq!(render_template("$ref-$n:02.$ext", &ctx, &[]).unwrap(), "三月七-02.png");
    let captures = capture_wildcards("*_*.png", "表情_角色.png").unwrap();
    assert_eq!(render_template("$2-$1.png", &ctx, &captures).unwrap(), "角色-表情.png");
}

#[test]
fn quick_rename_uses_first_name_and_current_order() {
    let names = quick_rename("三月七.png", &["png", "jpg", "webp"]);
    assert_eq!(names, ["三月七_1.png", "三月七_2.jpg", "三月七_3.webp"]);
}
```

- [ ] **Step 2: Confirm tests fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml file_operations -- --nocapture`  
Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement grammar and validation**

Define `RenameContext`, `RenameRule`, `RenamePreviewItem`, and `FilePlanIssue`. Implement literal-safe `$name`, `$ext`, `$n`, `$n:02`, `$n:03`, `$ref`, `$group`, and `$1`–`$9` replacement; `*` wildcard capture; Windows invalid-name checks; case-insensitive batch duplicate detection; destination existence/hash checks; and “unmatched rule” status.

```rust
pub enum RenameRule {
    Simple { template: String },
    Advanced { old_pattern: String, new_template: String },
    Quick { first_name: String },
}

```

Expose `preview_rename(items: &[RenameInput], rule: &RenameRule) -> Vec<RenamePreviewItem>` as the single deterministic entry point used by tests and commands.

- [ ] **Step 4: Write filesystem execution tests**

Use `tempfile` to verify `a.png ↔ b.png` swapping, a three-file cycle, conflict skip, copy-same-hash skip, move, and undo rename/move.

- [ ] **Step 5: Implement execution and undo**

Use unique same-directory temporary names before final rename. Never overwrite an existing unrelated file. Store `OperationBatch { id, kind, entries }` with pre-operation hash/size/mtime. Undo only when the current path still matches the stored fingerprint. Copy is logged but not auto-undone.

- [ ] **Step 6: Run focused tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml file_operations -- --nocapture`  
Expected: PASS.

- [ ] **Step 7: Commit the unit**

```bash
git add src-tauri/src/core/file_operations.rs src-tauri/src/core/mod.rs
git commit -m "feat: add safe batch file operations"
```

### Task 4: Tauri command boundary and transient state

**Files:**
- Create: `src-tauri/src/commands/difference_finder.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write command-helper state tests**

Test session cancellation and last reversible batch retrieval without constructing a Tauri runtime:

```rust
#[test]
fn cancellation_is_scoped_to_one_session() {
    let state = DifferenceFinderState::default();
    state.cancel("one");
    assert!(state.is_cancelled("one"));
    assert!(!state.is_cancelled("two"));
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::difference_finder -- --nocapture`  
Expected: FAIL because the command module does not exist.

- [ ] **Step 3: Implement and register commands**

Expose:

```rust
start_difference_search(request, window, state) -> Result<DifferenceSearchResponse, String>
cancel_difference_search(session_id, state) -> Result<(), String>
preview_difference_rename(request) -> Result<RenamePreviewResponse, String>
execute_difference_rename(request, state) -> Result<OperationBatchResult, String>
move_difference_files(request, state) -> Result<OperationBatchResult, String>
copy_difference_files(request, state) -> Result<OperationBatchResult, String>
undo_difference_batch(batch_id, state) -> Result<OperationBatchResult, String>
```

Run CPU/filesystem search work in `spawn_blocking`, emit `difference-search-progress` only to the invoking window, and manage `Arc<DifferenceFinderState>` separately from the comparison repository.

- [ ] **Step 4: Run focused and full Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::difference_finder -- --nocapture`  
Expected: PASS.  
Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: all tests PASS.

- [ ] **Step 5: Commit the unit**

```bash
git add src-tauri/src/commands/difference_finder.rs src-tauri/src/commands/mod.rs src-tauri/src/main.rs
git commit -m "feat: expose difference finder commands"
```

### Task 5: Typed frontend API and store

**Files:**
- Create: `src/api/differenceFinder.ts`
- Create: `src/stores/differenceFinderStore.ts`

- [ ] **Step 1: Define API types matching Rust camelCase payloads**

Include `DifferenceReference`, `DifferenceMatchItem`, `ReferenceRelation`, `DifferenceSearchProgress`, `RenameRule`, `RenamePreviewItem`, and operation request/result types. Wrap every command with typed `invoke` functions and progress-listener setup.

```ts
export async function startDifferenceSearch(request: DifferenceSearchRequest) {
  return invoke<DifferenceSearchResponse>('start_difference_search', { request })
}
```

- [ ] **Step 2: Implement isolated Pinia state**

Store references, roots, active reference filter, aggregate matches, selected canonical paths, drag order, rename rule, preview rows, progress, running state, last reversible batch, and recent copy destination. Ensure filtered views never duplicate a canonical path.

- [ ] **Step 3: Run type checking**

Run: `npm run build`  
Expected: PASS or only fail for not-yet-added routed components; API/store code has no TypeScript errors.

- [ ] **Step 4: Commit the unit**

```bash
git add src/api/differenceFinder.ts src/stores/differenceFinderStore.ts
git commit -m "feat: add difference finder frontend state"
```

### Task 6: Independent-window entry and routing

**Files:**
- Modify: `src/views/MainView.vue`
- Modify: `src/router/index.ts`
- Modify: `src-tauri/capabilities/default.json`
- Create: `src/views/DifferenceFinderView.vue`

- [ ] **Step 1: Add the route and minimal view shell**

Register `/difference-finder`, render the tool shell, and keep `/` unchanged.

- [ ] **Step 2: Add the compact homepage card**

Place it below the two existing task cards. On click, call `WebviewWindow.getByLabel('difference-finder')`; focus the existing window or create one with URL `/difference-finder`, title `ImageKeeper - 找差分图`, size `1280 × 820`, and minimum `980 × 680`.

- [ ] **Step 3: Allow webview window creation**

Add the narrow required Tauri core webview/window permission to `default.json`; do not broaden filesystem permissions because file mutations occur in Rust commands.

- [ ] **Step 4: Run frontend build**

Run: `npm run build`  
Expected: PASS.

- [ ] **Step 5: Commit the unit**

```bash
git add src/views/MainView.vue src/router/index.ts src-tauri/capabilities/default.json src/views/DifferenceFinderView.vue
git commit -m "feat: add difference finder window entry"
```

### Task 7: Search and result UI

**Files:**
- Create: `src/components/difference-finder/ReferenceImageStrip.vue`
- Create: `src/components/difference-finder/SearchSetupPanel.vue`
- Create: `src/components/difference-finder/DifferenceResultList.vue`
- Modify: `src/views/DifferenceFinderView.vue`

- [ ] **Step 1: Build reference and target inputs**

Support multi-file reference selection, multi-directory selection, drag/drop reference files, duplicate-path removal, recursive toggle, remove/clear actions, and disabled start state.

- [ ] **Step 2: Build progress and cancellation**

Show scan/feature/match/score/aggregate phases with processed/total/current file; cancellation calls the scoped command and leaves completed results visible.

- [ ] **Step 3: Build deduplicated result browsing**

Add all-reference/single-reference tabs, classification/source/selected filters, stable sorting, multi-select, result evidence, and thumbnail preview using `convertFileSrc`. A multi-reference match remains one selectable item with multiple reference chips.

- [ ] **Step 4: Integrate the view**

Use a responsive two-stage layout: setup and reference strip at top, result browser in the main pane, batch organizer opened from selected results.

- [ ] **Step 5: Run frontend build**

Run: `npm run build`  
Expected: PASS.

- [ ] **Step 6: Commit the unit**

```bash
git add src/components/difference-finder src/views/DifferenceFinderView.vue
git commit -m "feat: build difference search interface"
```

### Task 8: Batch rename grid and file actions

**Files:**
- Create: `src/components/difference-finder/BatchRenameGrid.vue`
- Modify: `src/views/DifferenceFinderView.vue`
- Modify: `src/stores/differenceFinderStore.ts`

- [ ] **Step 1: Build the div/grid organizer**

Render selection, native drag handle/order, thumbnail, original name, editable new-name input, best reference/classification, and validation state. Drag order updates `$n` previews immediately.

- [ ] **Step 2: Build detailed rule help and examples**

Place variable chips next to the template input and show the three simple examples plus three wildcard-capture examples from the approved design. Clicking a chip inserts it at the cursor.

- [ ] **Step 3: Add the quick rename action**

“按首项快速编号” uses the first visible organizer row’s current stem and produces `_1`, `_2`, … in drag order while preserving each original extension. The adjacent help text states this behavior exactly.

- [ ] **Step 4: Wire preview and conflict blocking**

Debounce backend preview, show red blocking errors and yellow existing-target conflicts, allow row-level manual names, and require a final source→target confirmation before execution.

- [ ] **Step 5: Wire rename, move, copy, and undo**

Add destination directory dialogs, new-subfolder input, recent copy destination, per-item outcome summary, result-path refresh, and undo button only for the most recent successful rename/move batch.

- [ ] **Step 6: Run frontend and Rust verification**

Run: `npm run build`  
Expected: PASS.  
Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: all tests PASS.

- [ ] **Step 7: Commit the unit**

```bash
git add src/components/difference-finder/BatchRenameGrid.vue src/views/DifferenceFinderView.vue src/stores/differenceFinderStore.ts
git commit -m "feat: add batch difference image organizer"
```

### Task 9: Documentation and end-to-end verification

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-07-15-difference-image-finder.md`

- [ ] **Step 1: Document the tool**

Add the independent entry, multi-reference behavior, direct visual-match scope, rename syntax, quick rename behavior, and no-silent-overwrite safety boundary to README.

- [ ] **Step 2: Run formatting and complete verification**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`  
Expected: PASS.  
Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: all tests PASS.  
Run: `npm run build`  
Expected: PASS.

- [ ] **Step 3: Perform desktop smoke test**

Run: `npm run tauri:dev`. Verify the compact card opens/reuses one independent window; two reference images search one target scan; reference filtering does not recompute; drag ordering changes numbering; duplicate names block execution; quick rename produces `first_1`, `first_2`; rename and undo restore files; copy and move never overwrite existing unrelated content.

- [ ] **Step 4: Mark plan checkboxes and commit docs**

```bash
git add README.md docs/superpowers/plans/2026-07-15-difference-image-finder.md
git commit -m "docs: explain difference finder workflow"
```

## Self-review result

- Spec coverage: entry/window, multi-reference scanning, direct-match aggregation, result filtering, drag order, simple and capture templates, detailed examples, quick rename, collision handling, rename/move/copy, cancellation, and undo all map to tasks above.
- Placeholder scan: no `TBD`, `TODO`, incomplete code-body comments, or deferred implementation instructions remain.
- Type consistency: Rust command/type names use snake_case commands and camelCase serde payloads; frontend wrappers use the same request/result names.


