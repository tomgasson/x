# oxc — Rust-based JavaScript tooling ecosystem

## Project Overview

oxc is a large Rust monorepo providing a high-performance parser, linter (oxlint), formatter (oxfmt), minifier, and transformer for JavaScript/TypeScript. It is the engine behind Rolldown (Rollup's Rust successor). The project maintains extremely high velocity with daily releases and has a strong emphasis on measured performance work.

---

## Substantive Commits

### oxc b9171eaed7 fix(track-memory-allocations): reserve transform scoping capacity (#22621)

**Author:** Dunqing
**Situation:** The `Allocations` CI job was failing on PRs unrelated to the transformer because `cargo allocs` reported different system allocation counts across platforms (Linux x64 vs macOS/arm64). This caused spurious CI failures.

**Approach:** Root-cause analysis traced the failure to the allocation tracker building `Scoping` for the transformer differently from the production path. Production uses `SemanticBuilder::with_excess_capacity(2.0)` because transforms can add scopes/symbols/references. The tracker wasn't reserving that excess, so a JSX-generated binding on Linux crossed an arena chunk boundary while macOS stayed within capacity — a platform-sensitive capacity cliff being captured in the snapshot.

**Mechanism:** The fix builds a separate production-like `Scoping` for transformer measurement using `with_excess_capacity(2.0)`, matching the compiler benchmark path. The `allocs_transformer.snap` is regenerated.

**Scale implications:** Capacity-boundary effects are a persistent source of cross-platform CI noise in arena-allocated storage. The arena chunk size (16 bytes on Linux) means any growth can flip allocation counts non-deterministically depending on where the boundary lands for a given binary's memory layout.

**Cost:** 2 files changed. No behavior change — only measurement accuracy.

---

### oxc 4ab57eb27e fix(allocator): fixed-size allocators use `VirtualAlloc` on Windows (#22124)

**Author:** overlookmotel
**Situation:** On Windows, allocating a large chunk from the `System` allocator immediately counts toward the memory limit, even if most of it is never touched. On macOS/Linux, overcommit accounting means virtual memory allocation is essentially free until pages are actually used. Oxlint's JS plugins pre-allocate a 6 GiB arena for fixed-size allocators — on Windows, this caused OOM just from linting the first file.

**Approach:** Switch Windows to use `VirtualAlloc`, which allows reserving address space separately from committing physical pages. Reserve 6 GiB (address space only), commit only 2 GiB as the active region starting from the last 16 KiB, and grow the committed region downward as needed. This replicates Linux-style on-demand commit behavior on Windows.

**Mechanism:** `VirtualAlloc` with `MEM_RESERVE` for the reservation, then `MEM_COMMIT` for pages as they're touched. The implementation references wasmtime's Windows mmap equivalent. Platform-specific code in `crates/oxc_allocator/src/arena/fixed_size/windows.rs`.

**Scale implications:** On Windows, memory-constrained environments are common (32 GB machines, shared CI). Virtual memory vs. committed memory is a fundamental distinction that differs across platforms. Fixed-size allocators (used by JS plugins) are particularly sensitive because they deliberately over-allocate virtual address space.

**Cost:** 8 files changed, significant platform-specific code added. Risk: the virtual address space exhaustion case for enormous projects (128 TiB limit with many concurrent 6 GiB arenas) is known but deferred.

---

### oxc 0bf0cb938e perf(allocator): per-platform `Arena::new_fixed_size` implementations (#22088)

**Author:** overlookmotel
**Situation:** A single cross-platform allocator strategy didn't optimally handle each platform's memory allocation characteristics. Different platforms have different abilities to service high-alignment requests.

**Approach:** Split into platform-specific implementations:
- **macOS**: System allocator refuses 4 GiB-aligned requests. Allocate ~4 GiB with 2 GiB alignment, use top or bottom half for arena chunk.
- **Windows**: System allocator can't handle alignment > 16. Allocate ~6 GiB with 16 alignment, find a 2 GiB block aligned on 4 GiB within it (what `std` was doing, but self-implementation avoids `std`'s annoyances).
- **Linux**: Direct high-alignment request (~2 GiB with 4 GiB alignment).

**Mechanism:** New files `crates/oxc_allocator/src/arena/fixed_size/{linux,macos,windows}.rs`. Reduces virtual memory on Linux, sidesteps `std`'s workaround on Windows.

**Scale implications:** Arena allocation strategy directly affects both performance and memory limits. The approach of using the platform's native high-alignment capabilities rather than fighting against them reduces waste.

**Cost:** Structural refactor — per-platform files, new module. No behavior change on macOS, improvement elsewhere.

---

### oxc d782b78d5f perf(minifier): use BitSet for LiveUsageCollector live references (#22425)

**Author:** Boshen
**Situation:** `LiveUsageCollector` runs after each peephole pass to collect surviving `IdentifierReference` IDs for pruning resolved-reference lists. It used a `FxHashSet<ReferenceId>`. `ReferenceId` is a dense `nonmax_u32` index from 0 to `scoping.references_len()` — a perfect candidate for a bitset.

**Approach:** Swap to the existing `oxc_allocator::BitSet` (already used by mangler and linter for `SymbolId`/`ScopeId`/CFG bitsets). Reference IDs are dense, so indexed access replaces hash + probe.

**Mechanism:**
| | FxHashSet | BitSet |
|---|---|---|
| insert | ~25 cycles | ~5 cycles |
| contains | ~25 cycles | ~3 cycles |
| memory | MB-scale | KB-scale, arena |

