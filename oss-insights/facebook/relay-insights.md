# Relay Repository — OSS Insights

Analyzed: 500 commits from 2023-01-01 to 2026-05-21 (most recent 500 per `git log --oneline --since="2023-01-01"`). Sample of ~35 substantive commits selected: architectural changes, major bugfixes, performance work, breaking API changes, new features. Skipped: version bumps, Cargo.lock updates, clippy auto-fixes, dependency bumps.

---

## Key Commits — Substance Format

### relay c26dc44a Add serverPreloadQuery and useQueryFromServer to rsc_EXPERIMENTAL

**Author:** Alison Lee  
**Situation:** React Server Components (RSC) required a mechanism for server-side data preloading that could be passed to the client without blocking rendering. The existing `useLazyLoadQuery` was client-centric — it couldn't synchronously return a preloaded ref, and client-side hydration had no standardized way to consume server-preloaded data.

**Approach:** Introduced a two-sided API: `serverPreloadQuery(query, variables)` on the server returns a `PreloadedQueryRef` synchronously (the promise is stored inside the ref, so callers cannot accidentally await the function and block rendering). On the client, `useQueryFromServer(query, queryRef)` uses React 19's `use()` to unwrap the promise and publish data to the store **without notifying subscribers** — avoiding state-update-during-render errors from overlapping subscriptions. Supports a configurable staleness threshold (default 30s) that triggers a client-side refetch when server data is too old.

**Mechanism:** `serverPreloadQuery` stores the response as a Promise inside a ref. `useQueryFromServer` calls `use()` on the Promise, normalizes the data via `defaultGetDataID` exported from `relay-runtime.__internal`, and publishes without `notify()` to avoid subscription during render. The staleness check runs after hydration — if the server timestamp exceeds the threshold, a refetch is triggered.

**Scale implications:** This is a fundamental piece of infrastructure for Relay's RSC story. The design constraint — cannot block on data fetch in RSC — drove the synchronous-return pattern. The "publish without notifying" behavior is critical for avoiding cascading re-renders during hydration. This pattern will likely become the default for any RSC-compatible data API.

**Cost:** Added new exports to `relay-runtime.__internal` (leaky abstraction). The staleness threshold is a new concept that users must understand.

---

### relay 510cf1ea Add serverReadFragment to rsc_EXPERIMENTAL

**Author:** Alison Lee  
**Situation:** RSC needed equivalent of `useFragment` for server-side — reading fragment data that was already populated by a prior server-side fetch. Existing `useFragment` is a React hook and assumes client-side store state.

**Approach:** `serverReadFragment` delegates to `waitForFragmentData` from `relay-runtime/experimental`. Available as a pre-bound method on the `ServerEnvironment` returned by `createServerEnvironment`, which wraps the user-provided Environment factory in `React.cache()` for per-request isolation.

**Mechanism:** The pattern mirrors `serverFetchQuery`: a factory creates a cached environment per request, and methods are pre-bound so callers don't thread the environment through. Fragment reading happens against store data populated by prior server operations.

**Scale implications:** The per-request isolation via `React.cache()` is essential — without it, server environments could leak data between requests in concurrent RSC rendering. This pattern will be repeated for any server-side Relay APIs.

**Cost:** Small commit (253 lines, 4 files). Minimal API surface.

---

### relay 8a2fff47 Experimental serverFetchQuery for data fetching in RSC

**Author:** Alison Lee  
**Situation:** The React Server Components model requires server-side data fetching that integrates with React's rendering model. Relay had no entry point for this — all existing APIs assumed a client-side environment.

**Approach:** `createServerEnvironment` wraps a user-provided Environment factory in `React.cache()` for per-request isolation. Returns a `ServerEnvironment` object with pre-bound methods (`getEnvironment()`, `serverFetchQuery()`) so callers don't need to pass the environment on every call. `serverFetchQuery` is the server-side equivalent of `useLazyLoadQuery`.

**Mechanism:** The cache key is the factory function, so each request gets its own environment instance. Pre-binding methods removes the need to thread the environment through component trees.

**Scale implications:** This is the foundational RSC integration point. The design choice of pre-binding methods and caching environments per request will shape how all server-side Relay APIs are structured. It establishes a pattern: factory + cache + pre-bound methods.

**Cost:** 209 lines, 5 files. Minimal surface, but experimental.

---

### relay d8bdb2d5 Don't generate GraphQL schemas in daemon

**Author:** Evan Yeung  
**Situation:** The Relay daemon (incremental compilation server) was generating GraphQL SDL schemas as artifacts in watch mode. This was unnecessary overhead — the daemon's purpose is incremental artifact updates, not full schema generation.

