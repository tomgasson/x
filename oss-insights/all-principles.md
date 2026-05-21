# What High-Scale, Long-Running Projects Actually Teach

> Synthesized from: facebook/hhvm, facebook/hermes, facebook/flow, facebook/relay, facebook/buck2, facebook/sapling, oxc-project/oxc, rolldown/rolldown, wild-linker/wild
> Analysis period: 2023–2026

---

## The Situations That Actually Arise

### 1. Codegen is the highest-leverage investment you can make
HHVM's Whisker migration (replacing mustache templates with structured-handle codegen) took years and dozens of commits. Buck2 rewrote `RelativePath` in-house (948-line file, 28 files changed). Flow's Rust port of the OCaml typechecker is multi-year. Sapling's C++20 coroutine migration follows the same pattern.

The pattern: these are infrastructure bets that pay off on *every future change*, not just the immediate task. When codegen is easier to reason about, every new feature and refactoring is cheaper. Teams invest in them because they understand codegen as leverage, not cost.

**Do this:** When you find yourself patching around a codegen limitation for the third time, propose a structured rewrite. One large correct system beats dozens of small workarounds.

---

### 2. Silent correctness failures are the worst failure mode
Flow's stale mergebase bug (EdenFS `LostChanges` events causing incorrect type results with no error signal), Hermes's handle leaks at specific thresholds (48 slots, not a memory growth curve), Sapling's symlink safety issues — these are failures that produce wrong results without visible errors.

The worst class: type checkers that return incorrect types, compilers that silently drop data, linkers that produce binaries that sort of work. Developers trust the tool and make incorrect decisions.

**Do this:** Build detection before building the fix. A silent failure that becomes loud is far cheaper than a silent failure that isn't caught.

---

### 3. Memory safety is ongoing engineering discipline, not a one-time fix
Hermes ran a multi-year phase program (P1-S1 through P1-S8+) to systematically retrofit RAII guards onto every compilation entry point. HHVM discovered that `longjmp` + RAII = handle leaks (the `_sh_throw_current` in `putByIndex_RJS` bypasses destructors, accumulating unflushed handle slots until a debug threshold is crossed). OXC's allocator work revealed that platform differences (overcommit on Linux vs. immediate commit on Windows) fundamentally change what "safe" means.

The discipline: debug allocator checks with specific thresholds (`HERMESVM_DEBUG_MAX_GCSCOPE_HANDLES = 48`) transform silent leaks into loud failures at a known point. Without them, leaks compound until they cause OOM or undefined behavior.

**Do this:** ASan by default. Debug allocator checks in test modes. RAII at every API boundary — not as an afterthought, as a systematic program.

---

### 4. Async I/O requires the entire call chain to be sync-safe
Rolldown's `emitFile` deadlock (triggered at just 400 emits under parallelism in a normal RSC app) happened because a sync NAPI binding called into async Rust, which waited on a bounded channel that filled up while the JS thread was parked waiting for a TSFN callback — classic producer/consumer deadlock through the ThreadsafeFN boundary. The fix: the entire Rust call chain from the binding to the result must be sync. No `.await` in any layer.

HHVM's io_uring rollout touched the deepest substrate (AsyncSocket, ClientFactory, zero-copy buffers, StopTLS) and took multiple years. Sapling's coroutine migration followed a careful dependency order (leaf → TreeInode → VirtualInode → EdenServiceHandler), with each layer exposing `co_get<Operation>` that chains via `.semi()` bridges during transition.

**Do this:** If you expose Rust to JS via NAPI, audit every `.await` in the call chain. If you're doing async I/O, design the migration path before you start. io_uring support in HHVM took years because it touched everything.

---

### 5. Spec compliance requires rewriting, not patching
Hermes rewrote Promise combinators (`all`, `allSettled`, `race`, `any`, `finally`) multiple times to match the spec exactly — dropping fast paths that violated spec behavior. The pattern: patches that preserve incorrect behavior build technical debt that eventually requires a full rewrite anyway. Spec-first, then optimize within constraints.

**Do this:** If you're patching spec-violating behavior for performance, you're deferring a rewrite. Better to spec-first from the start.

---

## The Mechanisms That Actually Work at Scale

### 6. Feature flags with production-safe defaults
Hermes disables experimental features in "supported roots" (production configurations). Relay's `@live_query` removal, `@fb_actor_change` removal — both show what happens when you don't have this discipline: you end up paying to remove something that many users depend on.