Also adds `Scoping::references_len()` to mirror `symbols_len()`/`scopes_len()`.

**Scale implications:** The minifier's fixed-point loop runs many iterations. HashSet hashing on every insertion was a hidden constant factor — especially visible on large files with many references (cal.com.tsx shows −4% wall-time, variance drops 20x). The bitset change makes the hot path O(1) with minimal memory footprint.

**Cost:** 4 files changed. Byte-identical output. Allocation snapshots update (heap → arena). Follow-up work to store one bitset on `MinifierState` and `clear()` instead of per-iteration allocation deferred.

---

### oxc 217d7d8a1c perf(minifier): index `SymbolValues` by `SymbolId` (#22441)

**Author:** Dunqing
**Situation:** `SymbolValues` backed by `FxHashMap<SymbolId, SymbolValue>` — every hot path in the minifier (inline_identifier_reference, is_symbol_mutated, dead-code checks) went through hash + probe.

**Approach:** Swap to `IndexVec<SymbolId, Option<SymbolValue>>`. Symbol IDs are dense u32s, so indexed access replaces the hash map. The buffer is sized once from `Scoping::symbols_len()` at `MininerState::new` and reset in-place between peephole iterations.

**Mechanism:** Stacked on #22425 (BitSet). Local harness on `Compressor::build_with_scoping` with 4 test files, 27 paired cycles: wins 27/27, median −207 µs (−2.97%), sign-test p < 0.000001.

**Scale implications:** The minifier mints no new `SymbolId`s mid-run (verified across all peephole/normalize/mangle/keep_var passes — only `traverse_context/` has `generate_uid_name`/`create_symbol` with zero callers), so indexed write never needs to grow. If that assumption breaks, the write would panic — serving as the signal to add a grow path.

**Cost:** 4 files changed. System allocations drop on every test file. No behavior change.

---

### oxc 98be95c009 perf(regular_expression): track regex flags via bitflags (#22427)

**Author:** Boshen
**Situation:** `FlagsParser` used `FxHashSet<u32>` to detect duplicate regex flags. Per-regex hash-set allocation in flag parsing was unnecessary overhead.

**Approach:** Replace with a typed `bitflags!` set over `u8`. No per-regex heap allocation.

**Mechanism:** 76-line rewrite of `flags_parser.rs`. `allocs_parser.snap` and `allocs_minifier.snap` show reduced allocations.

**Scale implications:** Parsing many regex literals creates many small hash sets. A bitflags enum eliminates those allocations entirely for a common hot path.

**Cost:** 3 files changed, 63 insertions / 23 deletions.

---

### oxc 5bd3b2580c perf(linter/no-unused-vars): avoid cloned ancestor iterator (#22598)

**Author:** camc314
**Situation:** The `no-unused-vars` parent/grandparent walks were cloning and skipping a chained parent iterator repeatedly for each pair, re-walking iterator state unnecessarily.

**Approach:** Advance one filtered ancestor iterator and carry the previous parent kind forward, avoiding the repeated re-walking.

**Mechanism:** Refactored `no_unused_vars/symbol.rs` — 15 insertions, 9 deletions. The helper now maintains iteration state across checks rather than cloning the ancestor chain per pair.

**Scale implications:** For files with many variable declarations in nested contexts, the per-pair re-walking accumulates. This is a targeted micro-optimization for one of the most-called linter rules.

**Cost:** 1 file, 24 lines changed (15 insertions, 9 deletions).

---

### oxc 7a4120ed92 perf(semantic): pre-reserve unresolved_references using Stats::references (#22580)

**Author:** Dunqing
**Situation:** `unresolved_references Vec<(Ident, ReferenceId)>` in `SemanticBuilder` grew from cap=0 via default doubling policy. For a moderately-sized TypeScript file (~5k references), that meant ~13 reallocations copying ~16k entries (~250 KB of memory traffic).

**Approach:** `Stats` already knows the exact reference count upfront. Pre-reserve with `stats.references` exactly using `reserve_exact`.

**Mechanism:** 8-line internal change to `crates/oxc_semantic/src/builder.rs`. Uses `reserve_exact` (not `reserve`) — empirically 100% reserve is optimal; sub-100% reserves performed worse than no reserve due to a single large-block realloc missing the small-doubling size-class fast paths.

**Scale implications:** Every semantic build pays the reallocation cost. For small files the absolute cost is negligible, but for large TypeScript files (193 KB binder.ts shows −1.7%), the cumulative copying is measurable. CodSpeed instruction counts show −4.34% on `pipeline[binder.ts]` and −9.75% on `semantic[binder.ts]`.

**Cost:** No public API impact. 2 files changed. Allocation snapshots update.

---

### oxc ce92c6ccc1 perf(semantic): `#[inline]` `Scoping::get_binding` (#22414)

**Author:** Dunqing
**Situation:** `walk_up_resolve_reference` (the hot scope-chain walk in reference resolution) was `#[inline(always)]` but called `Scoping::get_binding` which wasn't `#[inline]`. With `oxc_semantic` and `oxc_syntax` in separate crates and `borrow_dependent` from `self_cell`, the inliner couldn't reach through the boundary without a hint.

**Approach:** Add `#[inline]` to `get_binding` so the caller can inline through `borrow_dependent` + the indexed `FxHashMap` lookup as a single specialized path.

