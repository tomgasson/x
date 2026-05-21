# Relay Group D — Directives, Validation, Mutations

## 4613c9d1 — Support @catch directive on client edge fields
**Author:** Xin Chen  
**Files:** 18 | **Lines:** +1885

**Situation:** `throwOnFieldError` on `relay_everywhere` fragments causes UI sections to vanish when client edge data is transiently unavailable (initial load, campaign switching, Relay store GC). The natural fix — `catch(to: NULL)` — was blocked by the Relay compiler with "Unexpected directive on Client Edge field." 96 ads editor files were affected.

**Approach:** Unblock `catch` on client edges. The restriction was an allowlist omission in `client_edges.rs`, not a fundamental incompatibility.

**Mechanism:**
- Added `CATCH_DIRECTIVE_NAME` to the client edge directive allowlist in `client_edges.rs`
- Hoists `CatchMetadataDirective` from the field onto the wrapping inline fragment (same pattern as `required`)
- Added `catch` to the resolver field directive filter in `field_transform.rs` so resolver-backed fields accept it
- Added 4 RelayReader tests + compiler fixture tests for both `ClientEdgeToServerObject` and `ClientEdgeToClientObject`

**Scale implications:** Unblocks a significant pattern for ads editor resilience. Graceful degradation via `catch(to: NULL)` means sections stop disappearing on transient missing data.

**Cost:** High — 1885 lines of new test code, fixtures, and Flow/TS snapshot updates across reader and compiler.

---

## 3e9007ef — Remove @fb_actor_change directive and related infrastructure
**Author:** Gary Zeng  
**Files:** 23 | **Lines:** +4 / -502

**Situation:** The `@fb_actor_change` directive was a Meta-specific feature no longer needed.

**Approach:** Full removal across the codebase — feature flag, AST building, transforms, typegen, visitor logic, and test fixtures.

**Mechanism:**
- Removed from feature flags, `build_ast.rs`, `constants.rs`, `relay-transforms/lib.rs`, `errors.rs`, `relay_actor_change.rs` (205 lines), `apply_transforms.rs`, typegen (flow/typescript/visit/write/writer), and config schemas
- 23 files touched, net -498 lines

**Scale implications:** Cleanup of Meta-specific infrastructure. Leaves behind a smaller, more generically useful Relay.

**Cost:** Moderate LOC but broad blast radius across compiler layers.

---

## ae3b14b5 — Merge directive arguments in schema-set when existing directive has none
**Author:** Curtis Li  
**Files:** 1 | **Lines:** +50 / -5

**Situation:** A schema-set merge edge case: when merging a directive that has arguments into an existing directive that has no arguments at all, the merge logic failed to combine them correctly.

**Approach:** Fix the merge logic in `set_merges.rs`.

**Mechanism:** Added logic to handle the case where the base directive has no arguments but the incoming directive does — now merges the argument lists correctly.

**Scale implications:** Fixes a schema composition correctness bug in the subset validation pipeline.

**Cost:** Small but targeted — 50 lines in one file.

---

## f7d1c0f5 — Fix raw response with selections on abstract type (#4775)
**Author:** tobias-tengler  
**Files:** 52 | **Lines:** +1729 / -31

**Situation:** Previously, all selections on abstract types were generated as required properties in the raw response type. This is incorrect — a concrete type may not implement every abstract selection. The generated Flow/TypeScript types were too strict.

**Approach:** Track selections on abstract types separately and only make them required when the concrete type actually implements the abstract type. Add a feature flag for gradual rollout.

**Mechanism:**
- Added `disable_more_precise_abstract_selection_raw_response_type` feature flag
- Changed `type_selection.rs` and `visit.rs` to track abstract selections separately
- Updated 52 files of fixtures, Flow/TS snapshot tests, and config schemas
- The diff on generated types for WWW validated by claude shows strictly more accurate types

**Scale implications:** Affects all consumers of raw response types when using abstract types (interfaces, unions). Type accuracy significantly improved.

**Cost:** Very high — 1698 net new lines, extensive snapshot updates, 52 files.

---

