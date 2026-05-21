# Facebook Relay — Group C Insights (SchemaSet / Rust / Incremental)

---

## 581029c1 — Regression test + fix for mismatched coordinate merging
**Author:** Matt Mahoney | **Date:** 2026-05-17

**Situation:** A rare partial-schema edge case: `interface Inf { field } type Concrete implements Inf` with no original schema, calling `.fix_all_types()`, caused `Concrete.field` to inherit the interface's `SchemaCoordinate` (`Iface.field`). Merging this with a schema that actually defines `Concrete.field` triggered a panic in `merge_coordinate()`.

**Approach:** Fix at the source: make the LHS of the coordinate `None` instead of inheriting the interface's coordinate. This effectively treats the inherited field as `extend field: String` — a non-existent SDL syntax now representable internally.

**Mechanism:** `fix_all_types()` in `set_merges.rs` was propagating coordinate from the interface definition to the concrete type's field. The fix nullifies that coordinate, preventing the merge conflict at the cost of not excluding the field from a fixed schema.

**Scale:** +112/-2 lines in `set_merges.rs`. Targeted fix, no broader API surface change.

**Cost:** Low — isolated bug fix with a regression test.

---

## b0f81918 — Split location-based info from is-present/extension info
**Author:** Matt Mahoney | **Date:** 2026-05-12

**Situation:** `SchemaDefinitionItem` conflated two concerns: source location metadata (`is_client_definition`, descriptions) and business logic about whether a type is an extension. This made the round-trip `(server SDL, client SDL) → SchemaSet → (server SDL, client SDL)` lose the client/server split.

**Approach:** Decouple the concerns. `is_client_definition` stays on the location side (pure metadata about where something is defined). Extension-vs-base is now tracked orthogonally. The change enables perfect round-tripping including after `exclude` operations, which can produce "empty" definitions whose children may belong to base or client schemas.

**Mechanism:** Large refactor across 14 files. The `is_client_definition` field is preserved but now correctly distinguishes "where defined" from "extension status." The `exclude` operation can now produce `extend type Foo { notExcludedField }` without forcing it to be treated as a client definition.

**Scale:** 836 insertions, 562 deletions across 14 files. A foundational refactor.

**Cost:** High — changes core data structures. Requires careful round-trip testing.

---

## 22872a5f — Add incremental compilation test: subscription root type rename
**Author:** Evan Yeung | **Date:** 2026-05-08

**Situation:** No integration test existed to verify that renaming a subscription root type is handled correctly during incremental compilation (vs. requiring a full rebuild).

**Approach:** Added a fuzz-style integration test with input and expected output files (`incremental_fuzz_subscription_root_type_change.input/expected`). Tests the relay compiler integration framework end-to-end.

**Mechanism:** Pattern: mutate input files → run incremental build → compare against expected artifacts. 289 lines of test data + 1 line of test registration.

**Scale:** 289 insertions in test fixtures + 9-line test update. Test infrastructure only.

**Cost:** Zero production impact — pure test coverage.

---

## 6d10a0ce — Detect external modifications to generated artifacts
**Author:** Evan Yeung | **Date:** 2026-05-08

**Situation:** If an external tool modified generated artifacts on disk, the compiler's in-memory artifact cache had no way to know — leading to stale output or crashes when the compiler tried to write back.

**Approach:** Track artifact state in `compiler_state.rs` and validate on build. If external modification is detected, mark the artifact as dirty and recompute.

**Mechanism:** 3 files changed. `artifact_writer.rs` gets a check before writing. `compiler_state.rs` tracks artifact metadata. `compiler.rs` propagates the dirty signal.

**Scale:** ~37 lines net. Small, targeted change.

**Cost:** Low — defensive fix for an edge case.

---

## 69021d2b — Surface compiler crashes to meerkat clients
**Author:** Evan Yeung | **Date:** 2026-05-08

**Situation:** When the compiler crashed, there was no way to surface the error to clients using the meerkat protocol (watch mode).

**Approach:** Emit crash information through the `status_reporter.rs` channel so watch-mode clients can display meaningful error messages instead of silent failures.

**Mechanism:** +12 lines in `status_reporter.rs`. Crash info serialized and sent to subscribed clients.

**Scale:** Minimal — 12 lines, single file. Interface change only.

**Cost:** Very low.

---

## fc6be23d — Use watchman clock to synchronize file subscription and write events
**Author:** Evan Yeung | **Date:** 2026-05-08

**Situation:** File subscription events and file write events from the compiler could race, causing the compiler to miss updates or process stale state. The clock synchronization between watchman and the compiler was insufficient.