The discipline: a flag is only useful if the default is safe. Flags enabled by default in production are not flags — they're commitments.

**Do this:** Every experimental feature has a flag. The flag is `false` in production roots, `true` for internal experimentation. Flip the default deliberately, not accidentally.

---

### 7. Reverts are a normal tool, not a failure
HHVM's version control history shows multiple reverts ("Back out D84278317", "Back out D67605161"). The culture treats a revert as a standard safety tool, not an admission of failure. This enables more aggressive rollout of risky changes with the safety net of easy reversion.

**Do this:** Land the revert promptly after discovering a problem. The longer a broken change sits in main, the more integration work accumulates on top of it.

---

### 8. Disassembly and benchmarks are both necessary
HHVM's OpEncode serialization fix started from disassembly analysis (i64 encode ~80 bytes was being inlined at every call site, causing icache bloat). OXC's `NOINLINE` on i64 encode path required benchmarking all four integer sizes separately — i16/i32 benefited from full inlining, i64 regressed. The `FOLLY_ALWAYS_INLINE` functors fix came from profiling the lambda chain overhead separately.

Benchmarks without disassembly miss micro-level patterns. Disassembly without benchmarks misses workload-level impact. Use both.

**Do this:** Profile with real workloads. Use disassembly to understand why a benchmark changed, not just that it changed. Measure before and after every significant change.

---

### 9. Crash isolation via subprocess + scoped recovery
Hermes runs bytecode in a subprocess with `sigsetjmp`/`siglongjmp` within it for recovery. The key: `longjmp` bypasses C++ destructors, so RAII guards won't flush before the jump. The crash guard must explicitly flush before jumping. This is the pattern for running untrusted or potentially-miscompiled code safely.

Sapling surfaces compiler crashes to meerkat clients — the daemon doesn't silently die, it reports the crash to the client so the client can retry or surface the error.

**Do this:** For any execution environment that receives untrusted input, process-level isolation + scoped recovery is more reliable than in-process guards. Explicit flush before non-local exits.

---

### 10. Incremental infrastructure has failure modes that are hard to catch
Sapling's watch loop state management, Flow's mergebase tracking, Relay's incremental build loop (`Clear is_building when no pending changes`), Buck2's saved-state corruption fixes — these all share a pattern: the incremental path silently produces wrong results when its assumptions are violated (file changed during watch, mergebase shifted, external modification detected).

The detection patterns: explicit invalidation signals (EdenFS `LostChanges`), external modification detection, and regression tests specifically for incremental behavior.

**Do this:** Build regression tests for incremental behavior specifically. Watch loops that restart after rebase, cache invalidation on config change, mergebase tracking across filesystem events.

---

## What DevX Eventually Costs

### 11. Documentation is bus factor insurance
Hermes has `DESIGN.md`, IR type system design docs, `Features.md` for Promise deviations. Sapling has multi-commit architecture descriptions across Python/Rust/C++ layers. These capture the *why* of decisions, not just the *what*.

The alternative: knowledge lives in individual engineers' heads and walks with them.

**Do this:** Design docs are not bureaucracy. If it matters, write it down. Include the constraint you were working under and why you chose this approach over alternatives.

---

### 12. Test infrastructure is a first-class concern
Hermes's LIT integration for regression tests under sanitizer modes, Sapling's EdenFS test runner port from Python to Rust, OXC's CodSpeed for continuous instruction-count benchmarks, Rolldown's 2,200-entry app regression tests — all received the same systematic treatment as production code.

**Do this:** Treat test infrastructure investment as production investment. A test suite that can't catch regressions is not a test suite.

---

### 13. TypeScript/Flow type declarations are a maintenance burden you inherit
Relay added missing `.d.ts` entries for `RelayFeatureFlags` and `Store` constructor options. These were present in Flow source but missing from TypeScript declarations — a gap that breaks TypeScript users without warning.

The pattern: when a codebase has both Flow and TypeScript types, the two can drift. This creates silent breakage for TS users.

**Do this:** Sync type declarations in CI. Missing types should fail CI, not silently accumulate.

---

### 14. Compiler output check CI is a recurring tax on feature flag changes
Relay had three commits in quick succession ("Compiler output check" CI job failing) because feature flag changes altered compiler output but generated test-project files weren't regenerated. This happens every time a flag changes the generated artifact format.

**Do this:** Document that every feature flag that changes compiler output requires regenerating test fixtures. Automate the regeneration step as part of the flag-change process.

---

## What Performance Work Looks Like in Practice