## c356dc12 — Fix bug with conflicting @match fields
**Author:** Evan Yeung  
**Files:** 12 | **Lines:** +719 / -1

**Situation:** Validation failed to detect @match field conflicts across fragments when fragments were independent (not spread into each other). This allowed invalid multi-module artifact generation.

**Approach:** Extend the conflict detection algorithm to cover independent fragment spreads.

**Mechanism:**
- Added 6 new fixture files (`*match-conflicts*.invalid.graphql` / `.expected`) covering various @match conflict scenarios
- Updated `validate_selection_conflict.rs` with 69 new lines of detection logic
- Updated test harness

**Scale implications:** Critical correctness fix for the @match / multi-module feature. Prevents silent miscompilation.

**Cost:** 719 lines, dominated by new fixture files for edge cases.

---

## ee2d2354 — Report all fragment validation errors instead of stopping at first
**Author:** Evan Yeung  
**Files:** 7 | **Lines:** +321 / -11

**Situation:** Fragment validation stopped at the first error. Developers had to fix one error, recompile, see the next — a slow iteration loop.

**Approach:** Collect all validation errors and report them together.

**Mechanism:**
- Changed error accumulation in `validate_selection_conflict.rs` from early-return to collect-then-report
- Added two new fixture files for cross-fragment error scenarios (`*across-independent-fragments*.invalid`)
- Updated test harness to expect multiple errors

**Scale implications:** Developer experience improvement — fewer compilation iterations.

**Cost:** 321 new lines, mostly fixture cases for the new multi-error scenarios.

---

## 561cf15a — Fix stack overflow in RcDoc Drop for large single-type definitions
**Author:** Mathew Luo  
**Files:** 3 | **Lines:** +120 / -18

**Situation:** Deeply recursive `RcDoc::Drop` implementation caused stack overflow on large schema type definitions. Large single-type definitions (with many fields) were the trigger.

**Approach:** Rework the `RcDoc` drop logic to be iterative instead of recursive.

**Mechanism:**
- Rewrote `prettier_doc_builders.rs` Drop implementation (110 new lines)
- Minor adjustments in `prettier_executable_printer.rs` and `prettier_schema_printer.rs`
- Switched from recursive traversal to an iterative approach

**Scale implications:** Critical stability fix. Large schema definitions no longer cause compiler crashes.

**Cost:** Moderate — 120 lines, but fixes a real crash vector.

---

## 3fd69ac9 — Add 1MB stack overflow regression tests for balanced_intersperse
**Author:** Mathew Luo  
**Files:** 2 | **Lines:** +68

**Situation:** The prior stack overflow fix (561cf15a) needed regression tests to prevent future recurrence.

**Approach:** Add tests that exercise `balanced_intersperse` with 1MB-level inputs.

**Mechanism:**
- Added 30-line test to `prettier_doc_builders.rs`
- Added 38-line test to `prettier_schema_printer.rs`
- Both tests exercise the recursive path that previously overflowed

**Scale implications:** Defensive — ensures the overflow fix stays fixed under future refactoring.

**Cost:** Small — 68 lines of test code.

---

## 89f355f0 — Update hashbrown from 0.16.1 to 0.17.0
**Author:** David Tolnay  
**Files:** 1 | **Lines:** +1 / -1

**Situation:** Outdated hashbrown dependency.

**Approach:** Trivial version bump in `Cargo.toml`.

**Scale implications:** Keeps compiler on a recent hashbrown release. No semantic change.

**Cost:** Negligible.

---

## a093237c — Update hashbrown in first-party code from 0.14 to 0.16
**Author:** David Tolnay  
**Files:** 1 | **Lines:** +1 / -1

**Situation:** Two minor versions behind on hashbrown in the `intern` crate.

**Approach:** Version bump in `intern/Cargo.toml`.

**Scale implications:** Dependency modernization. Two-version jump.

**Cost:** Negligible.

---

## b32e1155 — Include full directive definition in subset violation error message
**Author:** Curtis Li  
**Files:** 2 | **Lines:** +22 / -3