**Approach:** Use the watchman clock value to order and synchronize events. Events are only processed after the clock confirms they are the latest.

**Mechanism:** 4 files, 262 insertions. `status_reporter.rs` grows a clock-synchronized event queue. `watchman_file_source.rs` implements clock-aware subscription. `compiler.rs` uses the synchronized events.

**Scale:** Large integration change. +262/-5 lines.

**Cost:** Medium — touches watch mode, requires watchman support.

---

## 520ffc1e — Fix incremental bug: multi-project cross-fragment crash in CommonJS mode
**Author:** Evan Yeung | **Date:** 2026-04-30

**Situation:** Incremental builds crashed when a fragment referenced fields across multiple projects in CommonJS mode. The bug only surfaced in incremental mode, not in full builds.

**Approach:** Added integration test reproducing the crash, then fixed `build_project.rs` to correctly handle cross-project field resolution during incremental builds in CommonJS.

**Mechanism:** +356/-2 lines. Fuzz test fixture captures the multi-project cross-fragment mutation. The fix ensures fragment artifacts are recomputed when referenced fields change across project boundaries.

**Scale:** Test: 338 lines fixture + production fix (~11 lines).

**Cost:** Low — targeted fix with comprehensive test.

---

## cf992d7b — Add unit tests to schema_set_collector.rs
**Author:** Matt Mahoney | **Date:** 2026-04-29

**Situation:** `schema_set_collector.rs` lacked unit test coverage despite accumulating significant logic.

**Approach:** Add +114 lines of unit tests directly in the module.

**Mechanism:** Tests cover the collector's core logic: document ingestion, error aggregation, deduplication.

**Scale:** 114 lines, single file. Pure coverage.

**Cost:** Zero production impact.

---

## adee3be6 — Port merge_schemas test and add unit tests to schema_set.rs
**Author:** Matt Mahoney | **Date:** 2026-04-29

**Situation:** `schema_set.rs` lacked unit tests. Internal tests existed but hadn't been ported to the open-source crate.

**Approach:** Port the internal `merge_schemas` test and add comprehensive unit tests covering `SchemaSet`'s public API.

**Mechanism:** +525 lines of tests in `schema_set.rs`. Tests cover schema merging, partitioning, and round-tripping.

**Scale:** 525 lines, single file. Significant test coverage addition.

**Cost:** Zero production impact.

---

## 13faa926 — Add complex partitioning test
**Author:** Matt Mahoney | **Date:** 2026-04-29

**Situation:** Internal tests for the `partition_base_extensions` feature weren't present in the open-source version, leaving the most complex scenarios untested.

**Approach:** Port the internal complex partitioning tests to `partition_base_extensions.rs`.

**Mechanism:** +242/-5 lines. Tests cover edge cases: nested type partitioning, extension-only types, directive partitioning.

**Scale:** 247 lines total. Comprehensive coverage for partitioning logic.

**Cost:** Zero production impact.

---

## 4662fd98 — Fix SchemaSet partitioning enum values + more
**Author:** Matt Mahoney | **Date:** 2026-04-29

**Situation:** Two bugs: (1) If an enum was ever extended, ALL its values ended up in the extension partition instead of just the added values — causing duplicate output in `graphql_schema` migrations. (2) Input object fields were never considered extensions, creating a silent correctness issue.

**Approach:** Fix the partitioning logic to correctly distinguish base enum values from extension-added values, using the same `SetEnumValue`/`SetArgumentValue` approach. Also audit `SetInputObject` creation to properly track field extension status.

**Mechanism:** +483/-45 lines across 6 files. Core logic in `partition_base_extensions.rs` and `schema_set_collector.rs`. Bug (1) affected flatbuffer schema generation; bug (2) was a latent issue discovered during the fix.

**Scale:** 528 lines net. Multi-file fix.

**Cost:** Medium — fixes a data correctness bug that would corrupt schema migrations.

---

## 4ec3088a — Fix SchemaSet::exclude to preserve type definitions when other is an extension
**Author:** Curtis Li | **Date:** 2026-04-17

**Situation:** `SchemaSet::exclude` incorrectly handled the case where the "other" schema is an extension. Type definitions were being dropped when they should be preserved, causing type loss in client schema generation.

**Approach:** Fix the exclusion logic to check whether the "other" is an extension vs. a base definition, and preserve base definitions in that case. Added comprehensive test coverage.

**Mechanism:** +153/-6 lines in `set_exclude.rs`. The fix handles the `is_extension` flag during subtraction; previously this flag was ignored in some code paths.

**Scale:** 159 lines, single file. Deep fix.