### 15. The highest-value optimizations are for the most common operations
Hermes's `===` fast path, numeric key array write optimization, OXC's arena allocation strategy — these are operations that run millions of times per second in every program. Even small per-operation savings compound.

**Do this:** Profile before optimizing. The most impactful optimizations target hot paths with predictable data layouts. Everything else is guesswork.

---

### 16. TDigest micro-optimization shows what sustained engineering looks like
HHVM's TDigest work had five commits over a sustained period: insertion sort as-you-go replacing `std::sort`, eliminating 18kB heap allocations, streaming heap merge replacing intermediate buffer + recursive sort, deduplication of merge paths, adding `folly::down_heap()` as a new general-purpose primitive.

Each commit made the previous one look obviously suboptimal in hindsight. The team measured carefully (named benchmarks, before/after nanoseconds) and documented the specific benchmark that motivated each change.

**Do this:** Performance work is iterative. Each small improvement makes the next one visible. Don't aim for a single "big win" — aim for a continuous stream of small improvements with measurement.

---

### 17. SIMD only matters for genuinely hot paths with predictable data layout
Hermes's SIMD-accelerated JSON string scanning, OXC's vectorized operations — the complexity cost of SIMD intrinsics is only worth it for operations running millions of times per second with predictable byte patterns.

**Do this:** SIMD for JSON parsing, string scanning, numeric array operations. Not for once-per-request initialization.

---

### 18. Platform differences are fundamental, not edge cases
OXC's Windows `VirtualAlloc` work (three PRs specifically for platform-specific allocator behavior), Sapling's multi-target ACL enforcement across Python/Rust/C++, Flow's path separator normalization on Windows — these show that abstraction layers that work on one OS can fundamentally fail on another not because of bugs but because of different operating system semantics.

**Do this:** Test on all target platforms, not just your development machine. Platform-specific code paths are not edge cases — they're load-bearing.

---

## What Failure Looks Like and How It's Contained

### 19. Binary metadata is part of the contract, not optional
Wild linker's code signature fixes (using `args.output` for CS identifier, not internal binary name), SFrame conditional output, debug section compression — these aren't optional metadata. They are what makes a binary usable in production: code signatures for Gatekeeper, debug info for crash reports, symbol tables for profilers.

A linker that produces correct machine code but missing metadata fails the users who depend on that metadata.

**Do this:** Treat binary metadata (code signatures, debug info, symbol tables, eh_frame) as load-bearing, not optional. Test with the tools that consume the metadata.

---

### 20. Chunk optimization is a constraint satisfaction problem, not a heuristic
Rolldown's `would_create_circular_dependency` algorithm went through multiple iterations: a fix for real crashes (#8371) introduced false positives that blocked 1,046 legitimate merges (+12.5% bundle size). The revert found that a different PR (#9085) had already fixed the underlying issue through a different mechanism.

The lesson: systems with multiple merging strategies (circular dependency, runtime placement, facade elimination, tree-shaking) have interaction points where a change to one constraint breaks another. Broad integration tests for this layer matter more than isolated unit tests.

**Do this:** When fixing a chunking bug, map the full constraint system before landing. Changing one heuristic can silently break another.

---

### 21. File descriptor limits are a production concern at scale
Wild linker had multiple commits specifically addressing file descriptor limits ("Set file limit before we open input files", "Increase file limit when linker plugin is active"). The cost of not raising limits: a link that fails 45 minutes into a 50-minute build.

**Do this:** Monitor file descriptor usage in CI at scale. Raise limits before you hit them.

---

## What Architecture Under Pressure Looks Like

### 22. Multi-phase projects require tracked phase decomposition
Hermes's IR type system (P1-S1 through P1-S8+), Sapling's coroutine migration (Phase 11), Rolldown's defer/stream (Phase 1+2, Phase 3), HHVM's io_uring rollout (multiple years) — all follow the same pattern: a large effort is decomposed into numbered phases, each independently testable and reversible. The phase number in the feature flag name is not decoration — it's the project management structure.

**Do this:** Decompose large efforts into numbered phases. Each phase is independently deployable and reversible. Track the phase number in the feature flag.

---

### 23. Schema/state migration complexity compounds
Relay's SchemaSet work (coordinate merging, partitioning, exclusion operations), Sapling's NanoDag for non-linear history, Buck2's panic→Result migration (668/week panics in DetailedAggregatedMetrics converted to proper error propagation) — all show that state migration is where subtle bugs live. The migration from `Option<bool>` to `MergeResolutionOverride { UseJk, ForceOn, ForceOff }` is a case study: the old type's implicit meaning of `None` was ambiguous at every callsite.