**Situation:** Subset violation error messages for directives showed only the directive name, not the full definition (arguments, locations). Developers couldn't diagnose violations without looking up the schema.

**Approach:** Include the full directive definition string in the error message.

**Mechanism:**
- Updated `find_subset_violations.rs` to render full directive definition
- Updated `print_schema_set.rs` to support rendering

**Scale implications:** Significantly improved debuggability for schema composition errors.

**Cost:** Small — 22 lines.

---

## a3587dd0 — Use CanHaveDirectives trait in walk_type_directive_violations
**Author:** Curtis Li  
**Files:** 1 | **Lines:** +3 / -10

**Situation:** `walk_type_directive_violations` had hand-rolled directive checking code instead of reusing the `CanHaveDirectives` trait.

**Approach:** Refactor to use the shared trait, reducing duplication.

**Mechanism:** Replaced custom type checks with `CanHaveDirectives::can_have_directives()` call. Net -7 lines.

**Scale implications:** Reduced duplication in the validation subsystem.

**Cost:** Negligible refactor.

---

## 40825fcd — Add CLI arg, comments, and tests for subset_directives in find_subset_violations
**Author:** Curtis Li  
**Files:** 1 | **Lines:** +92

**Situation:** The `subset_directives` feature in `find_subset_violations` lacked CLI exposure, documentation comments, and test coverage.

**Approach:** Fill in the gaps.

**Mechanism:**
- Added CLI argument for `subset_directives`
- Wrote comments explaining the feature
- Added comprehensive tests
- 92 lines of new code

**Scale implications:** Completed a feature that was otherwise only usable via programmatic API.

**Cost:** 92 lines.

---

## 226a8fae — Refactor exclude_directives to accept SafeExclusionOptions
**Author:** Curtis Li  
**Files:** 1 | **Lines:** +15 / -55

**Situation:** `exclude_directives` took raw parameters, but a richer `SafeExclusionOptions` struct was becoming the preferred abstraction.

**Approach:** Refactor to accept `SafeExclusionOptions` instead.

**Mechanism:** Updated signature and internals. Net -40 lines due to consolidating duplicate logic.

**Scale implications:** Cleaner abstraction, easier to extend exclusion options in future.

**Cost:** Refactor, net reduction in LOC.

---

## 8818b24c — Add field-level directive validation and rename InconsistentTypeDirectiveUse
**Author:** Curtis Li  
**Files:** 1 | **Lines:** +130 / -3

**Situation:** The schema validation was missing field-level directive checks. `InconsistentTypeDirectiveUse` error name was also imprecise.

**Approach:** Add field-level validation and rename the error type.

**Mechanism:**
- Added field-level directive validation to `find_subset_violations.rs` (130 new lines)
- Renamed `InconsistentTypeDirectiveUse` to something more precise
- 3 lines of deletion (renames)

**Scale implications:** Schema validation now catches directive misplacements at field level, not just type level.

**Cost:** 130 lines of new validation logic.

---

## 55eb93d4 — Add base_restricted_directives to SafeExclusionOptions
**Author:** Curtis Li  
**Files:** 1 | **Lines:** +140 / -32

**Situation:** `SafeExclusionOptions` didn't account for base restricted directives — directives that exist in the base schema and shouldn't be excluded even if they appear in a subset.

**Approach:** Extend `SafeExclusionOptions` with `base_restricted_directives` field.

**Mechanism:**
- Added the field to `SafeExclusionOptions` in `set_exclude.rs`
- Updated merge/exclusion logic to respect base restricted directives
- 140 lines of new logic

**Scale implications:** Enables more nuanced schema subset composition where some directives are special-cased.

**Cost:** 140 lines, single file.

---

## b1bc085c — Add base_restricted_directives to find_subset_violations
**Author:** Curtis Li  
**Files:** 4 | **Lines:** +262 / -11

**Situation:** `find_subset_violations` wasn't checking against base restricted directives — so it could incorrectly flag valid directive usage as violations.

**Approach:** Integrate `base_restricted_directives` into the violation-finding logic.