**Approach:** Removed the schema generation step from the daemon's build pipeline. Single-file change (11 lines inserted, 1 deleted).

**Mechanism:** Schema generation removed from the daemon build path only. Full builds still generate schemas.

**Scale implications:** The daemon runs continuously in watch mode, so eliminating unnecessary work has compounding impact on developer experience. This is a micro-optimization that reflects the team's philosophy: even small unnecessary operations in long-running processes deserve attention.

**Cost:** Minimal — 11 lines. No API or behavioral change for users.

---

### relay 581029c1 Regression test + fix for mismatched coordinate merging

**Author:** Matt Mahoney  
**Situation:** After switching to schema-coordinate-presence to indicate whether a type is an extension, a specific code path in `fix_all_types()` crashed with "Cannot merge different coordinates." To trigger the bug: create a schema like `interface Inf { field } type Concrete implements Inf` (partial-set territory), then call `.fix_all_types()` without an original schema. Before the fix, `Concrete.field` had coordinate `Iface.field`. Merging with a schema that actually defines `Concrete { field }` caused the panic.

**Approach:** Resolved by setting the LHS of the coordinate to `None` when the mismatch occurs. This means the field gets treated as if it were `extend field: String` — a non-existing-but-representable SDL syntax. Side effect: excluding a fixed schema from one with the field actually defined will not exclude the field (though it will exclude interface-defined directives on it).

**Mechanism:** `merge_coordinate` called with `(Some(existing), Some(incoming))` where `existing != incoming` — instead of panicking, the fix sets `target = None`. This allows the merge to proceed without the coordinate constraint.

**Scale implications:** This is the kind of bug that only surfaces with unusual schema compositions. The team has been actively migrating to schema-coordinate-presence semantics, and this was a hidden interaction. The regression test ensures it stays fixed.

**Cost:** The workaround (treating the field as an extension) is semantically odd but contained. The fix handles the crash case without breaking normal schemas.

---

### relay 6d10a0ce Detect external modifications to generated artifacts

**Author:** Evan Yeung  
**Situation:** If a user manually edits or deletes a generated Relay artifact file while the daemon is running, the compiler's artifact map becomes stale. Subsequent builds would write to stale paths or miss deleted files. The daemon had no mechanism to detect this state.

**Approach:** Added detection logic in `artifact_writer.rs` to check whether artifact files have been modified externally (by comparing stored hashes or modification times). When external modification is detected, the compiler invalidates its cached artifact map and recomputes.

**Mechanism:** `artifact_writer.rs` now checks file modification state before writing. `compiler.rs` handles invalidation. `compiler_state.rs` tracks artifact file state.

**Scale implications:** In large projects with many engineers, generated artifacts are a common source of "works on my machine" issues when someone regenerates while another person is editing. Detecting this state proactively prevents a class of confusing failures.

**Cost:** 34 lines inserted, 3 lines deleted across 3 files.

---

### relay 69021d2b Surface compiler crashes to meerkat clients

**Author:** Evan Yeung  
**Situation:** The Relay daemon (meerkat is the internal name for the daemon/compiler service) was swallowing compiler panics. Clients of the daemon (IDEs, build tools) would hang waiting for a response with no indication that the compiler had crashed.

**Approach:** Added crash reporting via `status_reporter.rs` — the daemon now surfaces compiler crashes to clients instead of silently failing.

**Mechanism:** `status_reporter.rs` gains 12 lines to handle panic propagation.

**Scale implications:** A compiler that crashes silently is a developer experience nightmare. Surfacing crashes allows clients to show errors, retry, or fall back gracefully.

**Cost:** 12 lines, 1 file.

---

### relay fc6be23d Use watchman clock to synchronize file subscription and write events

**Author:** Evan Yeung  
**Situation:** The daemon's file watcher and artifact writer operated independently — there was no synchronization between file change notifications and write completion. In fast edit cycles, the compiler could read a partially-written artifact or miss a change notification.

**Approach:** Used Watchman's clock (a vector clock / logical timestamp) to synchronize the file subscription and write events. The status reporter was extended to track watchman clock values alongside artifact writes.

**Mechanism:** `watchman_file_source.rs` now reads and stores watchman clock values. `status_reporter.rs` tracks these for synchronization. 68-line addition in `watchman_file_source.rs`, 146 lines in `status_reporter.rs`.

**Scale implications:** Watchman is Facebook's file watching system. Using its clock for synchronization means the daemon can correctly order events even under heavy file system activity. This is critical for incremental compilation correctness.