**Do this:** State migration types should be explicit enums with named variants, not `Option<T>` or `bool`. Exhaustiveness checking at compile time is cheaper than runtime bugs from implicit defaults.

---

### 24. Cross-tool behavior alignment validates correctness
OXC's DCE-only IIFE preservation and optional chain folding both cite empirical comparisons against Rollup, esbuild, SWC, and Terser as correctness validation. The team doesn't just implement features — they validate against established tools to catch behavioral divergences.

**Do this:** For parsers, formatters, and optimizers, maintain a cross-tool comparison test suite. divergences from established tools should be intentional and documented.

---

### 25. Default changes are high-stakes
Wild linker's "enable plugin support by default" (affects every user who upgrades without explicit flag changes), Buck2's RelativePath rewrite, Relay's `@live_query` removal — these show that changing a default is a high-visibility event. It requires careful backward-compat thinking, migration paths, and documentation.

**Do this:** Changing a default is a breaking change regardless of API compatibility. Treat it as a significant event: migration guide, deprecation timeline, escape hatch for users who need the old behavior.

---

## Cross-Cutting Concerns

### 26. Zero-overhead abstractions make profiling infrastructure viable
OXC's `TIMINGS` const generic (`TIMINGS in #22282`) adds profiling that costs nothing when disabled. Buck2's panic elimination removes `.expect()`/`.unwrap()` from hot paths, converting crashes to errors. Both patterns show: you can add observability without adding overhead if the design is sound.

### 27. Rust's type system as a firewall against unexpected behavior
Buck2 removed cross-type string comparisons from path types (`PartialEq<str>` implementations), citing that in the age of AI-generated code, the risk/reward calculus shifts — agents are more likely to trigger unexpected comparison behavior. The narrowing of allowed operations makes the codebase more predictable.

### 28. Dependency management has real costs at scale
Buck2's Kotlin 2.2 revert, Relay's `Update Cargo.lock` commits, OXC's hashbrown updates — these show that staying current has a cost (validation, potential revert), and falling behind has a cost (security, compatibility). The right answer is not "always update" or "never update" — it's a systematic approach with automated checks.

### 29. Jobserver coordination is load-bearing infrastructure
Wild linker's `-Wl,--jobserver` parsing, Sapling's thread pool management, Buck2's deferred instantiation — these show that build system integration is not peripheral. Every path that touches the build system (plugin invocation, response files, jobserver coordination) is a potential failure point that must be handled robustly.

### 30. Error message quality is ongoing investment, not polish
Flow's commits repeatedly improve error messaging (drop implementation details, explain root causes, add actionable guidance). Relay's `include full directive definition in subset violation error message`. These aren't cosmetic — they determine whether developers can solve problems without filing issues.

---

## Engineering Principles to Live By

1. **Make it work, make it safe, make it fast — in that order.** Correctness is never a trade-off against performance.

2. **Push work to compile time.** Types, const generics, exhaustiveness checking — catch errors before you run.

3. **Design for failure explicitly.** Every external resource (DB, HTTP, disk) can fail. Handle it. Every silent failure is a bug waiting to be discovered.

4. **Measure before you optimize.** Profile first. The most impactful optimizations are at the hottest call sites.

5. **Reverts are a safety net, not a failure.** Land them promptly.

6. **Feature flags without production-safe defaults are not flags.** They're commitments.

7. **When you do large migrations, do them commit-by-commit.** Each step independently testable and reversible.

8. **Documentation is bus factor insurance.** Write it down.

9. **Default changes are high-stakes events.** Treat them that way.

10. **Cross-platform is not an edge case.** Test on all target platforms. Platform-specific code is load-bearing.

11. **Incremental infrastructure has silent failure modes.** Build regression tests for them specifically.

12. **Chunk optimization / constraint satisfaction problems require full system modeling.** Changing one heuristic can silently break another.

13. **Binary metadata (signatures, debug info, symbols) is part of the contract.** It's not optional.

14. **Sustained micro-optimization compounds.** TDigest's five-commit campaign is the model — each small win makes the next visible.

15. **RAII at every API boundary.** Not as an afterthought — as a systematic program.

---

*This document is a synthesis. For the full commit-by-commit substance, see the per-repo insight files in `oss-insights/`. Every principle here is grounded in a specific commit, situation, and decision.*