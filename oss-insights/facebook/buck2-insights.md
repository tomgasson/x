# facebook/buck2 — OSS Insights

_Analysis of 2023–2026 commits, focusing on architectural changes, critical bugfixes, performance work, and breaking changes._

---

## fs: Implement `RelativePath` in buck2 (May 2026)
**Author:** Jakob Degen  
**Situation:** Buck2 depended on the external `relative_path` crate, which had historically caused "all sorts of surprise behavior" — inconsistent path separator normalization on Windows, silently rewriting inputs, and cross-type comparisons that were bug-prone. The `ForwardRelativePath` type had already been re-implemented internally as a model for how to do this properly.
**Approach:** Nearly direct re-implementation of `RelativePath` in `buck2_fs`, rolling in the Windows workarounds already accumulated in the codebase. A large FIXME at the top of the new implementation signals there is more cleanup to come — this is a deliberate staged rewrite.
**Mechanism:** 948-line `relative_path.rs` replaces the external crate. 28 files changed across the entire codebase as every callsite is updated. The new type preserves the same public API but with corrected semantics for path component handling and separator normalization.
**Scale implications:** Path handling is a foundational concern that touches almost every file in the codebase. Rewriting it in-house means the team can control semantics directly rather than patching around upstream behavior. The staged approach (one giant FIXME rather than trying to do everything at once) reflects how risky large底层 refactors are — better to ship a correct baseline and iterate than to try to fix everything before landing.
**Cost:** 28 files changed, 979 insertions, 174 deletions. The volume reflects the depth of coupling to this type. The staged FIXME approach means this is not yet "done."

---

## fs: Make directory renaming component based (May 2026)
**Author:** Jakob Degen  
**Situation:** Directory renaming in `RelativePath` was doing string operations, which produced "all sorts of surprise behavior." String-based path manipulation doesn't handle separators, normalization, or component boundaries correctly.
**Approach:** Switch to component-based operations — parse into components first, then manipulate, then reconstruct.
**Mechanism:** 3 files changed across `actions/impls/run/dep_files.rs`, `directory.rs`, and `relative_path.rs`.
**Scale implications:** This is a pattern that recurs throughout the fs rewrite — the team is systematically replacing string-based path hackery with component-based semantics. Each fix is small but the cumulative effect is a more robust path handling layer.
**Cost:** 33 insertions, 29 deletions.

---

## fs: Stop silently rewriting input in `RelativePathBuf::push` (May 2026)
**Author:** Jakob Degen  
**Situation:** The `push` method was stripping a leading `/` off the pushed path and then concatenating verbatim — papering over a clearly-broken caller rather than surfacing the error.
**Approach:** Stop doing the silent rewrite. Let the input speak for itself and fail loudly if it's wrong.
**Mechanism:** Removed the ad-hoc stripping logic from `RelativePathBuf::push`. The method now just concatenates.
**Scale implications:** Silent rewrites in path handling are a common source of cross-platform bugs. By removing the magic, callers must be explicit about what they want, which surfaces bugs earlier rather than letting them propagate into the wider system.
**Cost:** 3 insertions, 17 deletions — net reduction in code.

---

## fs: Delete nonsense code (May 2026)
**Author:** Jakob Degen  
**Situation:** Code existed that "very transparently attempts to turn an absolute path into a relative path" — something that cannot be done correctly without additional context (the base directory).
**Approach:** Delete it rather than try to fix it. Return an error immediately if this operation is attempted.
**Mechanism:** Removed code in `cmd_args/builder.rs` that attempted the impossible transformation.
**Scale implications:** Build systems accumulate hacky path manipulation code over time as developers work around bugs without fully understanding them. Deleting nonsense is often safer than refactoring it.
**Cost:** 4 insertions, 6 deletions.

---

## fs: Remove comparisons to strings (May 2026)
**Author:** Jakob Degen  
**Situation:** Cross-type comparisons like `RelativePath == str` were allowed, which is "a bit bugprone." In the age of AI agents that generate code, the risk/reward calculus shifts — agents are more likely to trigger unexpected comparison behavior.
**Approach:** Remove all cross-type string comparisons from path types.
**Mechanism:** Removed `PartialEq<str>` and similar implementations from `cmp_impls.rs`, `forward_rel_path.rs`, and other path-related files. 8 files changed, 13 insertions, 156 deletions — mostly deletions.
**Scale implications:** Type systems are a firewall against unexpected behavior. By narrowing the surface of allowed operations, the codebase becomes more predictable and agents that work with it have fewer surprising failure modes.
**Cost:** 156 deletions.