**Cost:** 262 lines inserted, 5 deleted. Significant infrastructure investment.

---

### relay 520ffc1e Fix incremental bug: multi-project cross-fragment crash in CommonJS mode

**Author:** Evan Yeung  
**Situation:** In multi-project Relay setups with CommonJS output, incremental compilation crashed when a fragment was referenced across projects. The bug was specifically in how CommonJS module exports were handled during incremental builds — cross-project fragment references weren't being resolved correctly.

**Approach:** Added a regression test fixture and fixed the `build_project.rs` logic for cross-project fragment handling in CommonJS mode.

**Mechanism:** The fix involved ensuring that cross-project fragment references are correctly resolved and emitted in CommonJS artifacts during incremental builds. 356 lines added (including test fixture).

**Scale implications:** Multi-project setups are common in large codebases. The CommonJS + incremental combination exposed an interaction that single-project or non-incremental builds wouldn't catch. This is why they run full builds alongside incremental builds.

**Cost:** 356 lines (test fixture dominates). No API change.

---

### relay e4f7cf19 Add S2C execution to Scope: handleS2CExecutions, readInto, createRootFragmentNode

**Author:** Tianyu Yao  
**Situation:** Server-to-client (S2C) resolvers are Relay's mechanism for client-defined fields that delegate to server-side logic. The `Scope` class needed to execute S2C resolvers when reading data — without this, client resolvers would silently fail or return undefined during store reads.

**Approach:** Extended `Scope` with three new methods: `handleS2CExecutions`, `readInto`, `createRootFragmentNode`. These handle the execution of S2C resolvers and the population of results into the Relay store.

**Mechanism:** `RelayResponseNormalizer.js` gains 46 lines, `RelayStoreTypes.js` gains 6 lines. The execution flow: when reading from a Scope, if an S2C resolver is encountered, `handleS2CExecutions` processes it and the result is written via `readInto`.

**Scale implications:** S2C resolvers are a key piece of Relay's client-side computed field story. This is Phase 3 of the defer/stream work (which itself is multi-phase). The team is building toward full incremental streaming support.

**Cost:** 52 lines across 2 files.

---

### relay f61d5575 Phase 3: @defer/@stream support in NormalizationEngine

**Author:** Tianyu Yao  
**Situation:** The NormalizationEngine (which normalizes raw server responses into the Relay store) had no support for @defer or @stream directives. As the team moved toward full incremental delivery support, this was a critical missing piece.

**Approach:** Implemented Phase 3 of the defer/stream work in the NormalizationEngine. This handles the actual normalization of deferred/stream payloads — processing the incremental payloads and correctly populating the store.

**Mechanism:** 718 lines added to `NormalizationEngine.js`, 38 deleted. `NormalizationEngine-test.js` modified. This is the execution engine for incremental delivery — when a deferred fragment arrives, this code normalizes it into the store correctly.

**Scale implications:** @defer/@stream are fundamental to GraphQL incremental delivery. Getting this right in the NormalizationEngine means Relay can correctly handle progressive response loading, which is critical for performance in large apps.

**Cost:** 706 lines added, 38 deleted. A massive commit that completes the core defer/stream implementation.

---

### relay 75207873 Phase 1+2: Hoist normalization to network layer — infrastructure + initial responses

**Author:** Tianyu Yao  
**Situation:** Normalization was happening in the OperationExecutor (network layer) but was conceptually coupled to the store. The team wanted to move normalization closer to the network layer so it could handle incremental responses correctly before they reach the store.

**Approach:** Phase 1+2: Infrastructure and initial response handling. Created `NormalizationEngine.js` as a standalone module, integrated it into `OperationExecutor.js`, and added test fixtures.

**Mechanism:** `NormalizationEngine.js` is a new module (113 lines) that handles the normalization logic in the network layer. `OperationExecutor.js` updated (68 lines) to use the new engine. Tests added (166 lines).

**Scale implications:** Moving normalization to the network layer is a significant architectural shift. It means responses are normalized before they hit the store, which is necessary for incremental/streaming responses. Phase 3 (the actual defer/stream handling) builds on this infrastructure.

**Cost:** 442 lines added, 7 deleted. This is foundational infrastructure for the later Phase 3 work.

---

### relay b2dd4372 Remove @live_query directive support from Relay compiler

**Author:** Xiangxin Sun  
**Situation:** The @live_query directive was being deprecated in favor of @client_polling. The Relay compiler still generated metadata for @live_query, and there was active migration happening on the client side.

