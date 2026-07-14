# ImageKeeper v1 Image Comparison Design

## Goal

Implement the complete workflow defined by `IMAGE_COMPARISON_WORKFLOW.md`: one protected baseline root, any number of comparison roots, deterministic analysis, explicit human review, recoverable recycling, audited permanent deletion, and run-scoped reports.

The existing Tauri and Vue shell remains useful, but the legacy data model and placeholder workflows are not compatibility constraints. Existing analysis records must never become executable recycle decisions after migration.

## Product Boundary

ImageKeeper answers whether each image in a comparison root is already represented by, or is a possible lower-quality or content variant of, an image in the baseline root. It does not perform comparison-root-to-comparison-root deduplication and does not modify the baseline root.

The v1 workflow consists of five user-facing stages:

1. Configure and preflight a protected baseline, comparison roots, and report root.
2. Run scanning, feature extraction, candidate search, scoring, and arbitration.
3. Review each comparison result and explicitly approve or reject recycle actions.
4. Revalidate approved files and move them into run-scoped recycle directories.
5. Restore or permanently delete recycled files from a separate recycle view and export run-scoped reports.

## Architecture

The application keeps the current Tauri 2, Rust, Vue 3, Pinia, and Element Plus stack. The Rust backend is rebuilt around domain modules with narrow interfaces:

- `preflight`: canonicalizes roots, rejects overlap and nesting, verifies access, and builds permanent exclusions.
- `manifest`: scans supported files without following directory links and records file-level stage outcomes.
- `features`: computes BLAKE3, 64-bit perceptual hashes, and versioned image metadata with cache validation.
- `matching`: builds the baseline exact-hash and pHash neighbor indexes and retains stable Top-K evidence.
- `scoring`: normalizes comparable images and computes a real, windowed SSIM metric.
- `analysis`: assigns exactly one analysis type using all candidate evidence and deterministic conflict rules.
- `review`: validates allowed review transitions and records the reason, note, and timestamp.
- `actions`: revalidates files, journals filesystem operations, recycles, restores, deletes, and reconciles interruptions.
- `reports`: generates atomic JSON, CSV, and HTML exports scoped to a single run.
- `runner`: executes the pipeline, emits stable progress events, and honors pause, resume, and cancel controls at real checkpoints.

Tauri commands expose request and response DTOs rather than database records. Commands delegate all safety decisions to domain services so that UI callers cannot bypass invariants.

## Persistence Model

SQLite uses explicit migrations through `PRAGMA user_version`. The v1 schema separates configuration, evidence, decisions, and actions:

- `runs`: immutable algorithm snapshot, lifecycle state, timestamps, warning acknowledgements, and error counts.
- `run_roots`: canonical path, role, stable alias, and root-specific recycle/report metadata.
- `manifest_entries`: run/root identity, relative path, filesystem fingerprint, dimensions, format, frame policy, BLAKE3, pHash, algorithm versions, and stage outcomes.
- `candidate_evidence`: comparison image, baseline candidate, pHash distance, aspect difference, SSIM, comparability, classification evidence, truncation metadata, and stable rank.
- `analysis_results`: one row per comparison image with exactly one of the eight analysis types and a primary match when available.
- `reviews`: current review status plus immutable review events containing reason, note, and time.
- `action_journal`: prepared and terminal events for every filesystem operation.
- `recycle_entries`: original and recycled locations, run identity, source result, fingerprints, and current recycle lifecycle.
- `feature_cache`: reusable features keyed by canonical identity, size, modification time, and algorithm versions.

Legacy settings may be migrated when their meaning is still safe. Legacy scans, duplicate pairs, and recycle candidates are retained only as non-actionable history or archived during migration; they cannot be approved or executed by v1 commands.

## Analysis Pipeline

### Preflight and Manifest

Exactly one baseline and at least one comparison root are required. Canonical paths must be distinct and non-nested across every root. The scanner always excludes `.recycle`, report output, application caches, and temporary output, and never follows directory symbolic links in v1.

Each discovered image becomes a manifest entry before expensive work begins. Decode, hash, and feature failures are recorded per file and do not abort unrelated files. Unsupported inputs become `not_evaluated`; attempted operations that fail become `error`. Neither can become an executable action.

### Features and Candidate Search

BLAKE3 is the only exact fingerprint. Exact comparison matches retain every baseline file with the same hash and choose the lexicographically stable baseline relative path as the primary display match.

Non-exact files query a 64-bit pHash neighbor index with inclusive Hamming distance and a versioned maximum of 10. The index must avoid the baseline-by-comparison Cartesian product. Results are sorted deterministically and capped by versioned Top-K configuration. Truncation is persisted and conservatively arbitrated.

### Normalization and SSIM

The versioned normalization profile applies EXIF orientation, converts decoded pixels to a defined sRGB representation, composites alpha against the configured background, selects the first animation frame, and resizes only the higher-resolution image down to the lower-resolution dimensions using a fixed filter. Aspect ratio comparability uses the relative-error formula from the target specification.

SSIM is a real windowed structural similarity computation with fixed channel and constant parameters. MSE-derived similarity must not be exposed as SSIM. Unalignable or failed candidate pairs remain non-actionable evidence.