---

## writes: Rewrite command line formatting (May 2026)
**Author:** Jakob Degen  
**Situation:** The old `CommandLineFormatter` used dynamic dispatch through `CommandLineContext` and `CommandLineBuilder` traits. "This was extremely hard to reason about as things were kind of non-local across these impls, as proven by the bugs." The ad-hoc wrapper pattern made it unclear which option stack was active at any point.
**Approach:** Rewrite entirely with static dispatch. `CommandLineFormatter` now maintains stacks of in-scope options and applies them in the correct order when an arg is pushed. Simpler sink at the bottom (`CommandLineBuilder`), static option application throughout.
**Mechanism:** 21 files changed, 762 insertions, 1042 deletions — the largest rewrite in this set. Intentional behavior changes include bug fixes and a change in how `absolute_prefix`/`absolute_suffix` nest: inner now overrides outer rather than both applying in a counterintuitive order.
**Scale implications:** Dynamic dispatch in hot paths (command line formatting happens for every action) hides costs. Static dispatch eliminates virtual call overhead and makes the option application order explicit and auditable. The bugs fixed were "clearly not by-design" — they were artifacts of the dynamic dispatch layer being too complex to reason about.
**Cost:** Very large rewrite, though Jakob notes benchmarking showed zero regression in performance.

---

## writes: `Cow` in `CommandLineBuilder` (May 2026)
**Author:** Jakob Degen  
**Situation:** Command line building was allocating unnecessarily, particularly visible when combined with subsequent diffs in the stack.
**Approach:** Use `Cow<str>` to avoid allocations when the value is already owned.
**Mechanism:** Changes across 7 files, 39 insertions, 32 deletions.
**Scale implications:** This is a micro-optimization that becomes visible at scale — command lines are built for every action in a build, and builds at Meta involve millions of actions. Avoiding a few allocations per action compounds.
**Cost:** Modest but targeted.

---

## writes: Remove `ArgBuilder` (May 2026)
**Author:** Jakob Degen  
**Situation:** `ArgBuilder` was a parallel abstraction to `CommandLineBuilder` that did "basically the same thing." Having two parallel types for the same job is confusing and maintenance burden.
**Approach:** Remove `ArgBuilder` entirely, consolidate on `CommandLineBuilder`.
**Mechanism:** 6 files changed, 34 insertions, 54 deletions.
**Scale implications:** Consolidation reduces the API surface and the number of concepts developers need to hold in their heads. Less abstraction to maintain means fewer bugs in the abstraction layer itself.
**Cost:** Net deletion, 20 fewer lines of code.

---

## writes: Refactor cmd_args tests (May 2026)
**Author:** Jakob Degen  
**Situation:** Tests for command line argument handling were in a single monolithic file with poor organization. "Repros a whole bunch of bugs" that existed in the system but weren't being caught.
**Approach:** Split into multiple files (`old.rs`, `options.rs`, `testing.rs`, `inputs_outputs.rs`) and add many more tests, particularly for the behavior when multiple options are applied in sequence.
**Mechanism:** 6 files changed, 938 insertions, 530 deletions.
**Scale implications:** Comprehensive test coverage of option sequencing exposes bugs that only appear in combination. The previous monolithic test structure likely missed interaction bugs that this refactor surfaces. Tests are documentation of expected behavior — better-organized tests make the system's behavior more legible.
**Cost:** Test code grew significantly, but this is a feature not a cost — the bugs caught will save debugging time later.

---

## feat: implement bytes type (May 2026)
**Author:** Neil Mitchell  
**Situation:** Starlark spec includes a `bytes` type but buck2's Starlark implementation lacked it. The type is an immutable sequence of bytes (integers 0-255), distinct from strings.
**Approach:** Full implementation per the Starlark spec: literal syntax (`b"hello"`, `b'hello'`, `b"""hello"""`, `rb"\n"` raw), constructor from various sources, indexing returning integers (not single-byte bytes), slicing, containment checks, concatenation, repetition, comparison, hashing, `str(b"...")` as UTF-8 decode, `ord()`, `.elems()` method.
**Mechanism:** 16 files changed, 1337 insertions, 17 deletions. Changes span lexer (247 lines for literal syntax), AST, grammar, type system.
**Scale implications:** Implementing a full type from spec means ensuring every operation behaves correctly in every context. The spec-driven approach means compliance is verifiable but the implementation must handle every edge case.
**Cost:** Large feature addition, 1337 lines.