**Mechanism:**
- Updated `find_subset_violations.rs` (+254 lines)
- Updated `builtin_scalars.rs`, `schema_set.rs`, `set_exclude.rs`
- `base_restricted_directives` now influence which violations are reported

**Scale implications:** Correctness fix for schema subset validation — prevents false positives on restricted directives.

**Cost:** 262 lines across 4 files.

---

## af82e6fe — Add useMutationAction_EXPERIMENTAL
**Author:** Jack Pope  
**Files:** 23 | **Lines:** +1674

**Situation:** Relay's `useMutation` uses callback-based error handling (`onCompleted`, `onError`). React's async action APIs (`startTransition`, `useTransition`, form actions) need promise-based semantics. The two models didn't compose cleanly.

**Approach:** Create a new hook `useMutationAction` that wraps `useMutation` and maps callback semantics to promises.

**Mechanism:**
- New `useMutationAction_EXPERIMENTAL` hook wrapping `useMutation`
- `onCompleted(response, errors)` → always resolves (even field-level errors)
- `onError(error)` → rejects
- Promise resolves directly with `TData` (not wrapped in tuple)
- `isPending` comes from caller's `useTransition()` — hook returns only the commit function
- Extensive documentation: field errors, form actions, optimistic updates, sequential mutations, error boundaries
- 23 files: hook implementation, tests, snapshot files, documentation

**Scale implications:** Major new React 18+ compatibility layer. Makes Relay mutations work with `startTransition`, form actions, and `useOptimistic`.

**Cost:** Large — 1674 lines across 23 files including extensive test fixtures and documentation.

---

## e7e43953 — Remove more @live_query tests in GraphQL codebase
**Author:** Xiangxin Sun  
**Files:** 3 | **Lines:** +1 / -48

**Situation:** Ongoing `@live_query` removal — dead tests for deprecated features were still present.

**Approach:** Delete remaining test fixtures and simplify test code.

**Mechanism:**
- Removed `live_by_config_id.expected` and `.graphql` fixtures
- Simplified `generate_live_query_metadata_test.rs`

**Scale implications:** Cleanup of deprecated feature tests.

**Cost:** Negligible — cleanup.

---

## 262e29c7 — Deprecate polling_interval on @live_query in Relay compiler
**Author:** Xiangxin Sun  
**Files:** 6 | **Lines:** +42 / -129

**Situation:** `polling_interval` on `@live_query` was being deprecated as a feature. The compiler needed to reflect this deprecation.

**Approach:** Remove the polling interval generation logic from the compiler and update affected test fixtures.

**Mechanism:**
- Removed `live_by_polling_interval.expected` and `.graphql` fixtures (28+13 lines)
- Simplified `generate_live_query_metadata.rs` (-104 lines)
- Updated error/fixture files to remove polling-related expectations
- 6 files touched

**Scale implications:** Deprecation signal. The feature still exists but is no longer being actively supported.

**Cost:** Net reduction of 87 lines.

---

## b2dd4372 — Remove @live_query directive support from Relay compiler
**Author:** Xiangxin Sun  
**Files:** 1 | **Lines:** +6 / -50

**Situation:** `@live_query` directive was being fully removed from the Relay compiler.

**Approach:** Strip out the directive metadata generation from `generate_live_query_metadata.rs`.

**Mechanism:**
- Removed 50 lines of `@live_query` directive support from `generate_live_query_metadata.rs`
- Only the file-stripping remained (6 lines)

**Scale implications:** Complete removal of `@live_query` directive support from the compiler.

**Cost:** 50 lines removed.

---

## a452183f — Fix RequiredDirectiveArgAdded description message in find_subset_violations
**Author:** Mathew Luo  
**Files:** 1 | **Lines:** +1 / -1

**Situation:** The description for `RequiredDirectiveArgAdded` incorrectly said "is not defined in base schema" when the arg actually IS defined in base and just missing from the subset. Misleading error message.

**Approach:** Fix the message to match the pattern used by `RequiredArgAdded` and `RequiredInputFieldAdded`.

**Mechanism:** One-line fix to the error description string.

**Scale implications:** Developer experience — error messages now accurately describe the problem.

**Cost:** Negligible.