**Mechanism:** Single-attribute change. Measured on a 7.83 MB TypeScript bundle:
- No features: 1.034s → 1.010s (1.02× speedup)
- `cfg,linter` features: noise-level (per-node bookkeeping dominates)

**Scale implications:** Cross-crate inlining is a known limitation in Rust. When hot paths cross crate boundaries, `#[inline]` hints become necessary even for small methods. The 2% win on the simple case is worth the zero-cost attribute.

**Cost:** 1 file, 1 line changed.

---

### oxc d5bead17bd fix(tasks): reset allocators in mangler keep_names benchmark (#22597)

**Author:** Dunqing
**Situation:** `bench_mangler`'s `keep_names` variant was missing `allocator.reset()`/`temp_allocator.reset()` calls that every other write-into-arena benchmark has. Without resets, arenas grew across criterion's warmup + measurement iterations and hit libc allocation paths — explicitly called out as non-deterministic in the benchmark lib. This caused ~3% recurring noise on CodSpeed for unrelated PRs.

**Approach:** Mirror the working sibling pattern at the regular `bench_mangler` immediately above. Add the missing allocator resets.

**Mechanism:** 4 insertions, 2 deletions in `tasks/benchmark/benches/minifier.rs`. The arenas now reset between iterations, keeping them within criterion's expected deterministic measurement.

**Scale implications:** Benchmark noise from non-deterministic allocation paths is a real problem for continuous performance measurement. CodSpeed runs must distinguish signal from noise — allocator growth across iterations was contaminating the measurement.

**Cost:** 1 file, 6 lines. Fixes false-positive CodSpeed regressions.

---

### oxc e216a840e8 refactor(allocator): add `Arena::grow_fixed_size_chunk` method (#22123)

