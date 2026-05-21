# Rolldown — Commit Insights

Rolldown is a JavaScript bundler written in Rust, designed to be a fast, standards-compliant replacement for Rollup with compatibility for the Vite ecosystem. It went from initial commits to a 1.0.0 release over ~2 years, absorbing massive real-world usage from the Vite/RSC ecosystem.

This analysis covers ~50 substantive commits from 2023–2026, sampling across performance work, architectural refactors, bugfixes, and feature additions.

---

## Key Architectural Themes

### 1. The NAPI Binding Deadlock Problem Is Pervasive

The most dangerous class of bugs in Rolldown is structural deadlocks caused by sync NAPI bindings calling into async Rust code on the JS thread.

**Commit `6f6cf5aa0` — `this.emitFile` deadlock fix**

**Author:** Viktor Lázár (lazarv1982@gmail.com)

**Situation:** Plugins that call `this.emitFile({ type: 'chunk' })` from a `transform` hook would cause Rolldown to hang indefinitely under scale (~400+ emits under parallelism, ~1025+ in a tight loop). No error, no stack trace — the build simply stops. The root cause was identified in a prior audit (#7311) but the fix was delayed.

**Approach:** Diagnosed via reduced repro: a React Server Components plugin building a Mantine-sized app (~3300 `"use client"` modules, each emitting a chunk). Under parallelism, the module loader's bounded `mpsc::channel(1024)` fills up while the JS thread is parked inside `block_on` waiting for the loader to drain the channel — but the loader is blocked waiting for the JS thread to service the TSFN callbacks for prior emits. Classic producer ⇄ consumer deadlock through the ThreadsafeFN boundary.

**Mechanism:** The fix collapses the entire `emit_chunk` path from async to sync. The `FileEmitter::emit_chunk` becomes a plain `fn` (not `async fn`), using `std::sync::Mutex<Option<UnboundedSender<...>>>` instead of `tokio::sync::Mutex`. The unbounded sender means `send()` is infallible — no `.await` that can park the thread. The `PluginDriver.tx` is unified on `std::sync::Mutex` for consistency. The bounded channel is replaced with `unbounded_channel()` as defense-in-depth: even if future code reintroduces a sync wait on this path, there's no `.await` that can block the JS thread.

**Scale implications:** This was not an "insanely large build" problem. 3300 client components is a normal-sized RSC app. The effective channel capacity under parallelism was much smaller than 1024 because in-flight tasks from prior emits were already waiting on TSFN responses from the blocked JS thread. The bug was structural — the primitive didn't survive its documented usage at scale.

**Cost:** Zero API surface change. `this.emitFile` remains synchronous and returns `string` directly, matching Rollup's contract. All existing tests pass.

---

### 2. Chunk Optimization Is a Dense Field of Interacting Constraints

The chunk optimizer is where most of the "hard to get right" code lives. Circular dependency detection, runtime placement, facade elimination, and tree-shaking interact in non-obvious ways.

**Commit `f4e60c928` — Reduce false positives in circular dependency detection**

**Author:** Alon Mizrahi (alonmiz1234@gmail.com)

**Situation:** PR #8371 fixed a real crash (`__commonJSMin is not a function`) by improving circular dependency detection in the chunk optimizer. But the fix over-approximated — it blocked many legitimate merges because the BFS started from `target.deps` as well as `source.deps`, and the post-merge edge simulation was too aggressive. In a large app with ~2,200 entry points, this blocked **1,046 legitimate merges**, producing **+1,224 extra chunks** and **+8% bundle size**.

**Approach:** The algorithm was split into two cases:
- **Source has deps:** Only BFS from `source.deps` without post-merge simulation. If target is reachable from source's dependency tree, merging would create a real cycle. Sufficient.
- **Source has no deps** (runtime chunk): Keep the full algorithm with `target.deps` BFS and post-merge simulation. Needed for the specific cycle pattern `target → chunk_A → source(=target after merge)` from #8361.

**Mechanism:** The BFS from `target.deps` was removed for the source-has-deps case. Post-merge simulation (which redirects source-dependent chunks back to target) was also dropped — it was the source of the false positives.

**Scale implications:** -25% JS files, -12.5% bundle size on a 2,200-entry app. This is a real production impact.

---

**Commit `31d040304` — Prevent chunk optimizer from creating import cycles (#9228)**

**Author:** IWANABETHATGUY (iwanabethatguy@qq.com)

**Situation:** The `f4e60c928` fix above introduced a regression. In the `source has deps` branch, the BFS starts from `source.deps` and dropped the "post-merge alias step" that models new back-edges created by retargeting `source`'s importers onto `target`. Without that step, the BFS cannot see cycles where the closing edge is one of those back-edges.

**Approach:** Full revert of `would_create_circular_dependency` to the pre-#9049 unified algorithm: BFS from `source.deps ∪ target.deps`, and when visiting a chunk `c` where `source ∈ c.dependencies`, also enqueue `target` (modeling the post-merge edge `c → target`). If `target` is dequeued, merge creates a cycle.

**Mechanism:** The key insight: the `issues/9049` fixture that #9049 was supposedly fixing was actually fixed by a different PR (#9085 — `fix: relax overly conservative side-effect leak check in chunk optimizer`). So reverting #9049 doesn't break the test — #9085 is what actually fixed it.

**Scale implications:** The chunk optimizer correctly handles complex dependency graphs now. The false-positive problem from #9049 was real but not exercised by any in-tree test. A follow-up can re-tighten the check without losing coverage.

---

**Commit `0b257a924` — Implement dynamic dominator merge logic**

**Author:** Alexander Lichter (github@lichter.io)

**Situation:** The chunk optimizer handles shared modules between a user entry and its dynamic descendants correctly (entry is always loaded first). But it didn't handle shared modules between **two dynamic chunks** where one dynamically imports the other. The `find_merge_target` only inspected static importer sets, so found nothing.

**Approach:** Added a dominator check to `find_merge_target` — dynamic importers can be considered as merge targets as long as they "dominate" the entry chunks needing the shared module (meaning they're guaranteed to load before those chunks).

**Mechanism:** Dynamic chunks get an entry index just like user entries. When a module's bitset shows it's shared between two dynamic chunks where one imports the other, the optimizer now picks the dominator as the merge target. If no dominator exists, it falls back to emitting a separate common chunk.

---

### 3. Tree-Shaking Is Fundamentally About Side Effect Propagation

Most tree-shaking bugs stem from incorrect reasoning about when side effects propagate through the module graph.

**Commit `b235a3865` — Module should consider having side effect if its dependencies have side effect**

**Author:** Yunfei He (i.heyunfei@gmail.com)

**Situation:** If a module's dependencies have side effects, the module itself must be considered as having side effects — but the original code didn't propagate this correctly. A module `main.js` importing `proxy.js` which imports `indirect-side-effect.js` (which has a side effect like logging) would incorrectly tree-shake `main.js`.

**Approach:** Side effect status propagates transitively through the import graph. The fix ensures that when computing whether a module should be included, its entire dependency chain's side effect status is considered.

---

**Commit `100317476` — Determine indirect side effects**

**Author:** IWANABETHATGUY (iwanabethatguy@qq.com)

**Situation:** Similar to the above but more nuanced — indirect side effects (modules that have side effects through a chain of pure intermediates) weren't being correctly determined.

**Approach:** The fix refines how side effect state is tracked in the module graph, ensuring that the transitive closure of side effects is correctly computed.

---

### 4. CJS Interop Has Many Subtle Failure Modes

CommonJS/ESM interop is where most correctness bugs hide. The combination of `module.exports` reassignment, `exports.xxx` assignments, dynamic imports, and IIFE/UMD wrapping creates a large state space.

**Commit `7bcb2e00d` — Skip inlining stale CJS export constants on module.exports reassignment**

**Author:** IWANABETHATGUY

**Situation:** When a CJS module uses both `exports.foo = 1` and `module.exports = { foo: 2 }`, the `exports.foo` constant was incorrectly inlined as `1` instead of preserving the runtime property access yielding `2`. The inlining optimization saw `exports.foo` as a constant and substituted it, but `module.exports = { ... }` replaced the entire `exports` object at runtime.

**Approach:** Fine-grained detection of `module.exports` reassignment during scanning:
- **Object literal RHS** (`module.exports = { foo, bar }`): only invalidates `exports.xxx` constants whose names overlap with the object's static properties. Non-conflicting constants are still eligible for inlining.
- **Non-analyzable RHS** (function call, variable, computed keys, spread): invalidates all `exports.xxx` constants.

**Mechanism:** Uses a `ModuleExportsReassignment` enum (`None` / `KnownProps(set)` / `Unknown`) tracked during the AST scan. This feeds stale symbol IDs into the existing `bailout_inlined_cjs_exports_symbol_ids` set.

---

**Commit `fb007e435` — Wrap CJS entry modules for IIFE/UMD when using exports/module**

**Author:** IWANABETHATGUY

**Situation:** CJS entry modules that reference `exports` or `module` were not wrapped with `__commonJSMin` for IIFE/UMD output formats, causing `exports is not defined` runtime errors.

**Approach:** Added wrapping condition in `determine_module_exports_kind` so CJS entries using `exports`/`module` get properly wrapped for IIFE/UMD. Side-effect-only `.cjs` entries (no `exports`/`module` usage) continue to be inlined directly without wrapping.

---

### 5. The oxc Dependency Is Central to Performance

Rolldown is built on `oxc` (the Rust parser/linter/formatter project). Many performance wins come from upgrading oxc or using its APIs more efficiently.

**Commit `6fb48119b` — Enable mimalloc v3 to reduce idle memory**

**Author:** IWANABETHATGUY

**Situation:** The binding layer was using the default system allocator, which has higher overhead for the patterns Rolldown exhibits (many small allocations, frequent frees).

**Approach:** Enabled mimalloc v3 in the Rust binding. mimalloc uses a hybrid approach with a central allocator and per-thread heaps, reducing contention and overhead for these allocation patterns.

---

**Commit `a52d258f6` — Const enum cross-module inlining support**

**Author:** Dunqing

**Situation:** Cross-module enum inlining was a known missing feature compared to esbuild. TypeScript's `const enum` is commonly used for compile-time constants in large codebases.

**Approach:** The pipeline extracts enum member values from oxc's `Scoping` before the transformer converts enums to IIFEs, stores them on `EcmaView` as `enum_member_value_map`, then inlines accesses in the scope-hoisting finalizer:
- `Direction.Up` → `0` (dot notation)
- `Direction["Up"]` → `0` (bracket notation)
- `ns.Direction.Up` → `0` (namespace chains)

Dead enum declarations are tree-shaken when all references are inlined.

**Scale implications:** Cross-module const enum inlining can eliminate significant overhead in TypeScript codebases. The feature also handles regular enums when all members have statically known values.

---

### 6. Facade Elimination Interacts With Runtime Placement

When facade entry chunks are eliminated (replaced with direct imports to the modules they facade), new runtime helper consumers can be introduced, which interacts with the runtime module's placement in non-obvious ways.

**Commit `b254f932f` — Prevent circular runtime helper imports during facade elimination**

**Author:** IWANABETHATGUY

**Situation:** `optimize_facade_entry_chunks` runs after the merge phase, which has already placed the runtime module into some chunk based on bitset assignment. When facade elimination calls `include_symbol(namespace_object_ref)`, it sets `target_chunk.depended_runtime_helper.insert(ExportAll)` and adds the target to `runtime_dependent_chunks`. If the target chunk transitively reaches the chunk currently hosting the runtime, the new helper-import edge closes a cycle.

**Approach:** Two-step decision:
1. **Peel gate:** Runtime is peeled out of its host chunk only when `host_has_other_modules` AND `has_external_consumer`. If runtime is alone in its chunk, that chunk is a leaf with no outgoing imports and cannot participate in a cycle.
2. **Placement decision:** If `consumer_chunks.len() == 1`, runtime moves into that single consumer. If `> 1`, runtime gets its own dedicated `rolldown-runtime.js` leaf chunk with zero outgoing edges.

**Mechanism:** The `consumer_chunks` set is the union of chunks with non-empty `depended_runtime_helper` from the linking stage AND `runtime_dependent_chunks` announced by facade elimination. Deduplication via `FxHashSet`.

---

### 7. Iteration Over Recursion to Prevent Stack Overflow

A recurring pattern in recent fixes: converting recursive traversal to iterative to handle deep or circular module graphs.

**Commit `a611dbbe1` — Convert `generate_transitive_esm_init` to iterative**

**Author:** IWANABETHATGUY

**Situation:** The recursive `generate_transitive_esm_init` would stack overflow on deeply nested or circular module graphs. This was exposed by #8979 (which prevents merging common chunks into side-effectful entry chunks, causing more modules to land in separate chunks).

**Approach:** Replace recursive traversal with explicit stack-based iteration. Also simplifies deduplication by using `insert`'s return value instead of separate `contains` + `insert` calls.

---

**Commit `1ae1584f0` — Prevent stack overflow in `generate_transitive_esm_init` on circular dependencies**

**Author:** dalaoshu

**Situation:** The visited-set guard (`generated_init_esm_importee_ids`) only existed in branch 1 (importee in same chunk → emit init call, record in visited). When circular dependencies exist and modules are in different chunks, branch 2 (importee not in same chunk → recurse) recurses infinitely: A → B → A → B → ...

**Approach:** Move the visited-set check before the branch, so it guards both paths. The fix is simple but critical — the guard must precede the recursion, not just the emission.

---

### 8. DevTools / Package Graph as First-Class Feature

Recent commits show significant investment in devtools — understanding the package graph, detecting duplicate packages, mapping packages to modules/chunks. This is tied to the Vite+RSC ecosystem's needs.

**Commit `2b235157c` — Write devtools logs on a background thread**

**Author:** (perf commit)

**Situation:** Devtools logging was synchronous on the main thread, adding overhead to each logging call.

**Approach:** Offload logging to a background thread with a dedicated channel. Non-blocking for the main build path.

---

### 9. Safe Code Is Preferred Over Fast Code at the Binding Layer

A consistent theme: when a sync NAPI binding requires accessing shared mutable state, the team prefers to make the underlying Rust code sync-safe (using `std::sync::Mutex` instead of `tokio::sync::Mutex`, using `UnboundedSender` instead of bounded channels) rather than making the binding async.

**Commit `4a2ac1124` — Drop unsafe napi string helper, hoist transform ArcStr**

**Author:** (refactor commit)

**Situation:** Some NAPI bindings were using `unsafe` code to work around sync restrictions on string handling.

**Approach:** Remove the unsafe code path by ensuring the Rust side properly owns its strings and the NAPI boundary is safe.

---

### 10. Config Semantics Must Be Tight

**Commit `5ac3e69b7` — Default unspecified inlineConst.mode to smart**

**Author:** IWANABETHATGUY

**Situation:** When `optimization.inlineConst` was a partial config object with no `mode` field (e.g., `{ pass: 1 }`), it silently fell back to `'all'` instead of the documented default `'smart'`. This was a semantic mismatch between documented and actual behavior.

**Approach:** Both the NAPI binding and the Rust normalizer are now aligned: unspecified `mode` defaults to `'smart'`. This is a breaking change for users who relied on the implicit `'all'` fallback.

---

## Patterns Observed

### What Triggers Changes

- **Production bugs at scale:** The most impactful fixes come from real-world usage at scale (2,000+ entry points, 3,000+ modules). Internal test suites don't expose the combinatorial explosion of interactions.
- **Ecosystem pressure:** RSC plugins, Vite usage patterns, TypeScript's type system features (const enums, namespaces) all drive specific fixes.
- **Oxidation:** As oxc evolves, new capabilities (const enum inlining, mimalloc v3) enable performance features.

### How Problems Are Diagnosed

- **Reduced repros are prized:** Many commits include minimal repros that isolate the bug. The `emitFile` deadlock was reproduced in 30 lines of plugin code.
- **Bisecting to find the real fix:** The `31d040304` commit shows sophisticated reasoning: the `issues/9049` fixture was named after PR #9049, but bisection revealed the actual fix was PR #9085. The commit message is a model of clarity about this.
- **Test infrastructure is comprehensive:** Integration tests with runtime assertions, fixture-based snapshot tests, and regression tests for specific issues all exist. The test suite has 1,600+ integration tests.

### Where Abstraction Is Refused

- The chunk optimizer is complex by necessity. The attempt in #9049 to simplify the circular dependency check by dropping cases actually reintroduced bugs.
- `std::sync::Mutex` over `tokio::sync::Mutex` for non-IO data — the team prefers the simpler abstraction when it suffices.
- The module tag system uses a `u64` bitset rather than a more complex abstraction.

### Where Abstraction Is Added

- `ModuleTagBitSet` for the tag system — simple, fast, `Copy`.
- `ModuleExportsReassignment` enum with `None / KnownProps(set) / Unknown` — fine-grained tracking of CJS module state.
- `FileEmitter` as a central place for async-safe file emission.

### What Breaks at Scale

- **Bounded channels under parallelism:** The `mpsc::channel(1024)` pattern is dangerous when the consumer is blocked waiting for producers who are blocked waiting for the consumer.
- **Recursive traversal:** Deep or cyclic graphs cause stack overflow in recursive implementations.
- **Over-approximations:** Circular dependency detection that starts from too many nodes or simulates too many edges produces false positives that block legitimate optimizations.

### What DevX Costs

- The Rust+Javascript hybrid architecture (Rust core + NAPI bindings + TypeScript package) means changes often need to touch multiple layers.
- Oxc upgrades are frequent and sometimes require adaptation (the `oxfmt` migration was a multi-commit refactor).
- The `justfile`-based CLI is convenient for developers but requires learning a project-specific tool.

---

## Engineering Principles to Extract

1. **Sync-safe NAPI bindings require the entire Rust call chain to be sync.** If any `.await` point exists in the path from the binding to the result, you have a deadlock risk on the JS thread. Use `std::sync::Mutex`, `UnboundedSender`, and explicit async only where truly necessary.

2. **Chunk optimization is a system of interacting constraints.** Changing one heuristic (circular dependency detection) can break another (facade elimination). Systems with multiple merging strategies need careful modeling of all interaction points. Prefer broad integration tests over isolated unit tests for this layer.

3. **Side effect propagation is the hard part of tree-shaking.** Getting module-level side effects wrong produces incorrect bundles. The transitive closure matters — a module importing a pure module that imports a side-effectful module is not pure.

4. **CJS interop has complex state.** The combination of `exports.xxx`, `module.exports = ...`, and `module.exports = fn()` creates cases where static analysis must be conservative. Fine-grained tracking of what was reassigned and how helps preserve optimization opportunities.

5. **Recursive traversal will eventually hit a deep or cyclic graph.** Prefer iterative with explicit stack. This is not premature optimization — it's correctness.

6. **Production scale reveals what test scale misses.** Build tests with 2,000+ entry points. Real plugin patterns (RSC, Vite) exercise edge cases that synthetic tests don't.

7. **Oxidation enables features.** The oxc dependency is not just infrastructure — it's a source of capability. Const enum inlining, mimalloc upgrades, semantic builder improvements all come from the oxc ecosystem.

---

*Analysis covers commits from 2023-01-01 to 2026-05-21. Sampled ~50 substantive commits out of ~6,900 total. Skipped: version bumps, dependency updates, trivial typo fixes, automated lint/formatting commits.*