**Cost:** Low — targeted fix.

---

## 58744e67 — Update schema set merging to return errors instead of panicking
**Author:** Janette Cheng | **Date:** 2026-04-10

**Situation:** Schema set merging used `panic!` for conflicting definitions. This is unacceptable for library use where callers need to handle errors gracefully.

**Approach:** Replace all panic sites with `Result` return types. Add `DiagnosticsResult` as the standard return type across the merging API.

**Mechanism:** +355/-120 lines across 5 files. `set_merges.rs` is the primary target, gaining a comprehensive error handling overhaul. `merge_sdl_document.rs` and `schema_set.rs` updated to propagate errors.

**Scale:** 475 lines net. Large error-handling refactor.

**Cost:** Medium — changes public API return types. Callers updated in the same commit.

---

## 3ddc095a — Update schema_set public functions to return DiagnosticsResult
**Author:** Janette Cheng | **Date:** 2026-04-10

**Situation:** Some `schema_set` public functions still returned `()` instead of `DiagnosticsResult`, creating an inconsistent API.

**Approach:** Audit all public functions and update signatures to return `DiagnosticsResult`. Propagate errors through call sites.

**Mechanism:** 12 files changed. Functions in `schema_set.rs`, `build_schema_document.rs`, `merge_sdl_document.rs`, etc. updated. Callers in the relay-compiler updated to handle errors.

**Scale:** 77 insertions, 40 deletions.

**Cost:** Medium — consistent API, but changes call sites.

---

## 884c06ea — Add incremental compilation test for schema field nullability change
**Author:** Jordan Eldredge | **Date:** 2026-04-06

**Situation:** No test verified that changing a field's nullability (e.g., `String!` → `String`) correctly updates generated artifacts during incremental compilation.

**Approach:** Added a fuzz-style integration test with input/expected fixtures covering nullability changes.

**Mechanism:** +139/-1 lines. Test harness confirms incremental output matches full-build output.

**Scale:** 139 lines (test fixtures + registration).

**Cost:** Zero production impact.

---

## 86dc40df — Fix incremental build missing field return type changes
**Author:** Tianyu Yao | **Date:** 2026-04-03

**Situation:** Incremental builds didn't detect when a field's return type changed — field artifacts weren't invalidated, leading to stale generated code that could cause type errors at runtime.

**Approach:** Add schema diff checking for field return type changes. When a return type changes, invalidate and recompute all artifacts that transitively depend on that field.

**Mechanism:** 9 files, +437 lines. `schema-diff/src/check.rs` gains return-type diff logic. `relay_compiler_integration_test.rs` gets integration tests. Test fixtures cover query/mutation field return type changes.

**Scale:** Large fix. ~440 lines across schema-diff and compiler.

**Cost:** Medium — correctness fix, touches dependency analysis.

---

## e6b450f7 — Round-trip extension-only types as `extend X { ... }` instead of `X { ... }`
**Author:** Matt Mahoney | **Date:** 2026-05-05

**Situation:** `SchemaSet::to_sdl_definition()` was lossy for extension-only types: `extend interface Foo { ... }` parsed and re-serialized came back as `interface Foo { ... }` (fresh definition, not an extension). Downstream code pairing this with the base schema would then trip `DuplicateType` errors. Internal `graphql_build_infra` had a workaround bypassing `to_sdl_definition`.

**Approach:** The fix distinguishes extensions from definitions during serialization by using the presence/absence of `definition: Option<SchemaDefinitionItem>` as the extension signal. Added `is_extends` parameter to `to_set_definition`, `ClearTopLevelDefinition` trait, and `to_sdl_extension` inherent impls for each top-level `Set*` variant.

**Mechanism:** 11 files, +773/-254 lines. Key insight: `merge_ext_into` passes `is_extends = true` so the resulting entry has `definition: None`, which `to_sdl_definition` then reads as "this is an extension." `partition_base_extensions` also fixed to clear `definition` on the extension half.

**Scale:** Large refactor. +773/-254 lines. Core schema-set serialization change.

**Cost:** High — fundamental change to how extension-vs-definition is tracked. Note the "hack" caveat: using absence of `SchemaDefinitionItem` as the extension signal is fragile; a dedicated `is_extends` field would be cleaner in a future refactor.

---

## 6e8c7da8 — Clarify how to construct SchemaSet from base + extension SDL sources
**Author:** Matt Mahoney | **Date:** 2026-05-05

**Situation:** `SchemaSet::from_schema_documents` was ambiguous: callers had to understand internal flag plumbing to correctly specify whether input was base or extension definitions. This was error-prone and leaked internal details.