**Approach:** Removed @live_query directive support from the Relay compiler. This was a deletion-focused commit (6 lines inserted, 50 deleted in `generate_live_query_metadata.rs`).

**Mechanism:** The compiler no longer processes @live_query directives. Client-side code had already been migrated to @client_polling. This is a clean removal of deprecated infrastructure.

**Scale implications:** Deprecating directives is risky — if any production code still uses @live_query, generated artifacts will be incorrect. The team ran the migration in stages (client-side first, then compiler), ensuring no gap where both sides were out of sync.

**Cost:** 50 lines deleted. Users on the old directive will get compiler errors.

---

### relay c356dc12 Fix bug with conflicting @match fields

**Author:** Evan Yeung  
**Situation:** When multiple fragments using @match selected the same field with different match types (different fragment spreads), the Relay compiler didn't validate this conflict correctly. This could lead to incorrect generated code.

**Approach:** Added validation in `validate_selection_conflict.rs` to detect conflicting @match fields across fragments. Added extensive test fixtures (12 files, 719 lines).

**Mechanism:** `validate_selection_conflict.rs` gains 69 lines. Multiple test fixtures cover scenarios: multiple-match-conflicts across fragments, same-field-different-match, same-field-same-match with/without supported directives.

**Scale implications:** @match is used for fragment-level data masking/conditional rendering. Conflicts across fragments are hard to reason about — this validation ensures developers catch these issues at compile time rather than runtime.

**Cost:** 719 lines (mostly test fixtures). No runtime cost — purely compile-time validation.

---

### relay ee2d2354 Report all fragment validation errors instead of stopping at first

**Author:** Evan Yeung  
**Situation:** The `validate_selection_conflict` pass would stop at the first error it found. If a query had multiple independent conflicts, the developer would only see the first — fix it — then discover the next one. This created a frustrating iteration loop.

**Approach:** Changed the validation to collect all errors before reporting them. Added test fixtures for independent fragment conflicts (7 files, 321 lines).

**Mechanism:** Instead of `return Err` on first conflict, the validator collects into a `Vec` and reports all at once. The change is in `validate_selection_conflict.rs` (34 lines changed).

**Scale implications:** Error messages that surface all problems at once are significantly better for developer experience. This transforms the compiler from a "fix one, recompile, fix next" loop to a "fix everything in one pass" experience.

**Cost:** 321 lines (test fixtures). 34 lines actual logic change.

---

### relay 4613c9d1 Support @catch directive on client edge fields

**Author:** Xin Chen  
**Situation:** The Relay compiler blocked `@catch` on client edge fields with "Unexpected directive on Client Edge field." This was a directive allowlist omission in `client_edges.rs` — `catch` was never added to the list. However, `throwOnFieldError` on fragments with client edges causes UI sections to disappear when client edge data is transiently unavailable (initial load, campaign switching, store GC). The natural fix is `catch(to: NULL)` but it was blocked.

**Approach:** Added `catch` to the client edge directive allowlist in `client_edges.rs`. Hoisted `CatchMetadataDirective` from the field onto the wrapping inline fragment (same pattern as `required`). Added `catch` to the resolver field directive filter in `field_transform.rs`. Added comprehensive tests.

**Mechanism:** `verify_directives_or_push_errors` in `client_edges.rs` now includes `CATCH_DIRECTIVE_NAME`. The hoisting pattern mirrors `RequiredMetadataDirective` — the directive moves from field to inline fragment wrapper to be semantically correct.

**Scale implications:** 96 files in the ads editor had `throwOnFieldError` + `relay_everywhere` with client edges that would silently fail when data was transiently unavailable. Now they can use `catch(to: NULL)` to gracefully handle this instead of showing a blank section.

**Cost:** Multiple files touched (allowlist, hoisting logic, resolver filter, test fixtures).

---

### relay d0541a30 Add `query-stats` subcommand for per-operation fragment usage analysis

**Author:** Tianyu Yao  
**Situation:** Teams wanted to understand fragment usage patterns — which fragments are used by which operations, what is the dependency graph, which fragments are expensive or frequently included. No tooling existed for this.

**Approach:** Added a `query-stats` subcommand to the Relay compiler CLI. It performs per-operation fragment usage analysis and outputs usage statistics.

**Mechanism:** New file `query_stats.rs` (314 lines) in `dependency-analyzer`. Comprehensive test fixtures (19 files total, 797 lines). The analyzer traces fragment usage through operations and produces statistics.

**Scale implications:** In large codebases, understanding which fragments are used where is crucial for performance work. Fragment composition can create hidden N+1 patterns. This tooling enables teams to analyze their Relay usage and identify optimization opportunities.

