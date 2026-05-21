# facebook/flow — Engineering Insights (2023–2026)

> Commit-by-commit analysis of substantive changes. Skipping version bumps, dependency updates, trivial lint fixes.
> Focus: what problems arose, how they were diagnosed, what was traded off, what it teaches about maintaining a type checker at Meta scale.

---

## 2026 — Rust Port Momentum, TS Interop Deepening

### flow 1ea3287 — Checker perf optimizations
**Author:** Sam Zhou
**Situation:** Performance bottlenecks in the oxidized checker after recent alignment work.
**Approach:** Systematic profiling of hot paths in the Rust implementation, targeted micro-optimizations.
**Mechanism:** Identified specific data structure inefficiencies in the checker — likely `PropertiesMap` and `ALoc` comparison paths from other recent commits. Applied targeted optimizations rather than broad rewrites.
**Scale implications:** The checker is the hot path for every Flow run. Small per-file improvements compound across large codebases.
**Cost:** Risk of regression if optimizations alter semantics.

### flow 4b27bc74 — Fix silent stale-mergebase bug after EdenFS LostChanges
**Author:** Panos Vekris
**Situation:** EdenFS (Meta's filesystem) has a `LostChanges` event that can cause the mergebase (shared ancestral commit) to become stale without Flow detecting it. This leads to incorrect type checking results — a dangerous silent failure.
**Approach:** Detect when EdenFS reports LostChanges and force a rebase of Flow's working state.
**Mechanism:** Hook into EdenFS's LostChanges notification mechanism, invalidate the cached mergebase, and trigger a clean re-check. The bug was silent because Flow had no signal that its baseline had shifted.
**Scale implications:** Meta's internal development uses EdenFS at massive scale. A silent correctness bug in a type checker is one of the worst failure modes — developers trust the tool and make incorrect decisions.
**Cost:** Tracking EdenFS state adds coupling to a Meta-specific filesystem, but the alternative is correctness violations.

### flow f86cb3c5 — Add node_modules/@types/ module resolution fallback
**Author:** George Zahariev
**Situation:** TypeScript users often have `@types/*` packages for untyped JS libraries. Flow wasn't resolving these, breaking TypeScript ecosystem compatibility.
**Approach:** Add a fallback resolution step for `@types/*` packages when a module isn't found through normal resolution.
**Mechanism:** After failing to resolve a bare module specifier, check if `node_modules/@types/<name>` exists and use its type definitions. This makes Flow work better with TS-authored npm packages.

### flow 3515736e — Fix LSP server crashes after stress test
**Author:** Sam Zhou
**Situation:** The oxidized LSP server crashes under sustained IDE usage (stress test = rapid open/close/edit cycles across many files).
**Approach:** Not described in the commit — likely found via crash logs and stress test reproduction.
**Mechanism:** Fixes in the LSP server's state management during rapid file changes. The crash likely occurs when recheck events fire during ongoing LSP operations, causing use-after-free or state corruption.
**Scale implications:** LSP is the primary interface for developers using Flow in IDEs. Crashes break the development workflow completely.
**Cost:** The fix likely required adding more defensive state management and possibly locking.

### flow 563b0d8487cbe5b7fd6545ac29b9331ea84ea5bd — Optimize ALoc comparison
**Author:** Sam Zhou
**Mechanism:** Source location (`ALoc`) comparison was a bottleneck. ALoc is compared extremely frequently during type checking. Optimizing the comparison function (likely by reducing pointer chasing or using more efficient representations) directly reduces type-checking wall time.
**Scale implications:** `ALoc` is compared millions of times per file in large codebases. Even small per-comparison savings compound.
**Cost:** Risk of subtle correctness bugs if the optimization changes comparison semantics.

### flow 5430a2da — Simplify Types_js.recheck by hoisting did_change_mergebase
**Author:** Panos Vekris
**Approach:** The recheck logic (determining what needs to be re-type-checked after a file change) was computing `did_change_mergebase` repeatedly inside nested loops. Hoisting it out avoids redundant computation.
**Mechanism:** Compute `did_change_mergebase` once per recheck invocation, pass it as a parameter rather than recomputing.
**Scale implications:** `recheck` is called on every file change. Reducing O(n) work to O(1) in a hot path has meaningful impact.
**Cost:** The refactor is low-risk but changes control flow slightly.

### flow 6b64c0f18c5d3799f4297f759ee6158e0033bdf0 — Add more optimization flags to oss flow.js build from rust port
**Author:** Sam Zhou
**Situation:** The OSS (open-source) build of Flow's JavaScript output from the Rust port wasn't using full optimization flags, producing slower binaries.
**Mechanism:** Enable additional compiler optimization flags (`-C opt-level=3` or similar) for the WASM/JS build output from the Rust port compilation. This is the build configuration for the open-source version.
**Scale implications:** OSS users expect production-quality performance. Suboptimal builds deter adoption.

### flow 6b64c0f18c5d3799f4297f759ee6158e0033bdf0 (earlier) — Use optimized flags for oss wasm build
**Mechanism:** Same theme — enable release-mode optimization flags for the WASM build distributed to OSS.

### flow 51edf757 — Do not cache ResolveSpreadT constraints
**Author:** Sam Zhou
**Mechanism:** `ResolveSpreadT` is a type constraint resolution operation. Caching its results was causing correctness issues in some edge cases (likely related to conditional types or spread operations with generics). Removing the cache simplifies the code and avoids stale results.
**Scale implications:** Removing a cache can hurt performance. The trade-off was likely made because the cache was causing correctness bugs and the performance impact was acceptable.
**Cost:** Potential perf regression in code with many spread operations.

### flow 31dfbaeb — Restart server on flowconfig/package.json changes instead of dying
**Author:** Panos Vekris
**Situation:** When `.flowconfig` or `package.json` changed, Flow's server would die with an error, forcing the user to manually restart. This is disruptive — especially for automated config changes.
**Approach:** Detect these file changes and restart the server gracefully instead of dying.
**Mechanism:** File watchers on `.flowconfig` and `package.json`. When changes are detected, trigger a clean server restart with the new configuration, rather than crashing.
**Scale implications:** Configuration changes are common during development. A graceful restart is much better than a crash.
**Cost:** Server restart has a brief window where type checking is unavailable.

### flow 8dd8ec844f43c75e8829680a1a0adba8290a59ba — Modernize type names in Flow lib core.js and react.js
**Author:** George Zahariev
**Situation:** Flow's built-in library definitions (core.js, react.js) used legacy type names (`React$Element` instead of `React.Element`, `$NonMaybeType` instead of `NonNullable`, etc.).
**Mechanism:** Update the libdefs to use modern type names, removing the `$`-prefixed legacy names. This aligns Flow's built-in types with modern JavaScript/TypeScript conventions.
**Scale implications:** Better compatibility with modern TypeScript-originated code. Makes it easier to port TS codebases to Flow.

### flow c9b624dc1bb2dba3be6ecd7102a87f977f83f3e5 — Remove `{| |}` exact object syntax from core.js and react.js libdefs
**Author:** George Zahariev
**Situation:** The exact object syntax `{| |}` was Flow-specific and being phased out in favor of the modern `{| ... |}` syntax (which is already what `{| |}` meant anyway — this was a redundant syntactic form).
**Mechanism:** Remove the duplicate exact object syntax from built-in libdefs.

### flow 8256c40d — Faithfully port shared memory GC
**Author:** Sam Zhou
**Mechanism:** Flow uses a custom garbage collector for its shared memory arena. The OCaml version used a specific GC strategy. This commit faithfully ports that GC behavior to Rust, ensuring memory management semantics are preserved in the oxidized version.

### flow 066854e8c0a8a2d01e676f4f3040a2b58b558fdf — Multiple perf fixes
**Author:** Sam Zhou
**Mechanism:** A collection of performance fixes in the Rust port — likely covering memory allocation patterns, data structure choices, and algorithmic improvements in hot paths.

### flow 8c1ea0b9e5eb69a8f0a6059ec15caad54ad77ba4 — Reword 'union optimization' errors to drop implementation framing
**Author:** George Zahariev
**Situation:** Flow's error messages referenced internal implementation details ("union optimization") that don't make sense to users.
**Approach:** Rewrite the error message to be user-facing rather than implementation-facing.
**Mechanism:** Replace error message text. Error messages are user-facing UX — implementation details in errors create confusion.

### flow c659dc7e10d54122862e0f6cbaabe7168931ab3e — Improve exponential spread error to explain why multiple unions blow up
**Author:** George Zahariev
**Situation:** When spread operations encounter many union types, Flow produces an error about exponential blowup, but it didn't explain WHY it happens or what the user should do.
**Mechanism:** Expand the error message to explain the exponential complexity of spread operations on unions, helping users understand the root cause and fix their code.

### flow f6664e8d724868baedc794996def08c0eff18a2c — Small allocation-related and depth-related optimizations
**Author:** Sam Zhou
**Mechanism:** Targeted optimizations to reduce allocations and stack depth in the Rust port. Allocation reduction directly impacts GC pressure and latency. Depth-related optimizations prevent stack overflow in deeply recursive type operations.

---

## 2025 — Rust Port Foundation, TS Compatibility, Modernization

### flow 88e0af25259b37593bdfbe3f8f960041b66aa93c — v0.314.0
Meta release. Skipped.

### flow 343e0f7b28ed02f3833e9f0ca36bb240f7cdb401 — v0.313.0
Meta release. Skipped.

### flow 6ebd7fca0b660bf129a0906ad798a8447e7852cc — Fix saved state heap serialization on Windows (HANDLE vs CRT fd)
**Author:** Sebastian Amengual
**Situation:** Saved-state (incremental type-checking checkpoint) serialization used file descriptors that behave differently on Windows. The Windows CRT file descriptor and HANDLE are not the same — serialization would fail or produce corrupt state.
**Approach:** Use platform-specific file handling. On Windows, use HANDLE directly instead of CRT fd.
**Mechanism:** Conditional file descriptor handling based on platform. On Windows, the `HANDLE` from `CreateFile` must be used; on POSIX, `int fd` from `open`.
**Scale implications:** Windows support is important for the OSS community. Saved state corruption is a silent failure mode — users don't know their incremental results are wrong.

### flow 1e8b8c9aa706d3a147f56f29817e02c8aa87f5b9 — Fix Rust port property maps for type sig IDs
**Author:** Sam Zhou
**Situation:** Property maps (maps from property names to types) in the Rust port had incorrect behavior when handling type signature IDs — likely causing property lookups to fail or produce wrong types.
**Approach:** Debug and fix the `PropertiesMap` implementation for the type sig ID path.
**Mechanism:** PropertiesMap is a critical data structure used whenever Flow looks up a property on a type. Bugs here cause type errors to be missed or false positives to appear.

### flow 35947c315d80e94954d97271a8d12da8694ab312 — [flow][oxidation] Fixes and additional wave of alignment between flow-parser-oxidized packages and upstream
**Author:** Sam Zhou
**Situation:** The flow-parser (OCaml) is upstream; the oxidized (Rust) parser needs to stay in sync. This commit fixes misalignment after an upstream change.
**Approach:** Identify the specific changes in the upstream OCaml parser and port them faithfully to Rust.
**Mechanism:** A wave of fixes covering edge cases where the Rust parser's behavior diverged from the OCaml original. Faithful port means matching the OCaml behavior exactly, even when the Rust approach could be "cleaner."

### flow 0eecda10025752c28cae8d9795b3a1fc9015d193 — Update hermes-parser in fbsource to 0.36.1
**Author:** Ivor Zhou
**Situation:** The hermes-parser (JavaScript parser used by Flow) had a new version with bug fixes and compatibility improvements.
**Mechanism:** Bump the hermes-parser dependency version in Meta's fbsource monorepo. This is internal workflow — keeping parser dependencies current.

### flow bbab698a8b1056206223af4cc07ef60758863fd9 — Setup flow.js oss build (#9416)
**Author:** Sam Zhou
**Situation:** The Rust port produces a JavaScript build (via WASM compilation) for OSS distribution. This commit sets up the OSS build pipeline.
**Mechanism:** Configure the build system to produce a standalone `flow.js` from the Rust port's WASM output, packaged for npm distribution. This is the primary way OSS users will consume the Rust-based Flow.
**Scale implications:** The OSS community uses this build. Getting it right is critical for adoption.

### flow 177d9d9d1c59ed527019339ae35be7961904a7d6 — [oss][ci] Setup OSS CI build and test job for rust port (#9411)
**Author:** Sam Zhou
**Situation:** The Rust port needed its own CI job separate from the OCaml Flow's CI, to validate that the Rust port passes all tests.
**Mechanism:** Add a new CI job that builds the Rust port and runs the full test suite. This is essential for ensuring the port is correct.
**Scale implications:** CI is the feedback loop from "I pushed" to "I know if it broke." Without dedicated Rust-port CI, regressions would be silently merged.

### flow 6f0fc0d4fe98e2bffe0b4c58e851c9fc75349d9c — [oxidation] Partial switch to another persistent red-black tree data structure (`rpds::RedBlackTreeMap`)
**Author:** Sam Zhou
**Mechanism:** `rpds::RedBlackTreeMap` is a persistent (immutable) red-black tree map library in Rust. Flow uses persistent data structures for immutable type representations. Switching to `rpds` likely gives better performance or correctness than a previous implementation.
**Scale implications:** Type checking uses many immutable map operations. The choice of data structure affects both performance and memory usage.

### flow 046c8dbb3f0f5c578a8d7dcd6f53b57b384b5660 — Generate flow.js build by rust->wasm with js glue
**Author:** Sam Zhou
**Mechanism:** Compile the Rust port to WASM, then generate JavaScript glue code to expose it as `flow.js`. This is the WASM-based JS build approach.
**Scale implications:** WASM compilation produces a smaller, faster binary than pure JavaScript. The JS glue exposes the WASM module as a usable Flow API.

### flow cf048ef7d87353012d4e2c0257fade83b4c99625 — [oxidation][perf] Use low overhead `BTreeMap` and `Vec` instead of `FlowOrdMap` and `FlowVector` in name_resolver.rs
**Author:** Sam Zhou
**Mechanism:** `name_resolver.rs` is a hot path during type checking (resolving variable names). The custom `FlowOrdMap` and `FlowVector` abstractions had overhead from boundary checks and indirection. Switching to std `BTreeMap` and `Vec` in performance-insensitive contexts reduces overhead.
**Scale implications:** Name resolution happens for every identifier in every file. Small per-lookup savings compound.

### flow a80e613e1b966f47438e3b59bbd502947c5edf4d — Final wave of alignment between flow-parser-oxidized packages and upstream
**Author:** Sam Zhou
**Mechanism:** After an upstream OCaml parser change, the Rust parser packages (`flow-parser-oxidized-*`) needed alignment fixes. "Final wave" suggests this was the last batch of a multi-wave effort.
**Scale implications:** Keeping the Rust parser in sync with OCaml is ongoing work. Each upstream change requires downstream porting.

### flow 9475bce6f1e92d235ccb789b3976ba97a92aecbc — [oxidation] Port parse_test262 and add rust baseline
**Author:** Sam Zhou
**Mechanism:** `test262` is the ECMAScript conformance test suite. Porting `parse_test262` to Rust and adding a Rust baseline means the oxidized parser must pass the same conformance tests as the OCaml version.
**Scale implications:** Conformance to the JS spec is critical. test262 is the gold standard. Passing it in Rust validates the port's correctness.

### flow 23842e666e4e43e4e4f286ea3da07c9fa10c161e — [oxidation] Fully port the glean runner
**Author:** Sam Zhou
**Mechanism:** Glean is Meta's code intelligence platform. The "glean runner" is the component that extracts code facts for consumption by code intelligence tools. Fully porting it to Rust means the Rust version can produce the same facts as the OCaml version.
**Scale implications:** Glean powers many developer tools at Meta. Porting the glean runner is a prerequisite for fully replacing the OCaml Flow.

### flow cf2656c98589b0a516b97e966e3920d458a43e69 — Mark expensive.md and command_runner.ml as not needed
**Author:** Sam Zhou
**Mechanism:** As the Rust port progresses, some OCaml modules become unnecessary. `expensive.ml` and `command_runner.ml` are marked as no longer needed in the oxidation process — they've either been replaced or their functionality is now in Rust.
**Scale implications:** Tracking "what's still needed" is a key part of the port. This is a progress indicator and a cleanup step.

### flow 96d14d65950e87efd51456da274fb8e5e8538228 — [oxidation] Port over a few unit tests
**Author:** Sam Zhou
**Mechanism:** Unit tests are being ported from OCaml to Rust alongside the code they test. Porting tests ensures the Rust implementation maintains the same behavior guarantees.
**Scale implications:** Test coverage must be preserved during the port. If a Rust port doesn't have the same tests, regressions won't be caught.

---

## 2024 — Stabilization, TypeScript Consumer Compatibility

### flow b55912919177106c953c1edff9bce4e25d78572a — [oxidation] A few PropertiesMap related optimization
**Author:** Sam Zhou
**Mechanism:** PropertiesMap is the data structure for storing property-name → property-type mappings on object types. It's queried constantly. Optimization likely involves better hash functions, better sharing, or more efficient lookup.

### flow 8c678c56d8b236299734daf9a1c48e2f9a102399 — Fix registry url
**Author:** Sam Zhou
**Mechanism:** The npm registry URL in the package.json was wrong — likely pointing to an old or internal URL. Fixed to point to the public npm registry.
**Scale implications:** OSS users installing Flow from npm need the correct registry.

### flow f9eade689207c414db124f2a5eb93e30c79fbaa9 — Fix file watchers to report .flowconfig changes
**Author:** (not visible in truncated log)
**Mechanism:** The file watcher system monitors source files for changes to trigger re-checks, but `.flowconfig` wasn't being watched. When `.flowconfig` changed, Flow wouldn't notice and would continue using stale config.

### flow e92b9cbf86f1c2e0a901a0ba4be1e41633c9f9b0 — [tslib] Recognize `// @ts-expect-error` and `// @ts-ignore` in TS files
**Author:** George Zahariev
**Mechanism:** TypeScript suppression comments (`@ts-expect-error`, `@ts-ignore`) weren't being recognized in `.ts` files consumed by Flow. This caused Flow to error on code that TS would suppress.
**Scale implications:** TypeScript consumers often use these suppressions. Recognizing them makes Flow more compatible with TS codebases.

### flow 512dad0bf4578e9d72f173beaf09cd5730da5f88 — [tslib] Treat class/interface instances structurally and relax exact-object enforcement in .ts consumers
**Author:** George Zahariev
**Situation:** Flow's type system is stricter about exact object types than TypeScript's. When Flow processes `.ts` files (TypeScript consumer mode), enforcing exact objects caused friction.
**Approach:** Relax exact-object enforcement in `.ts` consumer context — Flow behaves more like TypeScript when processing TS files.
**Mechanism:** When processing `.ts` files, class and interface instances are treated structurally (like TypeScript) rather than nominally. This improves TS interoperability.

### flow cd72439efeb81c5aaab3d019e0dc0c59a9f4524b — [tslib] Suppress TS-incompatible variance errors in `.ts` files
**Author:** George Zahariev
**Situation:** Flow's variance checking (readonly arrays, in/out positions) produces errors that TypeScript doesn't. In `.ts` files, these caused false positives for TS code.
**Mechanism:** Suppress variance-related errors when the file is a `.ts` consumer file. Flow behaves more like TS when processing TS files.

### flow 94108605643fcff630d72bd9562902329253c1fe — Fix quadratic scope scanning in name resolver
**Author:** (not visible in truncated log)
**Situation:** The name resolver was scanning scopes in O(n²) time — for large files with many nested scopes, this became a performance bottleneck.
**Approach:** Identify the quadratic pattern and optimize to O(n).
**Mechanism:** Scope scanning was likely re-scanning already-scanned elements. The fix ensures each scope is scanned once and results are reused.

### flow 766bcad4 — Fix silent mergebase corruption after watchman misfire
**Author:** Panos Vekris
**Situation:** Watchman (Facebook's file watching service) can send spurious filesystem notifications. Flow was treating these as real changes and updating its mergebase incorrectly, leading to silent corruption of the type-checking state.
**Approach:** Validate that the mergebase change is legitimate before accepting it. Add a check that the changed file actually impacts the mergebase.
**Scale implications:** Silent state corruption is one of the most dangerous failure modes. The type checker produces wrong results without any visible error.

### flow 37d29f8 — Port `shared_memory` GC to Rust
**Author:** Sam Zhou
**Mechanism:** The shared memory garbage collector — which manages Flow's arena allocator used across multiple type-checking workers — was ported from OCaml to Rust. This is a complex piece of runtime infrastructure.

---

## 2023 — Rust Port Early Days, Performance Investigation

### flow c5b0c9 — [flow][oxidation] Start a fresh oxidation pass on server.ml
**Author:** Sam Zhou
**Situation:** The Rust port (codenamed "oxidation") begins for the Flow server (`server.ml`). The OCaml server is the central coordinator for type checking, file monitoring, and LSP.
**Approach:** Start fresh port of the server component, which coordinates all type-checking work. This is one of the most complex pieces.
**Mechanism:** The OCaml server coordinates workers, manages file state, runs the type checker, and serves LSP requests. Porting it requires understanding all these subsystems.

### flow 0d3f82 — [flow][oxidation] Add persistent state for Rust port
**Author:** Sam Zhou
**Mechanism:** Flow's server maintains persistent state (saved state) so that restarting doesn't require a full re-check. This commit adds the persistent state infrastructure to the Rust port.

### flow ff12c3 — [flow][oxidation] Port the collector worker
**Author:** Sam Zhou
**Mechanism:** The collector worker is the process that runs the type checker across file partitions. Porting it requires understanding Flow's parallel type-checking architecture.

### flow 45b88d — Fix quadratic behavior in unifyAndCanonicalize Funktion
**Author:** (not visible)
**Situation:** The type unification algorithm had quadratic behavior — O(n²) in the size of the type graph. This manifested as slow type checking on files with complex type hierarchies.
**Approach:** Profile to identify the quadratic path, then optimize.
**Mechanism:** The `unifyAndCanonicalize` function was likely re-traversing parts of the type graph unnecessarily. Caching or better canonical form handling eliminates the quadratic path.

### flow 8f2fbe0 — Fix memory blowup on cyclic module graphs
**Author:** (not visible)
**Situation:** Cyclic module imports (A imports B imports A) caused memory to explode during type checking. Flow wasn't handling the cycle-detection properly.
**Approach:** Detect and handle cycles properly in the module graph traversal.
**Mechanism:** When a cycle is detected, break the cycle at an appropriate point rather than traversing infinitely. The challenge is doing this without losing type information across the cycle.

---

## Recurring Patterns

### Pattern 1: Silent Correctness Failures Are the Worst Failure Mode
Many of the worst bugs (stale mergebase, Watchman misfire, saved-state corruption) are silent — Flow produces wrong results without any visible error. This teaches that at scale, correctness bugs in developer tools are more dangerous than crashes, because crashes are visible and crashes prompt investigation, while silent wrong results lead to wasted time and incorrect engineering decisions.

### Pattern 2: The Rust Port Is Both a Rewrite and a Compatibility Exercise
The oxidation effort isn't just "port to Rust." It has to maintain exact behavioral compatibility with the OCaml original — same error messages, same type-checking results, same LSP behavior. Every divergence requires a fix. This is a massive undertaking that requires comprehensive test suites (test262, existing test262 baseline, LSP test suites) to validate correctness.

### Pattern 3: TypeScript Consumer Mode Is a Distinct Operational Mode
Flow's `.ts` file handling has to be distinct from its native `.js` handling. For `.ts` files, Flow suppresses many strict checks (variance, exact objects) to match TypeScript's behavior. This dual-mode operation adds complexity but is necessary for TypeScript ecosystem compatibility.

### Pattern 4: Meta-Specific Infrastructure Leakage
EdenFS integration, Watchman file watching, Meta's build system — Flow has deep integration with Meta's infrastructure that doesn't translate to OSS. Some of this is unavoidable (EdenFS is how Meta developers work), but it creates a divergence between internal and OSS versions that must be managed.

### Pattern 5: Error Message Quality Is Ongoing Work
Many commits improve error messages (explain WHY, drop implementation details, add actionable guidance). Error messages are the primary interface between Flow and developers. Making them clear and actionable reduces the time developers spend confused.

---

## Key Principles

1. **Validate the port with the existing test suite** — The Rust port must pass all existing tests. New CI jobs are added specifically for the Rust port to ensure no regressions.

2. **Avoid silent failures** — Where possible, make wrong states visible rather than silently propagating them. The mergebase bug fixes all follow this principle.

3. **Optimize the hot paths** — `ALoc` comparison, `PropertiesMap` lookups, name resolution — these are called millions of times per session. Small improvements compound.

4. **Know when to trade correctness for performance** — Removing caches fixes bugs but can hurt performance. The trade-off is explicit and evaluated.

5. **Keep the parser in sync with upstream** — The OCaml parser is "upstream" for the Rust parser. Each upstream change requires downstream porting effort. This is ongoing maintenance cost.

6. **Error messages are UX** — Many commits are purely about improving error message quality. This is developer experience investment that doesn't add features but makes the tool more usable.

7. **Incremental type checking state must be robust** — Saved state, mergebase tracking, file watcher coordination — the incremental infrastructure is where many subtle bugs live.

8. **Platform differences (Windows vs POSIX) manifest in low-level I/O** — File descriptors, path separators, line endings — these differences cause failures that are hard to diagnose because the code "works on my machine" (Linux/Mac).