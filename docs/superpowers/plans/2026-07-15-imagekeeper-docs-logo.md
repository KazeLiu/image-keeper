# ImageKeeper Docs and Logo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the temporary ImageKeeper icon with a maintainable “guarded image stack” brand mark and rewrite the README so it accurately explains the product, technology, and the distinction between variant classification and image grouping.

**Architecture:** Keep one SVG in `assets/` as the source of truth and let the Tauri CLI deterministically generate platform assets into `src-tauri/icons/`. Keep the documentation change isolated from the existing comparison implementation and describe thresholds and safety behavior directly from the current Rust code.

**Tech Stack:** SVG 1.1, Tauri CLI 2, Vue 3, TypeScript, Vite 5, Rust 2021, SQLite/rusqlite, BLAKE3, 64-bit pHash, `image`, `fast_image_resize`.

---

## File map

- Create `assets/imagekeeper-logo.svg`: canonical editable Logo source.
- Modify `README.md`: product overview, workflow, classification/grouping explanation, tech stack, safety boundaries, and development commands.
- Delete `create-icon.html`: remove the browser-download Emoji icon prototype.
- Delete `generate-icon.js`: remove the non-generating placeholder script and external default-icon suggestion.
- Generate `src-tauri/icons/**`: platform assets derived from the SVG by the installed Tauri CLI.
- Do not stage or edit the existing modified Vue/Rust feature files shown by `git status`.

### Task 1: Create the canonical Logo source

**Files:**
- Create: `assets/imagekeeper-logo.svg`
- Delete: `create-icon.html`
- Delete: `generate-icon.js`

- [ ] **Step 1: Add the SVG source**

Create a square SVG with transparent outer corners, a rounded indigo/cyan field, two image cards, and a shield-check badge. Use this exact structure so all geometry remains vector-based and font-independent:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" role="img" aria-labelledby="title description">
  <title id="title">ImageKeeper Logo</title>
  <desc id="description">Two layered image cards protected by a shield and check mark</desc>
  <defs>
    <linearGradient id="background" x1="132" y1="108" x2="894" y2="916" gradientUnits="userSpaceOnUse">
      <stop stop-color="#312E81"/>
      <stop offset="0.52" stop-color="#4F46E5"/>
      <stop offset="1" stop-color="#0891B2"/>
    </linearGradient>
    <linearGradient id="accent" x1="330" y1="448" x2="710" y2="690" gradientUnits="userSpaceOnUse">
      <stop stop-color="#4F46E5"/>
      <stop offset="1" stop-color="#06B6D4"/>
    </linearGradient>
    <filter id="shadow" x="-25%" y="-25%" width="150%" height="160%">
      <feDropShadow dx="0" dy="32" stdDeviation="34" flood-color="#111827" flood-opacity="0.3"/>
    </filter>
  </defs>
  <rect x="64" y="64" width="896" height="896" rx="224" fill="url(#background)"/>
  <path d="M176 222C290 102 480 80 650 120" fill="none" stroke="#FFFFFF" stroke-width="22" stroke-linecap="round" opacity="0.16"/>
  <g transform="rotate(-7 476 410)" opacity="0.72">
    <rect x="178" y="198" width="596" height="424" rx="84" fill="#CFFAFE" stroke="#FFFFFF" stroke-width="24"/>
    <circle cx="304" cy="320" r="46" fill="#22D3EE"/>
    <path d="M226 548L358 408L452 500L542 392L724 548Z" fill="#818CF8"/>
  </g>
  <g filter="url(#shadow)">
    <rect x="238" y="302" width="548" height="438" rx="92" fill="#F8FAFC"/>
    <circle cx="362" cy="420" r="48" fill="#22D3EE"/>
    <path d="M294 660L418 524L508 608L596 492L730 660Z" fill="url(#accent)"/>
  </g>
  <g filter="url(#shadow)">
    <path d="M678 482C730 518 778 530 824 534V650C824 746 766 817 678 858C590 817 532 746 532 650V534C578 530 626 518 678 482Z" fill="#312E81" stroke="#CFFAFE" stroke-width="18" stroke-linejoin="round"/>
    <path d="M608 664L654 710L752 604" fill="none" stroke="#F8FAFC" stroke-width="34" stroke-linecap="round" stroke-linejoin="round"/>
  </g>