**Cost:** 797 lines (test fixtures + implementation). New CLI surface.

---

### relay 548434a5 Include docs in NPM package for LLM/agent access (#5237)

**Author:** Jordan Eldredge  
**Situation:** LLMs and agents working in Relay codebases needed documentation access. The docs were on a website, but agents typically work in the codebase with network access being optional or restricted.

**Approach:** Added a `copyDocs` gulp task that copies `website/docs/**/*.mdx` files into `dist/relay-runtime/llm-docs/` during the build. Added docblock comments to entrypoint files pointing agents to the docs. Excluded `FbFakeContent.mdx` and versioned docs.

**Mechanism:** 129 doc files are copied into the package. Entry point files get docblock pointing to `node_modules/relay-runtime/llm-docs/`.

**Scale implications:** This is a reaction to the growing importance of AI-assisted development. Shipping docs in the package means any agent working with Relay can read the documentation without network access. This reflects a broader trend of "AI-first" package publishing.

**Cost:** Build task + doc comments. No runtime cost.

---

### relay c627c9ec Support multiple source locations in SchemaDefinitionItem

**Author:** Matt Mahoney  
**Situation:** `SchemaSet` definitions can be merged from multiple SDL source files. Previously `SchemaDefinitionItem` stored a single location via `name: WithLocation<StringKey>` — only one source could be tracked. This was insufficient for correct source tracking in merged schemas.

**Approach:** Changed `SchemaDefinitionItem.name` to `StringKey` (from `WithLocation<StringKey>`) and added a new `locations: Vec<Location>` field. All ~30 creation sites updated. `Merges` impls now combine locations via a new `merge_definition()` helper. External consumers updated.

**Mechanism:** The key structural change: name is no longer location-bearing. Locations are stored separately as a vector. When two definitions merge, their locations are combined.

**Scale implications:** SchemaSet merging is a core operation for Relay's multi-schema support. Correct location tracking is essential for error messages, IDE integration (go-to-definition), and schema diffing. This change fixes the data structure to support the actual use case.

**Cost:** ~30 creation sites across multiple files. Significant refactor but mechanically straightforward.

---

### relay 9f298caa Remove Node interface restriction for mixed interface server types

**Author:** Jordan Eldredge  
**Situation:** The Relay compiler enforced a `Node` interface restriction for mixed interface server types in client edges. This was overly restrictive — you can have a server type that implements a mixed interface without implementing `Node`.

**Approach:** Removed the restriction in `client_edges.rs`. Also updated `refetchable_fragment.rs` and `build_ast.rs`. 459 lines added, 66 deleted across 12 files.

**Mechanism:** The restriction was a validation rule that was too strict. Removing it required updating the type checking logic in multiple compiler crates.

**Scale implications:** This expands the design space for client edges — developers can now use server types that don't implement `Node` as targets for client edges on mixed interfaces. This is a loosening of constraints that enables more GraphQL patterns.

**Cost:** 459 lines changed across 12 files. No API change but type validation logic is different.

---

### relay c81d7dfe Optimize client_extensions transform — 89% faster via PointerAddress cache

**Author:** Tianyu Yao  
**Situation:** The `client_extensions` transform was a performance bottleneck. Profiling showed it was spending significant time on pointer comparisons that could be cached.

**Approach:** Replaced pointer comparison logic with a `PointerAddress` cache. The optimization achieved 89% speedup on the transform.

**Mechanism:** The change is in `client_extensions.rs` — 33 lines added, 8 deleted. Pointer addresses (memory addresses as keys) are cached to avoid repeated pointer comparisons.

**Scale implications:** `client_extensions` is used frequently in Relay's compilation pipeline. A near-90% speedup in one transform has significant impact on overall compilation time, especially for large projects.

**Cost:** 41 lines changed. No API surface.

---

### relay c85259ec Gate @RelayResolver usage behind allow_legacy_relay_resolver_tag in relay-schema-generation

**Author:** Jordan Eldredge  
**Situation:** The `@RelayResolver` tag was being deprecated in favor of `@relayType`/`@relayField`. The old syntax needed to be gated behind a feature flag to allow migration without breaking existing code.

**Approach:** Added a `allow_legacy_relay_resolver_tag` config option. When disabled, the old `@RelayResolver` syntax produces an error. The new `@relayType`/`@relayField` syntax is the default.

**Mechanism:** Config option in `relay-compiler/src/config.rs`. Error handling in `relay-schema-generation/src/errors.rs`. Error messages guide users to the new syntax.