---

## Add `collect_str` vtable method to `StarlarkValue` (May 2026)
**Author:** Neil Mitchell  
**Situation:** `bytes` type needs `str(b"hello")` to return `"hello"` (UTF-8 decode) rather than `b"hello"` (repr format). The existing `str()` behavior was hardcoded to use repr for most types.
**Approach:** Add `collect_str` method to the `StarlarkValue` trait with a default that delegates to `collect_repr`. Types that need different `str()` vs `repr()` behavior override it. The vtable gets a new entry.
**Mechanism:** 4 files changed, 42 insertions, 10 deletions. `StarlarkStr` overrides `collect_str` to be identity.
**Scale implications:** Extensibility of the trait system — adding a new vtable method without breaking existing implementers (default implementation). This pattern allows the type system to evolve without forcing all existing types to be updated.
**Cost:** Small, targeted, enables the bytes type's `str()` semantics.

---

## f-string expressions (May 2026)
**Author:** Troy Benson  
**Situation:** F-strings in Starlark could only contain simple identifiers, not arbitrary expressions. Developers used to Python's f-strings wanted `f"some text: {abc.xyz}"` or function calls inside expressions.
**Approach:** Move f-strings to be part of the grammar with arbitrary expression support, including quotations and sub-braces.
**Mechanism:** 21 files changed, 1877 insertions, 202 deletions. Grammar changes in lalrpop, parser changes, 857-line golden test file for lexer tests.
**Scale implications:** Grammar changes in a language runtime are high-stakes — incorrect parsing can produce subtle semantic bugs. The large golden test file suggests extensive test coverage for the new parsing behavior.
**Cost:** Very large change (1877 lines added).

---