</svg>
```

- [ ] **Step 2: Remove obsolete icon prototypes**

Delete `create-icon.html` and `generate-icon.js`. They are replaced by the SVG source and the reproducible command in Task 2.

- [ ] **Step 3: Validate the source is well-formed**

Run:

```powershell
npx tauri icon --help
```

Expected: exit code 0 and help text saying the input may be a squared PNG or SVG with transparency.

- [ ] **Step 4: Commit the source cleanup**

```powershell
git add -- assets/imagekeeper-logo.svg create-icon.html generate-icon.js
git diff --cached --check
git commit -m "feat: redesign ImageKeeper app icon"
```

Expected: one commit containing the SVG source and deletion of the two temporary generators, without existing Vue/Rust working-tree changes.

### Task 2: Generate and inspect Tauri platform icons

**Files:**
- Modify: `src-tauri/icons/icon.ico`
- Create: Tauri-generated PNG, ICNS, Store, iOS, and Android assets under `src-tauri/icons/`

- [ ] **Step 1: Generate assets from the canonical SVG**

Run:

```powershell
npx tauri icon assets/imagekeeper-logo.svg
```

Expected: exit code 0 and generated assets under `src-tauri/icons/`, including `icon.ico`, `icon.icns`, `icon.png`, `32x32.png`, `128x128.png`, and `128x128@2x.png`.

- [ ] **Step 2: Verify required Windows and desktop outputs**

Run:

```powershell
$required = @('icon.ico', 'icon.png', '32x32.png', '128x128.png', '128x128@2x.png')
$missing = $required | Where-Object { -not (Test-Path -LiteralPath (Join-Path 'src-tauri/icons' $_)) }
if ($missing) { throw "Missing icons: $($missing -join ', ')" }
Get-ChildItem -LiteralPath 'src-tauri/icons' -File | Sort-Object Name | Select-Object Name, Length
```

Expected: no exception and all required files have non-zero lengths.

- [ ] **Step 3: Visually inspect large and small raster outputs**

Open `src-tauri/icons/icon.png`, `src-tauri/icons/128x128.png`, and `src-tauri/icons/32x32.png`. Confirm the layered cards and shield-check remain recognizable and no SVG element is clipped.

- [ ] **Step 4: Commit generated assets**

```powershell
git add -- src-tauri/icons
git diff --cached --check
git commit -m "feat: generate platform icons from new logo"
```

Expected: generated icon files only.

### Task 3: Rewrite the project README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace the obsolete project description**

Write the README with these sections and facts:

1. Centered Logo, name, and the sentence “本地运行、以保护原图为优先的图片对比与整理工具”。
2. “它是做什么的”：single-directory internal comparison or baseline-versus-comparison directories; results are review suggestions, not automatic deletion.
3. “工作流程”：scan metadata → BLAKE3 exact comparison → 64-bit pHash candidate recall/grouping → normalize to grayscale up to 512px and calculate `1 - MSE / 65025` → conservative classification → manual review/recycle/export.
4. “如何区别差分图和组图”：grouping is a transitive pHash connected component; variant is a per-comparison-image result relative to baseline evidence. Include the default 10 grouping distance, 0.75 variant lower bound, 0.995 compressed threshold, 0.95–1.05 pixel ratio, and the directional compressed constraints.
5. “结果类型”：document `exact_duplicate`, `likely_compressed`, `variant`, `similar_keep`, `no_baseline_match`, `inconclusive`, `not_evaluated`, and `error`, with only the first two eligible for cleanup suggestion and all destructive operations requiring review.
6. “技术栈”：Vue 3/TypeScript/Vite/Element Plus/Pinia/Vue Router; Tauri 2/Rust/Tokio/Rayon; SQLite/rusqlite; `image`/`fast_image_resize`; BLAKE3/pHash; CSV.
7. “支持格式”：JPG/JPEG, PNG, WebP, BMP, and first-frame GIF, matching the current default config.
8. “开发”：Node.js, Rust, Tauri prerequisites, `npm install`, `npm run tauri:dev`, `npm run build`, `cargo test --manifest-path src-tauri/Cargo.toml`, and `npx tauri icon assets/imagekeeper-logo.svg`.
9. “安全边界”：baseline never selected for deletion, grouping does not prove duplication, same-resolution variants are retained, recycle operations validate file fingerprints, and incomplete/conflicting evidence fails closed.

Do not claim “enterprise grade”, 500,000-image support, complete SSIM, AVIF support, or finished features that are not proven by the current code.

- [ ] **Step 2: Check documentation terminology against code**

Run:

```powershell
rg -n "企业级|50 万|AVIF|完整 SSIM|SSIM 引擎" README.md
rg -n "BLAKE3|pHash|0\.75|0\.995|0\.95|组图|差分图|MSE|回收" README.md
```

Expected: the first command has no matches; the second command finds every required concept.

- [ ] **Step 3: Commit the README**

```powershell
git add -- README.md
git diff --cached --check
git commit -m "docs: explain ImageKeeper workflow and grouping"
```

Expected: README only.

### Task 4: Verify, audit scope, and push

**Files:**
- Verify all files from Tasks 1–3; do not modify unrelated feature files.

- [ ] **Step 1: Run the frontend build**

```powershell
npm run build
```

Expected: `vue-tsc --noEmit` and `vite build` exit with code 0.

- [ ] **Step 2: Run Rust tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust unit tests pass. If unrelated current working-tree changes cause a failure, record the exact failing test and run `cargo check --manifest-path src-tauri/Cargo.toml` as an additional diagnostic; do not rewrite those unrelated files.

- [ ] **Step 3: Audit commits and dirty files**

```powershell
git status --short --branch
git log -4 --oneline --decorate
git diff --name-only HEAD -- src-tauri/src src/components src/types src/views
```

Expected: README/Logo work is committed; the pre-existing modified Vue/Rust files remain unstaged and unchanged by this plan.

- [ ] **Step 4: Push the current main branch**

```powershell
git push origin main
```

Expected: `origin/main` advances to the final documentation/Logo commit. If authentication or remote policy rejects the push, report the exact Git error without changing credentials or rewriting history.