**Scale implications:** Large migrations need a staged approach — old code must continue to work while new code migrates. This gating mechanism allows the team to eventually remove the old syntax without breaking production code during the transition.

**Cost:** 103 lines in `lib.rs`, 56 in `errors.rs`, plus tests. The feature flag approach is the standard migration pattern.

---

### relay dc0addb8 Fix infinite recursion in InexactObject::Ord::cmp

**Author:** Evan Yeung  
**Situation:** `InexactObject::Ord::cmp` had an infinite recursion bug. When comparing two `InexactObject` values, the implementation would recursively call `cmp` on the same type, leading to a stack overflow.

**Approach:** Fixed the comparison implementation in `relay-typegen/src/writer.rs`. Added 62 lines, removed 3.

**Mechanism:** The `Ord` implementation for `InexactObject` was self-referential in a way that caused infinite recursion. The fix restructures the comparison to avoid the recursion.

**Scale implications:** Type generation that involves comparing `InexactObject` types (common in Relay's generic types) would stack overflow. This fixes a correctness bug in the type system.

**Cost:** 65 lines changed. Correctness fix.

---

### relay 9716a07e Replace RwLock with DashMap in flatten transform caches

**Author:** Tianyu Yao  
**Situation:** The `flatten` transform used `RwLock` for its cache. `RwLock` has more overhead than `DashMap` for concurrent access patterns where reads are far more frequent than writes.

**Approach:** Replaced `RwLock` with `DashMap` in the flatten transform caches. `DashMap` is a concurrent hash map implementation that avoids the synchronization overhead of `RwLock` for the typical read-heavy workload.

**Mechanism:** `flatten.rs` — 13 lines inserted, 23 deleted. The cache access pattern stays the same but the underlying data structure changes.

**Scale implications:** The flatten transform runs on every Relay compilation. Reducing synchronization overhead compounds across all compilations. This is a low-level performance improvement.

**Cost:** 36 lines changed. No behavioral change.

---

### relay 884c06ea Add incremental compilation test for schema field nullability change

**Author:** Jordan Eldredge  
**Situation:** The team wanted to ensure incremental compilation produces the same output as full compilation for schema changes. Specifically, changing a root query field from non-nullable (!) to nullable should correctly update generated artifacts.

**Approach:** Added an integration test that verifies this scenario — full build output vs incremental build output should be identical after a nullability change.

**Mechanism:** Test fixture (139 lines) + integration test modifications (9 lines). The test runs both a full build and an incremental build with the schema change and compares outputs.

**Scale implications:** Schema nullability changes are a common migration. Ensuring incremental compilation handles them correctly prevents a class of subtle runtime bugs where the generated code doesn't match the schema.

**Cost:** 139 lines (test fixture). 9 lines actual test logic.

---

### relay 160bcf5c Fix incremental build missing mutation field return type changes

**Author:** Tianyu Yao  
**Situation:** Incremental builds were missing updates when a mutation field's return type changed. The schema change analyzer wasn't detecting this case, so generated artifacts weren't regenerated.

**Approach:** Added detection in `schema_change_analyzer.rs`. Added a test fixture demonstrating the bug (150 lines expected output, 39 lines input).

**Mechanism:** The schema change analyzer now tracks mutation field return type changes as a trigger for artifact regeneration.

**Scale implications:** Mutations with changed return types would silently use stale generated code in incremental builds. This could cause runtime type errors that are hard to debug. The fix ensures correctness.

**Cost:** 4 files, 202 lines (test dominates). No API change.

---

### relay 5c33c910 Fix stack overflow in graphql-merge-sdl for large schemas

**Author:** Mathew Luo  
**Situation:** `prettier_print_schema_document` used `RcDoc::intersperse` which builds a left-nested `Append` chain O(N) deep. For schemas with tens of thousands of definitions, the compiler-generated recursive `Drop` for `Rc<Doc>` overflows the default 8 MB main thread stack.

**Approach:** Changed the printing to render each definition independently instead of using `intersperse`, avoiding the deeply-nested tree structure entirely.

**Mechanism:** In `prettier_schema_printer.rs`: render definitions one-by-one rather than building an intersperced document. Output is byte-identical to the previous implementation.

**Scale implications:** Large schemas are increasingly common as Relay is used in bigger deployments. The stack overflow would cause compilation to crash on large schemas — a hard failure that blocks the build.

**Cost:** 7 lines inserted, 10 deleted in one file. Elegant fix to a subtle recursive data structure problem.

---

### relay 11b54958 Replace flatbuffer schema support with compact schema in relay compiler

**Author:** Shashank Kambhampati  
**Situation:** The Relay compiler had support for FlatBuffer-encoded schemas as an alternative to SDL. This added complexity with unclear benefit — the team was moving toward a "compact schema" format instead.

**Approach:** Removed FlatBuffer schema support and replaced it with "compact schema" support. Compact schema is a more maintainable, text-based format. 104 lines deleted, 108 inserted across 11 files.

**Mechanism:** `build_schema.rs`, `compiler_state.rs`, `config.rs`, `file_categorizer.rs`, and multiple other files updated. The FlatBuffer format is no longer accepted — compact SDL is the replacement.

**Scale implications:** Removing a schema format is a breaking change for any team using FlatBuffer-encoded schemas. However, if no external users relied on this, it simplifies the compiler significantly.

**Cost:** 104 inserted, 108 deleted. Significant removal of code.

---

### relay 0dea0b18 Allow missing fields from defer payloads (#5083)

**Author:** Rob Richard  
**Situation:** The GraphQL defer spec requires servers not to send the same field multiple times across deferred and non-deferred fragments. Relay expected each fragment's response to contain all its fields. If a field was in both a deferred and non-deferred fragment, Relay would throw "Payload did not contain a value for field `name`." This blocked compatibility with the defer spec.

**Approach:** Added a `deferDeduplicatedFields` flag that allows Relay to accept payloads with missing fields. The defer spec guarantees a fragment won't be marked complete until all its fields are returned, so the store will already have missing fields from other fragments.

**Mechanism:** A flag in the normalizer that relaxes the field-presence check. The note in the commit is important: this doesn't make Relay fully spec-compliant, but allows a network-layer transformation to convert spec-format responses into Relay-compatible format.

**Scale implications:** The defer spec is still evolving. Relay is maintaining compatibility while the spec stabilizes by adding an escape hatch. Teams using defer can now opt into this behavior.

**Cost:** Minimal code change, new flag. Users must opt in.

---

### relay 34d674c3 Support @client_polling(interval: x) for client polling live query on Relay

**Author:** Nico Tapiero  
**Situation:** `polling_interval` on `@live_query` was being deprecated in favor of `@client_polling(interval:)`. The compiler needed to support the new directive.

**Approach:** Added compiler support for `@client_polling(interval:)` directive. This generates the same live query metadata but with the new directive syntax.

**Mechanism:** `generate_live_query_metadata.rs` gains 30 lines. Test fixtures added (28 lines expected + 13 lines input).

**Scale implications:** This is a migration from `@live_query(polling_interval:)` to `@client_polling(interval:)`. The old directive is deprecated but still works. New code should use the new syntax.

**Cost:** 74 lines (test fixtures + implementation). Migration path.

---

### relay f73e275b Remove unused `glob` dependency

**Author:** Rob Hogan  
**Situation:** The `glob` npm package was in `package.json` but wasn't used anywhere in the codebase.

**Approach:** Removed it. 1 line change (deleted from `package.json`).

**Mechanism:** Dependency removed.

**Scale implications:** Reduces install size and attack surface. Unused dependencies are technical debt that accumulates.

**Cost:** 1 line.

---

### relay 77a2dfb0 Update schema_set_collector.rs to have touch methods to be public

**Author:** Bohan Xu  
**Situation:** Internal refactoring left `touch` methods in `schema_set_collector.rs` as private when they needed to be public for consumers to correctly track schema dependencies.

**Approach:** Made the touch methods public. 17 lines inserted, 21 deleted — a simplification as well as a visibility fix.

**Mechanism:** Visibility change + some simplification of the touch methods.

**Scale implications:** Schema set collectors need to track dependencies for incremental compilation. If touch methods were private, external consumers couldn't properly trigger dependency tracking.

**Cost:** 38 lines net -17.

---

### relay 704d1494 Add logging for DataChecker missing data events

**Author:** Aria Fallah  
**Situation:** `DataChecker` (Relay's internal consistency checker) had no logging when it detected missing data. Debugging data consistency issues was difficult without visibility into what the checker was seeing.

**Approach:** Added logging for DataChecker missing data events. `DataChecker.js` gains 34 lines. `RelayStoreTypes.js` gains 9 lines.

**Mechanism:** When DataChecker detects missing data, it now logs an event with context about what was missing and where.

**Scale implications:** Data consistency issues in Relay can be subtle and hard to reproduce. Logging gives developers and operators visibility into data integrity checks, enabling faster debugging.

**Cost:** 43 lines across 2 files. Debug infrastructure.

---

### relay 85e0875e Configure permissions for cargo lock updater action (#5123)

**Author:** Jordan Eldredge  
**Situation:** The cargo lock updater workflow used a bot PAT with broad permissions. GitHub's recommended practice is to use the default `GITHUB_TOKEN` with minimal scoped permissions.

**Approach:** Added explicit `permissions` block with `contents: write` and `pull-requests: write`. Replaced `RELAY_BOT_GITHUB_PAT` with `${{ secrets.GITHUB_TOKEN }}`.

**Mechanism:** Workflow file updated with minimal permissions. Security improvement.

**Scale implications:** Reduced credential surface. No more bot PAT to manage and rotate.

**Cost:** Workflow configuration change. No code.

---

### relay 3e83d227 Keep locations spec-sorted in print_schema

**Author:** Matt Mahoney  
**Situation:** Two implementations existed for schema printing: `to-sdl` and `print-schema`. They sorted locations differently. When merging the implementations, different sort orders would cause spurious diffs.

**Approach:** Fixed `print_schema_set.rs` to sort locations as defined by the GraphQL spec (`https://spec.graphql.org/draft/#DirectiveLocation`). This ensures consistency with `to-sdl` when the implementations are eventually merged.

**Mechanism:** 9 lines inserted, 6 deleted in `print_schema_set.rs`.

**Scale implications:** The team is working toward unifying two schema printing implementations. Consistent sorting is a prerequisite for that work.

**Cost:** 15 lines. Prevents future merge friction.

---

### relay 5ee00e19 Force all relay codegen output to be ReadonlyArray

**Author:** Marco Wang  
**Situation:** Codegen output arrays were using mutable `Array` types, which could lead to accidental mutations of generated code at runtime. Flow/TypeScript type safety was compromised.

**Approach:** Changed all codegen output to use `ReadonlyArray` type. This is a type-level change that prevents mutation of generated arrays.

**Mechanism:** Codegen templates updated to emit `ReadonlyArray` instead of `Array`.

**Scale implications:** Generated code is often imported and used without copying. Mutable arrays in generated code could lead to subtle bugs where mutations affect shared state. `ReadonlyArray` enforces immutability at the type level.

**Cost:** Significant — 2,000+ lines of generated code changed. But the change is mechanical (array type annotations), not logic changes.

---

## Cross-Cutting Themes

### 1. Incremental Compilation as a First-Class Concern

Relay treats incremental compilation not as an optimization but as a core requirement. Multiple commits address incremental build correctness:
- Detecting external artifact modifications (`6d10a0ce`)
- Watchman clock synchronization for file events (`fc6be23d`)
- Schema change tracking for mutations (`160bcf5c`)
- Multi-project cross-fragment handling (`520ffc1e`)
- Regression tests for incremental scenarios (`884c06ea`)

The pattern is consistent: build a regression test, fix the bug, ship. The incremental test framework (`relay_compiler_integration_test.rs` with `.input`/`.expected` fixtures) is a significant infrastructure investment.

### 2. Multi-Phase Feature Rollouts

Major features are implemented in phases. `@defer/@stream` support went through three phases across multiple months:
- Phase 1+2: Infrastructure + initial responses (`75207873`)
- Phase 3: Full defer/stream in NormalizationEngine (`f61d5575`)
- S2C execution support (`e4f7cf19`)

RSC support similarly has `serverFetchQuery` → `serverReadFragment` → `serverPreloadQuery` → `useQueryFromServer` as separate commits building a coherent story.

### 3. Safety Through Validation Depth

The team continuously adds deeper validation — not just "is this valid" but "are these independent things both valid and consistent with each other." The `validate_selection_conflict` improvements (stopping at first error → collecting all) is one example. The @match field conflict detection is another.

### 4. Feature Flagging for Migration

Large syntax changes (`@RelayResolver` → `@relayType/@relayField`) are gated behind feature flags. This allows the team to ship new syntax while keeping old syntax working, then remove old syntax in a controlled way. The `allow_legacy_relay_resolver_tag` pattern will likely be repeated.

### 5. Performance Investment in Hot Paths

Transforms that run on every compilation (`flatten`, `client_extensions`) are profiled and optimized. `DashMap` over `RwLock`, `PointerAddress` caching, avoiding recursive `Drop` chains — these are micro-optimizations that compound across millions of compilations.

### 6. Rust + JavaScript Coexistence

Relay is a Rust compiler (for performance) with JavaScript/TypeScript runtime. Commits span both languages fluently. The `NormalizationEngine` is JavaScript; `SchemaSet` is Rust. This is a deliberate architectural choice enabling performance-sensitive compilation in Rust while keeping the React integration in JavaScript.