### Arbitration

Each comparison manifest entry receives exactly one analysis type. Exact matches win first. Remaining evidence is ordered by comparability, SSIM, pHash distance, baseline resolution, and stable relative path. Candidate truncation, incomplete evidence, close top scores, or conflicting candidate conclusions produce `inconclusive` unless a rule can prove a unique conservative result.

`likely_compressed` requires every directional condition from the target specification: both dimensions, pixel count, and file size are smaller than the baseline; aspect difference is within tolerance; and SSIM meets the inclusive threshold. Same-resolution re-encodes never become `likely_compressed`. Similar evidence that should remain is classified as `variant` or `similar_keep`; complete searches with no qualifying evidence become `no_baseline_match`.

## Review and Action Safety

Analysis, review, and action states are independent. Exact duplicates may be selected in the UI but require explicit confirmation. Likely compressed images begin pending and are never preselected. Inconclusive, not-evaluated, error, and stale results cannot be batch approved.

Before recycling, the backend verifies that the source remains inside its comparison root, its size, modification time, and BLAKE3 match the manifest, and at least one relied-upon baseline match remains inside the baseline with the expected BLAKE3. A failed check marks the action stale and performs no move.

Successful recycling writes a prepared journal event, moves the source to `.recycle/<runId>/<relative-path>` without overwriting, verifies the destination, and records the recycled event. Unique target naming resolves pre-existing recycle paths. Startup reconciliation inspects prepared operations and identifies whether the source, target, both, or neither exist.

Restore never overwrites an occupied original path. Permanent deletion is available only from the recycle view, requires a separate confirmation with an exact count, verifies the path remains inside the recorded recycle root, and records an audit event. No timer silently deletes recycled content.

## Reports and Privacy

JSON is the complete canonical report. CSV includes run, analysis, review, and action state for filtering. HTML is an offline human-readable view without embedded images or absolute paths by default. All queries are scoped by `runId`; filenames contain `runId`; output is written to a temporary file, validated, and atomically renamed without silent overwrite.

Logs use root aliases and relative paths, omit pixel data and thumbnails, and never emit complete BLAKE3 values. All image analysis remains local.

## User Interface

The Vue application is a quiet desktop workbench rather than a marketing surface:

- `Runs`: create a run, assign the protected baseline and comparison roots, choose a report location, and view preflight errors.
- `Progress`: display stable pipeline stages, counts, file-level errors, pause/resume/cancel controls, and completion state.
- `Review`: filter by analysis type and root, compare baseline and source images side by side, switch candidates, inspect evidence, and record keep/recycle decisions.
- `Recycle`: list run-scoped recycled files, restore selected entries, and perform separately confirmed permanent deletion.
- `Reports`: show summary conservation, report history, and JSON/CSV/HTML export actions.
- `Settings`: edit only defaults for future runs and explain that historical algorithm snapshots do not change.

The baseline side never exposes a delete control. Batch confirmation groups counts by analysis type and source root and explicitly states that only comparison files are affected. All destructive or long-running actions have loading, empty, error, stale, and interrupted states.

## Failure and Recovery

Undefined states fail closed. File errors continue with safe classifications; database, manifest, or report infrastructure failures stop the run. Pause, resume, and cancel are cooperative controls checked between bounded work units and persist checkpoints. Repeating a run command or recovery operation is idempotent by run and entry identity.

Concurrent feature computation does not share a rusqlite connection. Workers return bounded result batches to a single persistence boundary. Concurrency may change throughput but cannot change rankings or classifications.

## Verification Strategy

Rust unit and integration tests provide the primary safety evidence. Fixtures are generated synthetic images or explicitly public assets and cover exact bytes, re-encoding, proportional resize, compression quality, equal-resolution re-encoding, local edits, text/watermarks, color shifts, aspect changes, crop, rotation, EXIF orientation, alpha, grayscale, animation-first-frame behavior, corrupt files, inclusive thresholds, and multi-baseline conflicts.

Filesystem tests cover canonical overlap, nesting, symlink escape, permanent exclusions, source and baseline staleness, baseline action rejection, target collisions, interrupted moves, reconciliation, restore conflicts, idempotent controls, and run isolation. State and report tests prove the eight-category conservation equation and the absence of executable actions for error, unevaluated, stale, truncated, and conflicting results.

Frontend tests cover stores, route workflows, filter and selection rules, confirmation copy, and unavailable action states. Completion requires fresh successful Rust tests, frontend tests, TypeScript checking, production web build, Tauri build/check, and a local desktop smoke test using the demo fixtures. A final requirement-by-requirement audit maps every target-spec checklist item to code and test evidence.

## Implementation Order

1. Establish migrations and domain types with state invariants.
2. Implement path preflight, manifest scanning, exclusions, and cache identity.
3. Implement features, baseline indexes, normalization, SSIM, and arbitration.
4. Implement real runner controls and run-scoped queries.
5. Implement review transitions and the journaled recycle lifecycle.
6. Implement report generation and privacy constraints.
7. Replace the placeholder frontend with the complete workbench.
8. Calibrate synthetic fixtures, run full verification, and audit the target specification.

This order produces testable layers while keeping the final scope equal to the complete target specification.