**Author:** overlookmotel
**Situation:** Preparatory work for the Windows `VirtualAlloc` fix (#22124). All platforms had identical `Arena::new_fixed_size` but there was no in-place growth capability for fixed-size arenas.

**Approach:** Add a private `Arena::grow_fixed_size_chunk` method with per-platform implementations. Currently all return `None` (could not grow), but the Windows implementation in #22124 will provide actual growth via `VirtualAlloc`.

**Mechanism:** 102 insertions across `alloc_impl.rs`, `unix.rs`, `windows.rs`. Sets up the platform abstraction for growth.

**Scale implications:** Fixed-size arenas that can't grow are constrained by their initial reservation. Adding in-place growth (Windows-specific) enables handling larger ASTs within the same arena without allocating new chunks.

**Cost:** Pure refactor, no behavior change. Foundation for #22124.

---

### oxc 5086ddc902 refactor(allocator): per-platform implementation of freeing fixed-size chunks (#22122)

**Author:** overlookmotel
**Situation:** All platforms used `System` allocator for deallocation. Windows-specific `VirtualAlloc` use requires corresponding `VirtualFree` — the current uniform approach wouldn't support that.

**Approach:** Add per-platform `dealloc_fixed_size_arena_chunk` implementations. Currently all identical (just deallocate with `System`), but Windows impl in #22124 switches to `VirtualFree`.

**Mechanism:** 101 insertions, 20 deletions across 6 files. Abstraction for platform-specific deallocation.

**Scale implications:** When allocation strategy is platform-specific, deallocation must match. This is a common pattern when using OS-native memory APIs — you can't mix `VirtualAlloc` with `free()`, etc.

**Cost:** Refactor only. Currently all platforms behave identically.

---

### oxc 0ffbe0dd4c feat(allocator)!: remove `Allocator::end_ptr` method (#21871)

**Author:** overlookmotel
**Situation:** `Allocator::end_ptr` method was no longer required since #21869 (the raw transfer store refactor that moved metadata into the arena chunk itself).

**Approach:** Breaking API removal — remove the method entirely.

**Mechanism:** 8 deletions in `crates/oxc_allocator/src/from_raw_parts.rs`.

**Scale implications:** When internal implementation changes make methods unnecessary, removing them prevents dead code and keeps the API honest. The `!` marker indicates this is a breaking change for consumers.

**Cost:** Breaking change to one public API method. Minimal given the small scope.

---

### oxc 21bb5d1c1d perf(oxfmt)!: Avoid config pre-scan (#22258)

**Author:** leaysgur
**Situation:** oxfmt was pre-scanning all config files before formatting, which added overhead (600ms on a 50k-directory repo). The change in this PR reduces it from 3.4s to 2.8s.

**Approach:** Remove the pre-scan. Instead, discover valid configs during the walk and abort on error at that point. Files with a valid config get formatted; files encountering an invalid config report the error and abort.

**Mechanism:** CLI behavior changes in niche cases: nested configs + invalid config + write mode now proceeds to format files where a valid config applies, rather than pre-scanning and aborting entirely. This is a behavioral change but unlikely to affect most users.

**Scale implications:** Config discovery is a significant overhead for large repos. The pre-scan was O(n) in config files regardless of how many files actually needed formatting. Removing it shifts config validation to lazy discovery during the walk.

**Cost:** 7 files changed. CLI behavior change for a specific edge case (documented with [!WARNING]).

---

### oxc 2fd907d194 perf(formatter): sort imports during IR construction (#22065)

**Author:** overlookmotel
**Situation:** Import sorting ran as a separate pass after the entire file's IR was generated, requiring a full IR copy to shuffle elements. The transform had to search the entire IR rather than a small section.

**Approach:** Move import sorting to happen during AST-to-IR conversion. Sort operates on IR but at the end of the buffer where the sorting transform only has to deal with small sections of IR containing only import statements.

**Mechanism:** The transform now runs at the end of the IR buffer, so replacing unsorted `FormatElement`s with sorted ones only requires popping and pushing at the buffer end — no shuffling of the entire IR.

**Scale implications:** This is a significant architectural change in the formatter. The separate sorting pass was clean but expensive; inlining it during construction trades simplicity for speed. The trade-off is justified for the perf win.

**Cost:** 7 files changed, 580+ line refactor in `sort_imports/mod.rs`. Subsequent PRs (#22073, #22075, #22204) cleaned up the abstraction.

---

### oxc b0365581a2 refactor(formatter): remove `SortImportsTransform` abstraction (#22075)

**Author:** overlookmotel
**Situation:** After #22065 moved import sorting into IR construction, `SortImportsTransform` served no purpose. The struct + single method was overhead.

**Approach:** Convert `SortImportsTransform` to a free function. Pure refactor, no behavior change.

**Mechanism:** 286 insertions, 294 deletions in `sort_imports/mod.rs`. The structure of the code is changed; the logic is preserved.

**Scale implications:** Abstraction layers that no longer serve a purpose add maintenance burden. Removing them after the architectural shift they enabled is standard debt management.

**Cost:** No behavior change. 1 file.

---

### oxc 5de13ff080 refactor(formatter): `SortImportsTransform::transform` return `Vec` not `Option<Vec>` (#22073)

**Author:** overlookmotel
**Situation:** After #22065, the "return `None` if empty file" case can never fire — `transform` is only called with a segment of IR containing at least one `ImportDeclaration`. The `Option` wrapper was dead code.

**Approach:** Remove the `Option` wrapping. Return `Vec<FormatElement>` directly.

**Mechanism:** 5 insertions, 18 deletions in `sort_imports/mod.rs`. Also fixes a typo in a comment ("fush" → "flush").

**Scale implications:** Removing dead code paths prevents future confusion and reduces branching in hot paths.

**Cost:** No behavior change. 1 file.

---

### oxc f14e81e9a8 perf(formatter/sort_imports): Skip sort for single import runs (#22204)

**Author:** leaysgur
**Situation:** One of the optimizations proposed in #22079. When a run contains only a single import, sorting is a no-op but still runs.

**Approach:** Skip the sort entirely for single-import runs.

**Mechanism:** 9 insertions, 3 deletions in `print/program.rs`. Check at the start of the sort pass.

**Scale implications:** Many source files have only one or two imports. Avoiding the sort overhead for these cases reduces formatter overhead proportionally.

**Cost:** 1 file, 12 lines.

---

### oxc 3c1bb6f704 fix(linter): skip per-node dispatch for run_once-only rules in large files (#22398)

**Author:** Connor Shea
**Situation:** The large-files branch (>200k AST nodes) in `execute_rules` was adding every rule without bucketable AST types to `rules_any_ast_type`, then calling `rule.run` per node on the entire bucket — including `run_once`-only rules whose `run` is the default empty impl. With `--debug timings`, each per-node call incremented `Calls` and added `time()` overhead, inflating "boring rules floor" timings even when those rules had almost no real work.

**Approach:** Gate the else branch on `is_run_implemented()` so `run_once`-only rules don't get dispatched through `run` per node.

**Mechanism:** Before: `unicode-bom` showed 240,002 calls on a 240k-node file. After: 1 call. The fix is a single guard condition.

**Scale implications:** This bug made timing data unreliable for large files. Any file with a large generated/bundled artifact (common in node_modules-containing repos) would make all the cheap rules appear expensive. The `--debug timings` feature was misleading without this fix.

**Cost:** 1 file. Production (non-`--debug`) gets a small win on files >200k nodes from skipping per-node dispatch into empty `run`.

---

### oxc b46d4dee60 feat(linter): add `--debug` options and add per-rule timing info (#22282)

**Author:** camchenry
**Situation:** No way to understand which rules were slow in a given oxlint run without a profiler. Users couldn't submit useful performance data without recompiling and installing additional tools.

**Approach:** Add a `--debug` option accepting comma-separated flags (extensible), and implement `--debug timings` to record and output per-rule timing data. Critical constraint: **when timings are not enabled, it doesn't cost any performance.**

**Mechanism:** `TIMINGS` const generic argument added to all core `run`, `run_on_jest_node`, `run_once` functions in the linter. This creates two versions of each function — one optimized with `TIMINGS = false`, one with `TIMINGS = true`. The runtime `--debug timings` flag selects between them. When disabled, zero overhead. When enabled, 20-30% overhead from bookkeeping in hash maps.

**Scale implications:** The ability to get per-rule timings from a production binary without recompilation is invaluable for debugging performance issues. The `TIMINGS` const generic approach is clean — it avoids branching overhead in the hot path by generating two separate code paths.

**Cost:** Large refactor (many files). 20-30% overhead when timings are enabled, zero otherwise.

---

### oxc 2afef79001 perf(linter): optimize `no-loop-func` (#22491)

**Author:** camchenry
**Situation:** `no-loop-func` looked at **all** function expressions then checked if they were inside loops. On large files, this meant traversing the entire AST. Additionally, some helpers iterated over many nodes to determine context.

**Approach:** Invert the traversal — find loop nodes first, then find functions inside them. This guarantees the AST traversal is bounded by the number of loops (not the number of functions). The rule is now compatible with linter codegen, so it gets skipped in files with no loops.

**Mechanism:** New flow:
1. Find loop nodes
2. Look for functions inside the loop (AST visitor)
3. Check each function in the loop (IIFE/nested function checks, unsafe reference reporting)

On `actualbudget/actual` repository: **410ms → 2ms** for this rule alone.

**Scale implications:** Inverting the traversal is a classic optimization when the "container" nodes are fewer than "contained" nodes. Most files have few loops but many functions — this change makes the rule O(loops) rather than O(functions × context checks).

**Cost:** 345-line refactor in `no_loop_func.rs`.

---

### oxc 08595fbcbf perf(linter): optimize no-unreachable (#22397)

**Author:** camchenry
**Situation:** `no-unreachable` ran expensive DFS path analysis on all files, even those without loop statements.

**Approach:** Skip `no-unreachable` on files without relevant control-flow nodes. Avoid the DFS path for files without loop statements.

Benchmark on `vscode`: **137ms → 54ms**

**Mechanism:** Guard at the rule entry point checks for presence of relevant control-flow nodes before running the expensive analysis.

**Scale implications:** The DFS for unreachable code detection is proportional to the complexity of the control flow graph. Files without loops have simpler CFGs — skipping the expensive path for them is a clear win.

**Cost:** 1 file, 107 lines (89 insertions, 18 deletions).

---

### oxc 4c9ca72b5f perf(oxlint): align walker thread count with rayon pool (#22494)

**Author:** Boshen
**Situation:** The file walker (`ignore::WalkParallel`) used its own default thread count (`available_parallelism()`), independent of rayon's pool size. When users passed `--threads=N` to lower rayon's count, the walker still silently spun up a full background pool, adding kernel pressure.

**Approach:** Bring oxlint in line with oxfmt, which already aligned the walker thread count with rayon's pool size.

**Mechanism:** 2 insertions in `apps/oxlint/src/walk.rs`. On a 72k-file repo (rolldown), M3 Max, 8 runs per config:
- `--threads=14` (default): 3.31s → 3.21s wall, 32.0s → 29.4s sys
- `--threads=6`: 1.36s → 1.28s wall, 5.88s → 3.91s sys (**33% sys-time reduction**)

**Scale implications:** Thread pool oversubscription adds kernel scheduling overhead that doesn't show up in wall-clock time but does show in system time. When users explicitly reduce the thread count (common in CI or resource-constrained environments), the walker shouldn't fight against that choice.

**Cost:** 1 file, 2 lines.

---

### oxc cf86d7ab44 feat(linter): bulk suppression (#19328)

**Author:** Said Atrahouch
**Situation:** ESLint supports suppression comments (`eslint-disable`) but no mechanism for project-wide suppressions tracked in a file. Large teams managing lint debt needed a way to track and update suppressions systematically.

**Approach:** Implement `oxlint-suppressions.json` — a project-wide suppression file. Workers report diffs through a channel; the main thread consumes and applies updates. Uses `Arc` instead of duplicating data into a concurrent hash map (suggestion from @wagenet).

**Mechanism:** `SuppressionManager` in `crates/oxc_linter/src/suppression/` with:
- `tracking.rs`: Loading, saving, updating `oxlint-suppressions.json`
- `mod.rs`: Acts as middleman between oxlint/tsgolint and tracking/diff
- `diff.rs`: Hides diff complexity

CLI args for `create`/`update`/`prune` modes. Shared via `Arc` across workers, not duplicated.

**Scale implications:** ~130 fixture files in the PR. Large-scale adoption by PostHog and other projects validates the approach. Thread-safe sharing via `Arc` rather than concurrent hash map reduces indirection. The feature handles JS plugins and TSGo lint errors.

**Cost:** Large feature — 100+ file changes (mostly fixtures). Significant new surface area in the linter.

---

### oxc c73c159e16 fix(transformer/async-to-generator): reparent parameter initializer scopes (#22507)

**Author:** camc314
**Situation:** `async_to_generator` for async methods with parameter default initializers reused the original method scope as the inner generator scope and moved parameter bindings to the wrapper scope. Scopes created inside parameter defaults were left parented under the generator, even though the transformed AST places those defaults with the wrapper parameters. A fresh semantic rebuild from the transformed AST would see them parented differently.

**Approach:** Update `BindingMover` to reparent direct scopes created inside parameter-side expressions when parameter bindings move to the wrapper. Still stops at function/class scope boundaries.

**Mechanism:** `BindingMover` now traverses one level of parameter default scopes and re-parents them to the wrapper scope. Nested body scopes are not traversed.

**Scale implications:** The semantic mismatch between the transformer's internal state and a fresh semantic rebuild is a class of bugs that affects correctness — subsequent passes that rely on the semantic model would see different scopes depending on whether they rebuilt or reused the transformer-created semantic.

**Cost:** Transformer internal refactor. No public API change.

---

### oxc fb4d98b8e7 refactor(semantic): add `move_binding_by_symbol_id` (#22408)

**Author:** Dunqing
**Situation:** Every call to `move_binding` was paired with `set_symbol_scope_id` to keep `symbol_scope_ids` consistent with the binding map. Callers often needed to hold an arena-lifetime `Ident` across `&mut Scoping` access just to look up the name.

**Approach:** Add `Scoping::move_binding_by_symbol_id(from, to, symbol_id)` that bundles both operations, looking up the symbol's name internally via `Ident`'s precomputed hash (skipping the byte-by-byte hash path).

**Mechanism:** 6 call sites migrated:
- `oxc_semantic/src/binder.rs` (var-hoist Annex B)
- `oxc_transformer/src/common/arrow_function_converter.rs` (`adjust_binding_scope`)
- `oxc_transformer/src/es2017/async_to_generator.rs` (`BindingMover`)
- `oxc_transformer/src/es2018/object_rest_spread.rs` (for-init bindings)
- `oxc_transformer/src/es2026/explicit_resource_management.rs` ×3 (for-of init, static block, try statement)

Also: 23 insertions in `scoping.rs` for the new helper.

**Scale implications:** Batching paired operations into atomic helpers is a common pattern for maintaining consistency. The internal `remove_entry(&name)` using precomputed hash is a micro-optimization that avoids re-hashing when the name is already available in the binding map.

**Cost:** No behavior change. 6 files changed.

---

### oxc 618bc765f0 perf(diagnostics): inline `OxcDiagnosticInner` to avoid heap allocation (#22406)

**Author:** Boshen
**Situation:** `Box<OxcDiagnosticInner>` was added before the removal of `Result<T, Error>` from the parser. It's no longer needed — the boxing was a defensive measure that's no longer warranted.

**Approach:** Inline `OxcDiagnosticInner` directly instead of boxing it. Eliminates a heap allocation on every diagnostic.

**Mechanism:** 17 insertions, 16 deletions in `crates/oxc_diagnostics/src/lib.rs`. `Cargo.toml` gains a new dependency. Allocation snapshots update across parser, semantic, and minifier.

**Scale implications:** Diagnostics are created frequently throughout parsing, linting, and transformation. Every diagnostic that was previously heap-allocated is now stack/inline allocated. This is a fundamental cost reduction in a hot path.

**Cost:** 5 files changed. API unchanged (just implementation).

---

### oxc e431a0eb52 fix(parser): break extends clause loop on fatal error (#22517)

**Author:** Boshen
**Situation:** `parse_extends_clause` kept iterating after `parse_lhs_expression_or_higher` set a fatal error, producing a `TSInterfaceHeritage` with an inverted span (`start > end`). `parse_class` passed that span to `classes_can_only_extend_single_class`, which panicked in `miette` when converting the span to a label (`debug_assert!(self.start <= self.end)` in `Span::size`). Release builds silently produced garbage diagnostics.

**Approach:** Break out of the extends clause loop when a fatal error is set.

**Mechanism:** 3 insertions in `crates/oxc_parser/src/js/class.rs`. Added a fatal-error check after the inner expression parse.

**Scale implications:** Parser recovery on fatal errors must be intentional — continuing to parse after a fatal error can produce malformed AST nodes that downstream passes assume are valid. The inverted span triggered a `debug_assert` in debug builds and silent garbage in release builds. Both are serious.

**Cost:** 3 files, 11 insertions, 1 deletion. Regression test added for the specific case `class extends,{`.

---

### oxc 0f26de6dd2 fix(ecmascript): resolve identifier value type via tracked constants (#22234)

**Author:** Alexander Lichter
**Situation:** `value_type` for `Expression::Identifier` returned `Undetermined` for any non-global identifier, even when the binding had zero writes and a known constant initializer. This caused code like `slot?.()` on a `let slot;` with all writers tree-shaken away not being folded to `void 0`.

**Approach:** Check `GlobalContext::get_constant_value_for_reference_id` for non-global identifier references. Map the resulting `ConstantValue` to a `ValueType` via a new private helper. The lookup is gated by ensuring there are no write references.

**Mechanism:** When an identifier reference has zero writes, the constant value is looked up and mapped to a `ValueType`. Once the identifier resolves to a tracked nullish value, `fold_chain_expr` collapses the chain to `void 0`.

**Scale implications:** Dead code elimination relies on knowing when variables are constant. If `value_type` returns `Undetermined` for variables that are actually constant-initialized, subsequent folds miss opportunities. This fix enables the minifier to understand that a variable initialized to `null` and never reassigned is still `null` at usage sites.

**Cost:** 4 files changed. Matches behavior that other tools (Rollup, esbuild, SWC, Terser in non-minify mode) already have.

---

### oxc e9ec7c6c5f fix(minifier): fold optional chains by base nullishness (#22236)

**Author:** Alexander Lichter
**Situation:** `fold_chain_expr` only operated on the outermost element of a `ChainExpression` and only handled the nullish case. Non-nullish bases like `({})?.foo` were not folded. Multi-level chains like `null?.foo.bar` kept their `?.` because the optional was on an inner element.

**Approach:** Rewrite the fold to walk inward to the deepest (= leftmost in source) optional and act there. A `ChainFoldResult` enum encodes three outcomes: `None` (no fold), `Flipped` (non-nullish base, `optional` flag set to `false`), and `Collapse` (nullish base, chain short-circuits to `void 0`, preserving side effects).

**Mechanism:** Walks the chain expression inward to find the deepest optional element before deciding. Preserves side effects of the base via sequence expressions.

**Scale implications:** Optional chaining is extremely common in TypeScript codebases. The previous implementation missed many fold opportunities, leaving dead `?.` operators in the output. The new implementation covers:
- `null?.foo.bar` → `void 0` (nested nullish)
- `({})?.foo` → `({}).foo` (non-nullish)
- `[]?.foo.bar` → `[].foo.bar` (nested non-nullish)
- `(() => 0)?.()` → `0` (chains with IIFE inliner)

**Cost:** 3 files. 501/501 minifier tests pass. Idempotent under `--twice`.

---

### oxc 702b14e808 fix(minifier): preserve IIFE structure in DCE-only mode (#22547)

**Author:** Dunqing
**Situation:** In DCE-only mode (rolldown's per-module preprocess), `substitute_iife_call` inlined `const x = /* @__PURE__ */ (() => [1, 2, 3])()` down to `const x = [1, 2, 3]`, dropping the `@__PURE__` annotation downstream tree-shakers rely on. Previous `iife_inline_would_lose_pure` only bailed for side-effecting bodies, so array/object/primitive bodies still got inlined.

**Approach:** `try_take_iife_body` bails on `ctx.state.dce` directly. Remove the `iife_inline_would_lose_pure` helper. DCE-only mode is identified by `state.dce = true` set only for `dead_code_elimination[_with_scoping]` entry points.

**Mechanism:** Cross-tool comparison shows oxc DCE-only was the only mode inlining pure IIFEs (Rollup, esbuild, SWC, Terser all preserve them in non-minify mode). Fix aligns behavior.

**Scale implications:** Rolldown relies on `/* @__PURE__ */` annotations to determine what can be tree-shaken. When the minifier strips the annotation from the IIFE structure, downstream tools lose the information. This is a correctness fix for the Rolldown integration path.

**Cost:** 3 files. 501/501 minifier tests pass. `minsize`/`allocs` snapshots unchanged.

---

### oxc a15be790bf feat(bench): add kitchen-sink.tsx to TestFiles (#22609)

**Author:** Dunqing
**Situation:** The existing bench input set didn't reliably surface general-purpose perf wins above the ~1-2% measurement noise floor. Several recent perf PRs (#22580, #22594, #22596, #22599, #22603) showed no measurable improvement on the old set.

**Approach:** Add `kitchen-sink.tsx` — a comprehensive synthetic TypeScript+JSX fixture exercising every AST node, every transformer plugin, every minifier optimization, and every semantic step in one large file (21,117 lines, 732.90 kB, ~133,000 AST nodes).

**Mechanism:** Fixture maintained at `oxc-project/benchmark-files`. Added to both `TestFiles::minimal()` (bench input set) and `TestFiles::complicated()` (alloc-tracking input set). Verified by re-benching #22596: **minifier mean −1.5%, min −3.7%** — above noise, signal confirmed.

Also fixes `SourceCleaner` missing `visit_ts_template_literal_type` — type-level template literals were being lexed as value-level, causing spurious errors.

**Scale implications:** Better benchmark coverage means perf regressions are less likely to slip through. The kitchen-sink exercises the full pipeline rather than specific hot paths, making it a better indicator of general perf work.

**Cost:** 7 files changed. Snap baselines updated with kitchen-sink row across all 5 pipelines.

---

### oxc 0440b0f060 feat(linter/eslint): implement `id-match` rule (#22379)

**Author:** Vladislav Sayapin
**Situation:** The `eslint/id-match` rule (enforces configured naming regex for identifiers) wasn't implemented in oxlint. Issue #479 tracked this.

**Approach:** Implement the rule with intentional stricter behavior in two cases:
- Computed destructuring keys (`const { [bad_name]: x } = obj`) are checked (ESLint doesn't)
- Ordinary top-level dynamic import option keys with `properties` enabled are checked

**Mechanism:** Hot path limited to identifier-like AST nodes (`BindingIdentifier`, `IdentifierReference`, `IdentifierName`, `PrivateIdentifier`, `LabelIdentifier`). Regex check runs first and returns immediately on match. Non-default configurations add ancestor/context checks only for failed matches. Default pattern `^.+$` matches everything, so `should_run` skips work for default configuration.

**Scale implications:** Regex matching is expensive if run on every identifier. The optimization of returning immediately on match and only doing expensive work on failure is key to keeping this rule fast.

**Cost:** Multi-file change (generated code, rule implementation, snapshots). AI assistance disclosed.

---

### oxc 1884833279 feat(linter/plugins): implement `SourceCode.getDisableDirectives` method (#21029)

**Author:** Nicolas Le Cam
**Situation:** `eslint-plugin-unicorn` uses `SourceCode.getDisableDirectives` — but oxlint's JS plugin implementation didn't provide it, causing crashes when unicorn rules tried to access it.

**Approach:** Implement `getDisableDirectives` in the JS plugin `SourceCode` object.

**Mechanism:** `apps/oxlint/src-js/plugins/directives.ts` (84 lines) and `source_code.ts` (4 lines). Plugin provides the method that ESLint marks as optional but some plugins require.

**Scale implications:** JS plugin compatibility with popular ESLint plugins is critical for adoption. This was a blocker for users trying to run unicorn through oxlint's plugin interface.

**Cost:** 4 files. Small, focused fix.

---

### oxc fe7194dee9 feat(oxlint): add agent output mode (#21955)

**Author:** Jovi De Croock
**Situation:** Running oxlint from an AI agent needs minimal token usage. The default output format includes summaries and verbose formatting that isn't necessary for programmatic consumption.

**Approach:** Add `--format agent` option with one-line diagnostics and no summary. Include diagnostic help text inline when available.

**Mechanism:** New `agent.rs` output formatter module. Single-line diagnostic format. No summary line.

**Scale implications:** AI agents parsing lint output benefit from compact, machine-readable formats. The agent format reduces token count per lint run, which matters when running on large codebases or in CI.

**Cost:** 5 files. Small addition.

---

### oxc ea0380c189 feat(linter/unicorn): implement `import-style` rule (#22173)

**Author:** Hao Chen
**Situation:** Part of #684 (implementing unicorn rules). The `import-style` rule wasn't implemented.

**Approach:** Full implementation of the unicorn `import-style` rule, passing all upstream tests.

**Mechanism:** 968-line rule implementation. Generated code updates (config, rule runner, rules enum). Snapshot added.

**Scale implications:** Unicorn rule coverage continues to expand. 968 lines for one rule indicates this rule has substantial complexity.

**Cost:** 8 files, 543 lines in snapshot alone. AI co-authored.

---

### oxc 5fa47746a3 feat(linter/n): implement `callback-return` rule (#22470)

**Author:** Mikhail Baev
**Situation:** The `n/callback-return` rule (issue #493) wasn't implemented.

**Approach:** Implement the rule, passing all upstream tests.

**Mechanism:** Generated code updates, rule implementation in `node/callback_return.rs`.

**Scale implications:** Node.js rules coverage expands.

**Cost:** 8 files changed. AI co-authored.

---

## Architectural Patterns

### Arena Allocation Strategy

oxc's allocator uses arena allocation as the primary strategy for AST nodes and semantic data. The recent Windows fix (#22124) exemplifies the platform-specific complexity this introduces:

- **Linux**: Overcommit accounting means virtual memory allocation is "free" until pages are touched. Direct high-alignment allocation works.
- **macOS**: System allocator refuses 4 GiB-aligned requests. Must overallocate and use a sub-range.
- **Windows**: Commits immediately against memory limits. Must use `VirtualAlloc` with reserve/commit separation.

The arena abstraction (`Arena::new_fixed_size`, `Arena::grow_fixed_size_chunk`, `dealloc_fixed_size_arena_chunk`) has platform-specific implementations. This is a recurring pattern — when the underlying OS memory model differs significantly, platform-specific code is the only clean solution.

### Performance Measurement Discipline

oxc demonstrates exceptional measurement discipline:
- **CodSpeed** for continuous instruction-count benchmarking
- **criterion** for local wall-clock benchmarking
- **cargo allocs** for memory allocation tracking
- **`--debug timings`** for per-rule profiling in production binaries

The key insight is that measurement must be:
1. **Continuous** (CodSpeed on every PR)
2. **Instruction-level** (not just wall-clock, to avoid noise)
3. **Platform-aware** (different platforms have different characteristics)
4. **Actionable** (the kitchen-sink.tsx addition shows they recognized their bench set wasn't sensitive enough to detect real wins)

### Linter Codegen and Node Type Inference

A recurring pattern in linter performance work is "populate node types" — refactoring rules so their `run` method starts with a `match node.kind()` guard so `linter_codegen` can infer which AST nodes the rule needs rather than falling back to "every node." This optimization (seen in #22600, #22601, #22602) reduces the nodes a rule is dispatched on.

The `TIMINGS` const generic pattern (#22282) is notable for achieving zero-overhead profiling — rather than runtime-branching on a boolean, the compiler generates two complete code paths selected at compile time.

### Transformer Semantic Consistency

The `async-to-generator` reparenting fix (#22507) illustrates a class of bugs where the transformer's internal semantic state diverges from what a fresh semantic rebuild would produce. The `move_binding_by_symbol_id` helper (#22408) is part of addressing this — bundling `move_binding` + `set_symbol_scope_id` prevents the pair from getting out of sync.

### Formatter IR-Centric Architecture

The formatter's shift from separate sorting pass (#22065) to during-IR-construction sorting shows an architectural evolution:
1. Initial separate pass (clean but expensive)
2. Middle-way approach (during IR construction, at buffer end)
3. Abstraction cleanup (remove `SortImportsTransform`, simplify return types)

The formatter operates on an IR (Intermediate Representation) rather than the AST directly. Import sorting happens on the IR, which requires careful handling of comments (must be output in source order for AST→IR conversion to work correctly).

## Engineering Principles Observed

1. **Zero-overhead abstractions for profiling**: The `TIMINGS` const generic shows you can add profiling infrastructure without runtime cost when the approach is sound.

2. **Benchmark fidelity matters**: Adding kitchen-sink.tsx because existing benches couldn't detect real perf wins proves that measurement tooling must be validated, not just assumed correct.

3. **Platform-specific code is sometimes unavoidable**: The Windows allocator changes (3 PRs: #22088, #22122, #22123, #22124) show that when OS memory models differ fundamentally, abstraction layers with platform implementations are the right answer.

4. **Cross-tool behavior alignment**: The DCE-only IIFE fix and the optional chain folding both cite cross-tool comparisons (Rollup, esbuild, SWC, Terser) as validation. oxc doesn't just implement features — it validates against established tools.

5. **Semantic model consistency is hard**: Transformer passes that modify AST and then rely on semantic state face a recurring challenge: ensuring the transformer's semantic view matches what a fresh rebuild would produce. This is a source of subtle correctness bugs.

6. **Breaking changes are marked clearly**: The `!` marker in commit messages (`feat(allocator)!:`) signals breaking API changes, making them auditable.

7. **AI disclosure is consistent and meaningful**: Nearly every complex commit includes an AI usage disclosure. The project has normalized AI assistance as part of the development process rather than hiding it.