**Approach:** Replace the ambiguous single entry point with two clear ones: `from_base_schema_documents(docs)` for base-only, and `from_schema_documents_with_extensions(base, extensions)` for pre-partitioned documents.

**Mechanism:** 12 files, +260/-18 lines. New public API. Internal test helpers renamed. Double-mutable-borrow bug fixed in `from_schema_documents_with_extensions` by pre-chaining iterators.

**Scale:** 278 lines net. API redesign.

**Cost:** Low — better API, fixes a borrow issue, no behavior change.

---

## ccd7f51e — Add default schema definition for scalars in BUILTIN_SCALAR_SET
**Author:** Curtis Li | **Date:** 2026-04-17

**Situation:** Built-in scalars (e.g., `String`, `Int`) in `BUILTIN_SCALAR_SET` lacked a default schema definition, causing inconsistent behavior when used in certain schema operations.

**Approach:** Add a default schema definition to the scalars in `builtin_scalars.rs`.

**Mechanism:** +2/-1 lines. Minimal targeted change.

**Scale:** 2 lines. Trivial.

**Cost:** Very low.

---

## ea3984e5 — Fix exclude_schema to also remove fields from client extension types
**Author:** Curtis Li | **Date:** 2026-04-13

**Situation:** `exclude_schema` only removed fields from base types, not from client extension types. This meant fields added by client extensions could "leak" through when they should have been excluded.

**Approach:** Extend the exclusion logic to traverse and remove fields from client extension types as well as base types.

**Mechanism:** +189/-2 lines in `set_remove_defined_references.rs`. The fix ensures exclusion is applied recursively to client extension type fields.

**Scale:** 191 lines, single file.

**Cost:** Low — targeted fix with good coverage.

---

## 9f298caa — Remove Node interface restriction for mixed interface server types
**Author:** Jordan Eldredge | **Date:** 2026-04-03

**Situation:** Mixed-interface server types (types implementing interfaces where some implementations are server-only and some are client-only) had an unnecessary restriction requiring them to implement `Node`. This was overly restrictive and caused valid schemas to be rejected.

**Approach:** Remove the `Node` interface requirement for mixed-interface server types. Update `client_edges.rs` and related transform code to handle this case.

**Mechanism:** 12 files, +459/-66 lines. `client_edges.rs` gets major logic update to support mixed interfaces without requiring `Node`. Error messages updated. Test fixtures updated.

**Scale:** Large. 525 lines net. Significant expansion of valid schema space.

**Cost:** Medium — expands what schemas are accepted as valid. Could affect codegen output for previously-rejected schemas now being accepted.

---

## e3c9d1b5 — Clear is_building in incremental build loop when no pending changes
**Author:** Evan Yeung | **Date:** 2026-04-17

**Situation:** In the incremental build loop, `is_building` wasn't cleared when there were no pending file changes. This caused the compiler to remain in "building" state and potentially skip the next round of changes.

**Approach:** Clear `is_building` flag at the start of the loop iteration when the pending change queue is empty.

**Mechanism:** +8 lines in `compiler.rs`. Single conditional.

**Scale:** 8 lines. Minimal.

**Cost:** Very low.

---

## c529ecba — Clear daemon artifact cache on watch loop restart after rebase
**Author:** Evan Yeung | **Date:** 2026-04-16

**Situation:** When the watch loop restarted after a rebase, stale artifacts from the previous loop remained in the cache, causing incorrect incremental builds.

**Approach:** Clear the daemon artifact cache when the watch loop restarts.

**Mechanism:** +11 lines across 2 files. `artifact_writer.rs` clears cache on restart. `compiler.rs` triggers the clear.

**Scale:** 11 lines. Minimal.

**Cost:** Very low.

---

## cf9a9baf — Add source control update simulation to daemon integration test infrastructure
**Author:** Evan Yeung | **Date:** 2026-04-14

**Situation:** The daemon integration test infrastructure lacked a way to simulate source control updates (file changes, deletions) for testing watch mode scenarios.

**Approach:** Add simulation infrastructure to `file_source.rs`, `compiler.rs`, and `config.rs` that can simulate source control events in tests without actual file system changes.

**Mechanism:** 5 files, +98/-23 lines. `config.rs` gets simulation config. `file_source.rs` gets simulation event emission. Used to test rebase/restart scenarios.

**Scale:** 121 lines. Test infrastructure only.

**Cost:** Zero production impact.

---

**Last processed SHA:** 581029c1fc29ba276535e565d2bb36fba445c5be (first), cf9a9baf68cd63554bece169b54d397062f0ae45 (last)