## bugfix: external cells race condition/inconsistent state (#1259) (May 2026)
**Author:** Jade Lovelace  
**Situation:** `git remote add origin` is not idempotent — if `.git/config` already has an "origin" remote, it fails with "remote origin already exists." This happens during cross-process races: daemon restart mid-fetch, `--no-buckd`, or two daemons briefly overlapping. The in-process semaphore only coordinates within a single daemon process.
**Approach:** Remove the named remote entirely. `git fetch` accepts a URL directly as its first argument, so `git remote add origin` is unnecessary.
**Mechanism:** 1 file changed, 3 insertions, 8 deletions. The race condition was fixed by removing the operation that could fail non-idempotently, rather than making the operation idempotent.
**Scale implications:** Distributed system edge case: external cells (similar to Bazel's workspace status) can be initialized concurrently by multiple processes. The fix acknowledges that coordination within a single process is insufficient — operations must be idempotent or eliminated.
**Cost:** Minimal code change, but fixes a real-world race that manifested as "Error fetching external cell with git, exit code: ExitStatus(unix_wait_status(768))".

---

## Fix SERVER_PANICKED crash(b48474) in LSP: remove `Url::from_file_path().unwrap()` (May 2026)
**Author:** Scott Cao  
**Situation:** Three `Url::from_file_path().unwrap()` calls in LSP code caused ~1/week daemon panic. `Url::from_file_path()` returns `Err(())` if the path is not absolute, and in rare edge cases `ProjectRoot::resolve()` could produce a path that fails this check.
**Approach:** Convert panics to proper error propagation using `internal_error!()`, so the LSP operation fails gracefully instead of crashing the daemon.
**Mechanism:** 1 file changed, 13 insertions, 5 deletions.
**Scale implications:** LSP servers run long-lived and process many files. A panic that crashes the daemon disrupts the developer's workflow significantly. Converting to error handling means the daemon stays up and the operation fails cleanly with an error message.
**Cost:** Small, surgical.

---

## Replace panic with process::exit in schedule_termination (May 2026)
**Author:** Scott Cao  
**Situation:** `maybe_schedule_termination` used `panic!()` to force-terminate the daemon after a timeout. This triggered crash reporting in logview, creating the most noisy panic key in the system.
**Approach:** Replace `panic!()` with `soft_error!(task: false)` + `std::process::exit(1)`. The forced termination behavior is preserved, the diagnostic message goes to stderr for debugging, but the panic hook is not triggered.
**Mechanism:** 1 file changed, 22 insertions, 6 deletions.
**Scale implications:** Panic vs exit: panics trigger the Rust panic hook (logging, crash reporting), exits do not. For intentional forced termination, exit is the right tool. This eliminates logview noise and avoids wasted cycles in crash processing for an expected termination.
**Cost:** Modest but significant noise reduction.

---

## Convert DetailedAggregatedMetrics event handler panics to Result (May 2026)
**Author:** Scott Cao  
**Situation:** `action_executed`, `analysis_started`, and `analysis_complete` methods on `DetailedAggregatedMetricsEventHandler` used `.expect()` which panics when the receiver is dropped (state tracker task exits before senders are done). ~668 panics/week in production.
**Approach:** Convert the three methods to return `buck2_error::Result<()>`. Replace `.expect()` with `.map_err(|_| internal_error!(...))?`.
**Mechanism:** 2 files changed, 22 insertions, 15 deletions.
**Scale implications:** Async race condition: when a receiver (task) is dropped before senders finish, `.expect()` on the send fails. By making the methods return `Result`, callers propagate the error gracefully instead of crashing. The pattern of converting panic to error is consistent with other fixes in this codebase.
**Cost:** Moderate.

---

## Convert panic to error in starlark unpack instruction (May 2026)
**Author:** Scott Cao  
**Situation:** `InstrUnpackImpl` had `assert!` calls that fired when a user-defined type had inconsistent `length()` and `iterate()` implementations. This panicked 3 times in 7 days in production.
**Approach:** Convert both `assert!` calls to proper error returns using `AssignError::IncorrectNumberOfValueToUnpack`.
**Mechanism:** 1 file changed, 15 insertions, 4 deletions.
**Scale implications:** User-facing errors that can be triggered by configuration (inconsistent type implementation) should not be panics. The build system should report the error and continue, not crash the daemon.
**Cost:** Small.

---

## pin multi-byte UTF-8 panic in create_action_key_suffix (May 2026)
**Author:** Scott Cao  
**Situation:** `String::truncate` requires the byte index to fall on a UTF-8 char boundary. `create_action_key_suffix` truncated at `MAX_SUFFIX_LEN - "(truncated)".len()` (= 1013) regardless of where that index lands. Test names made of 3-byte codepoints reliably triggered this ~60 times/week.
**Approach:** First commit lands a `#[should_panic]` regression test pinning the current (buggy) behavior. The follow-up in the stack fixes it and converts to a positive assertion.
**Mechanism:** 1 file changed, 14 insertions.
**Scale implications:** This is the disciplined approach to fixing a bug with a regression test: first pin the current behavior, then fix. The bug is about encoding assumptions — truncating a byte index without checking if it's a valid UTF-8 boundary is a classic off-by-one/multibyte character issue.
**Cost:** Small.

---

## buck2: normalize `\` to `/` in `ExternalSymlink::new` on Windows (May 2026)
**Author:** Mark Shamis  
**Situation:** `ExternalSymlink` targets serialized verbatim into `RE::SymlinkNode.target` and materialized on Linux RE workers. On Windows hosts with EdenFS, `read_link` returns backslash separators, which flowed through to Linux workers that couldn't resolve them, producing ENOENT. Any Windows-host Buck2 build using Linux RE that touched third-party2 prebuilts failed.
**Approach:** Normalize `\` to `/` at the constructor boundary inside `ExternalSymlink::new`, gated on `cfg!(windows)`. Places the invariant ("ExternalSymlink targets are POSIX-style strings") at the type's constructor, where no future caller can bypass it. Factored into a pure helper `normalize_target_for_re` so both branches can be tested on any platform.
**Mechanism:** 2 files changed, 155 insertions, 3 deletions.
**Scale implications:** Cross-platform build systems have a persistent problem: the host running the build tool may differ from the target where actions execute. Path separator normalization must happen at the boundary, not propagate through the system. By putting the fix at the constructor, the invariant is enforced for all future callers automatically.
**Cost:** Significant but targeted to the one unfixed branch.

---

## Add `enforce_visibility_intersection()` PACKAGE function for AND-based visibility caps (2025)
**Author:** Jiawei Lv  
**Situation:** Buck2 needed AND-based visibility composition — the ability to say a target must be visible to both package A AND package B, not either/or.
**Approach:** Pure type infrastructure: new `Intersection(ThinArcSlice<VisibilityPatternList>)` variant whose `matches_target` requires every sub-list to match. Flattening of nested intersections, identity handling for `Public`.
**Mechanism:** 33 files changed, 416 insertions, 5 deletions.
**Scale implications:** Visibility rules are a security boundary in build systems. The ability to compose visibility constraints with AND semantics enables more precise access control. The infrastructure approach (building the type before wiring it up) is deliberate — the follow-up diff actually uses it.
**Cost:** Large infrastructure change.

---

## Add `Intersection` variant to `VisibilityPatternList` (2025)
**Author:** Jiawei Lv  
**Situation:** Continued from `enforce_visibility_intersection()` — the type infrastructure for intersection-based visibility composition.
**Approach:** Paths that cannot legally see an `Intersection` (serialization, `any_matches`, attribute serialization, BXL) return `internal_error!` or `unreachable!`. The design is explicit about what operations are and aren't supported on the new variant.
**Mechanism:** 5 files changed, 156 insertions, 10 deletions.
**Scale implications:** Adding a new variant to a discriminated union requires audit of every match site. Returning errors at boundaries where the new variant isn't legal is the right approach — it forces callers to handle the new type rather than silently ignoring it.
**Cost:** Moderate.

---

## Return Result from `PagableStorage::store_data` (2025)
**Author:** Chris Tolliday  
**Situation:** Storage operations that fail should return errors, not panic. "Normal for storing in a DB to return an error, shouldn't panic if storing data fails."
**Approach:** Change `store_data` signature to return `Result`. Update the sled implementation and trait. Normal DB errors are handled gracefully.
**Mechanism:** 5 files changed, 21 insertions, 31 deletions.
**Scale implications:** Pagable storage is the foundation of DICE state persistence. A panic in the storage layer kills the daemon and loses work. Returning errors allows the system to report failures and potentially retry or degrade gracefully.
**Cost:** Moderate reduction in code (net deletion).

---

## Add SqliteBackedPagableStorage (2025)
**Author:** Chris Tolliday  
**Situation:** Sled was the existing storage backend but had performance and memory characteristics that weren't optimal for the workload. SQLite offered better performance and memory efficiency.
**Approach:** Implement SQLite as an alternative pagable storage backend. Benchmark both with real workloads: 1M keys small values and 100K keys large values. Results show SQLite is faster and more memory-efficient across the board.
**Mechanism:** 8 files changed, 212 insertions, 9 deletions. Benchmarks show SQLite vs sled:
- 1M keys small values: compute 5.41s vs 7.39s, page_out 25.46s vs 37.37s, page_in 10.28s vs 22.18s
- 100K keys large values: compute 0.56s vs 0.63s, page_out 12.27s vs 15.00s, page_in 32.48s vs 34.65s
**Scale implications:** Storage backend choice has massive implications for the build system's performance at scale. The decision is data-driven with benchmark results. "Already faster and more memory efficient than sled, but will be faster with later diffs" — this is an evolving optimization story.
**Cost:** Large addition, but improves build performance measurably.

---

## Unify `SameHeapPtr` and `CrossHeapPtr` into a single explicit-heap_id wire variant (2025)
**Author:** Chenhao Zuo  
**Situation:** The encoding of `FrozenValue` inside a pagable-serialized `Arc<T>` was context-dependent. The wire format distinguished `SameHeapPtr` (no heap_id, implicit "current heap") from `CrossHeapPtr` (explicit heap_id). The choice was driven by `StarlarkSerializerImpl::current_heap_id` at first encode, making the encoding non-deterministic when multiple heaps held the same arc.
**Approach:** Collapse both variants into a single `HeapPtr { heap_id, offset, is_str }` that always carries an explicit `heap_id`. Encoding becomes a pure function of `(state, raw_ptr, is_str)` — no current-heap consultation, no session-context state crossing arc boundaries.
**Mechanism:** 6 files changed, 77 insertions, 198 deletions.
**Scale implications:** Non-deterministic serialization is a correctness bug that can cause subtle failures in distributed builds where the same action is serialized by different heaps in different orders. Pure functions are far more testable and predictable.
**Cost:** Significant refactor enabling subsequent simplifications.

---

## Flatten StarlarkSerState lookup to a single ptr → (heap_id, offset) map (2025)
**Author:** Chenhao Zuo  
**Situation:** After unifying SameHeapPtr/CrossHeapPtr, the encoder lookup degraded to a linear scan over `HashMap<HeapRefId, HashMap<usize, ArenaOffset>>` because there was no longer a "current heap" perf hint.
**Approach:** Replace the nested map with a flat `ptr_to_location: HashMap<usize, (HeapRefId, ArenaOffset)>` plus `registered_heaps: HashSet<HeapRefId>` for bookkeeping. Lookup becomes a single `ptr_to_location.get(&raw_ptr)` — O(1).
**Mechanism:** 1 file changed, 32 insertions, 17 deletions.
**Scale implications:** Serialization performance matters because it happens for every frozen value in the graph. O(n) lookup in the encode path would be a scalability bottleneck. The O(1) flat map recovers and improves on the original per-heap lookup pattern while being simpler.
**Cost:** Modest change with significant performance impact.

---

## [buck2] Use DashMap for SessionContext (2025)
**Author:** Chris Tolliday  
**Situation:** Need to lock `Mutex<SessionContext>` for serialization, making it impossible to serialize anything in parallel. The mutex was the bottleneck.
**Approach:** Replace the inner mutex-protected map with DashMap (ShardedHashMap) which allows concurrent access without a single global lock.
**Mechanism:** 10 files changed, 71 insertions, 77 deletions.
**Scale implications:** Parallelization of serialization enables better throughput on multi-core machines. The inner StarlarkSerContext/StarlarkSerdeContext still uses a mutex (addressed in next diff), but this is progress.
**Cost:** Moderate.

---

## Add hydration/paging benchmark (2025)
**Author:** Chris Tolliday  
**Situation:** Needed a way to measure and compare the performance of DICE hydration and paging operations across different storage backends and configurations.
**Approach:** Standalone DICE binary (`hydration_bench.rs`) plus a runner (`bench_runner.rs`) that formats results as markdown tables with aggregation. Also adds `edge_count` to DICE to measure memory overhead.
**Mechanism:** 9 files changed, 1069 insertions.
**Scale implications:** Performance work requires measurement. Having a reproducible benchmark enables data-driven optimization decisions. The runner can be configured with perf/jemalloc profiling snapshots at each benchmark stage.
**Cost:** Large addition, but provides the infrastructure for all subsequent perf work.

---

## Remove expensive operations from Dice::metrics (May 2026)
**Author:** Chris Hopman  
**Situation:** `Dice::metrics` was called periodically during buck commands and was locking the core state to iterate over all dice nodes — an O(# of dice nodes) operation that is unacceptable in a hot path.
**Approach:** Remove the expensive operations from the metrics call.
**Mechanism:** 5 files changed, 2 insertions, 22 deletions.
**Scale implications:** Periodic operations in a build system must be cheap. Metrics collection that takes O(nodes) every time it runs will become a scalability bottleneck as the graph grows. The fix is to remove the expensive parts rather than cache them, suggesting the metrics design needs rethinking.
**Cost:** Small.

---

## Drop buck2_hash dep from static_interner (2025)
**Author:** Neil Mitchell  
**Situation:** `static_interner` only used `buck2_hash` for `BuckDefaultHasher`, which is literally a type alias for `std::collections::hash_map::DefaultHasher`. This was blocking crates.io publication of `static_interner` because `buck2_hash` is internal.
**Approach:** Switch `Interner`'s default hasher to `DefaultHasher` directly (same type, zero behavioral change). All buck2 callsites use the `interner!` macro which passes the hasher explicitly, so they're unaffected.
**Mechanism:** 3 files changed, 11 insertions, 14 deletions.
**Scale implications:** Dependency on internal crates blocks open-source publication. Removing unnecessary dependencies is a prerequisite for OSS releases. The fix is trivial (type alias to the same underlying type) but the impact on publishability is significant.
**Cost:** Small.

---

## Drop unused gazebo dependency from pagable (2025)
**Author:** Neil Mitchell  
**Situation:** `gazebo::variants::VariantName` derive on `PagableArcInnerState` was unused — nothing called `.variant_name()` on it. `gazebo` was blocking `static_interner` crates.io republish through the transitive dependency tree.
**Approach:** Remove the derive, which removes the `gazebo` dependency from `pagable`, which removes `gazebo` and `gazebo_derive` from the set of crates needing matching crates.io releases.
**Mechanism:** 3 files changed, 1 insertion, 4 deletions.
**Scale implications:** Dead code and unnecessary dependencies compound. Each unnecessary dep is a maintenance burden and a publication blocker.
**Cost:** Minimal.

---

## Properly allocative-attribute Starlark values with extra storage (2025)
**Author:** Jeremy Braun  
**Situation:** Frozen list, tuple, array, any-array, and string trailing storage bytes were being charged to the container type's self-size rather than being attributed to `content`, `unused_capacity`, or `padding` children. This meant memory accounting for these types was inaccurate.
**Approach:** Add an `AValue` hook for reporting inline extra payloads that live after the Rust payload in Starlark arena allocations. Use it from arena allocative traversal.
**Mechanism:** 7 files changed, 96 insertions, 1 deletion.
**Scale implications:** Accurate memory accounting matters for memory profiling and for understanding where build memory goes. This ensures that arena-allocated values with trailing storage are correctly attributed.
**Cost:** Small.

---

## Add network_access to executor config (May 2026)
**Author:** Callum Ryan  
**Situation:** Network access policy needed to be expressible at the executor level, not just at the individual request level.
**Approach:** Add `network_access` parameter to `CommandExecutorConfig` and carry the effective policy through command preparation for remote and local execution. Request-level network access still wins, executor config provides the default.
**Mechanism:** 10 files changed, 77 insertions, 3 deletions.
**Scale implications:** Build systems need to enforce security boundaries around network access. Action execution can be restricted from making network calls, which is important for hermetic builds.
**Cost:** Moderate.

---

## Fix daemon process title lost when spawned via systemd-run (May 2026)
**Author:** Ben Carr  
**Situation:** When `systemd-run --scope` spawns the daemon, the client-side `cmd.arg0("buck2d[repo]")` applies to the systemd-run process, not the actual daemon. The daemon shows as `buck2-daemon` instead of `buck2d[fbsource]` in `top`/`ps`.
**Approach:** For systemd spawns, wrap the daemon invocation in `bash -c 'exec -a "$0" "$@"'` so the daemon gets the desired argv[0]. For non-systemd spawns (including macOS), the existing `cmd.arg0()` call continues to work. On Windows, no process title is set.
**Mechanism:** 6 files changed, 97 insertions, 15 deletions. Includes a regression test.
**Scale implications:** Process titles are how operators identify which process is which in monitoring and debugging. A daemon that can't set its title is harder to operate. The systemd-run wrapper is a pragmatic solution to a platform-specific constraint.
**Cost:** Moderate with test.

---

## feat: expose comments to parsing without changing API (May 2026)
**Author:** Tod Hansmann  
**Situation:** External tools (like Gazelle) want to read comments from BUILD files to merge them into the resulting BUILD output, but the current API doesn't expose them.
**Approach:** Add comment exposure to the parser without changing the public API — existing usage is unaffected.
**Mechanism:** 1 file changed, 68 insertions, 1 deletion.
**Scale implications:** Build file metadata preservation is important for build tool interoperability. Gazelle and similar tools need to preserve comments when regenerating BUILD files.
**Cost:** Small but enables external tool integrations.

---

## feat: expose positional to the public api (May 2026)
**Author:** Sahin Yort  
**Situation:** There was no way to read positional arguments for types implementing the `invoke` function from `StarlarkValue` trait.
**Approach:** Expose positional arguments to the public API.
**Mechanism:** 1 file changed, 1 insertion, 4 deletions (reduction due to simplification).
**Scale implications:** Embedders often build host-side constructors that need to inspect callable arguments. This enables patterns like `handler("name", run=my_fn)` where the host needs to know the name of the callable.
**Cost:** Small.

---

## feat: expose RecordType from record_type module (May 2026)
**Author:** Sahin Yort  
**Situation:** Needed to accept arguments that are of type `record` in StarlarkValue implementations.
**Approach:** Export `RecordType` from the `record_type` module to the public API.
**Mechanism:** 1 file changed, 1 insertion.
**Scale implications:** Record types needed to be accessible to embedders for type-checking purposes.
**Cost:** Minimal.

---

## Use std::ptr::copy_nonoverlapping instead of std::intrinsics (May 2026)
**Author:** Neil Mitchell  
**Situation:** `std::intrinsics::copy_nonoverlapping` is deprecated. The stable `std::ptr::copy_nonoverlapping` has existed since Rust 1.0.
**Approach:** Use the stable equivalent.
**Mechanism:** 1 file changed, 1 insertion, 1 deletion.
**Scale implications:** Deprecation warnings in codebases become noise that obscures real issues. Keeping up with API changes avoids technical debt accumulation.
**Cost:** Trivial.

---

## Make ExternalRunnerTestInfo constructor args named-only (May 2026)
**Author:** Jakob Degen  
**Situation:** All callsites already used keyword arguments exclusively, but the constructor didn't enforce this.
**Approach:** Add `#[starlark(require = named)]` to enforce consistent named-only usage at the type level.
**Mechanism:** 1 file changed, 14 insertions, 15 deletions.
**Scale implications:** Enforcing API contracts at the type level prevents accidental positional usage. The refactor is basically cosmetic since all callers were already compliant, but it makes the contract explicit.
**Cost:** Modest.

---

## Add JVM Per-test coverage listener (May 2026)
**Author:** Omkar Yadav  
**Situation:** JUnit-based Java frameworks only had bundle-level coverage because all test methods in a target produced one combined `.exec` file. This meant test selection could only happen at target granularity, not per test method.
**Approach:** `PerTestJUnitCoverageRunListener` uses JaCoCo's runtime agent API to reset probes before each test and dump execution data after each test. Each test method gets its own `.exec` file with a manifest mapping test names to files.
**Mechanism:** 4 files changed, 420 insertions. This diff adds the listener and tests only — integration into JUnitRunner and coverage pipeline follows in a subsequent diff.
**Scale implications:** Per-test coverage enables fine-grained test selection in RELATES. The JaCoCo agent API calls (reset, getExecutionData) are just memcpys so overhead should be negligible.
**Cost:** Large, but staged — the listener is built but not yet wired up.

---

## Add bloks_registrations attribute and provider propagation to android_library (May 2026)
**Author:** Minyu Li  
**Situation:** `capabilities_registrations` needed to be a first-class attribute on `android_library` with proper provider propagation via tsets.
**Approach:** Declare `capabilities_registrations` attribute in `android_rules.bzl` and wire up `CapabilitiesRegistrationInfo` provider propagation in `android_library.bzl`.
**Mechanism:** 2 files changed, 4 insertions, 1 deletion.
**Scale implications:** This is about wiring new Android capabilities infrastructure through the build rules. The staged approach (adding the attribute, then propagating the provider) is the right pattern for rule infrastructure.
**Cost:** Small.

---

## Revert D96950683: Upgrade Kotlin to 2.2 (May 2026)
**Author:** Kaining Mao  
**Situation:** Kotlin 2.2 upgrade was attempted but had to be reverted.
**Approach:** Revert the Kotlin version change.
**Mechanism:** 1 file changed, 4 insertions, 2 deletions.
**Scale implications:** Major version upgrades of key languages (Kotlin, Java, etc.) in a build system require extensive validation. The revert indicates something in the upgrade didn't work in production. This is the cost of staying current — upgrades sometimes need to be rolled back.
**Cost:** Minimal.

---

## Remove KSP1 step from the toolchain (2025)
**Author:** Alexey Soshin  
**Situation:** KSP1 (Kotlin Symbol Processing) won't be supported in Kotlin 2.3, so the class representing KSP1 step needs to be removed from the toolchain.
**Approach:** Remove `Ksp1Step.java` and `KspStepsBuilder.java` — 221 deletions across 4 files.
**Mechanism:** 4 files changed, 2 insertions, 221 deletions.
**Scale implications:** Deprecating a processing step in a build toolchain requires removing it from all rules that reference it. The "won't be supported in 2.3" rationale is clear — don't invest in something that's being removed.
**Cost:** Significant deletion.