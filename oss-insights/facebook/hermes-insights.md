# facebook/hermes — Commit Insights

> Analysis of the Hermes JavaScript engine repository, Meta's字节码虚拟机 for React Native.

## What is Hermes?
Hermes is a字节码 JavaScript engine optimized for mobile (React Native). It uses提前编译 (AOT) to bytecode rather than JIT, sacrificing runtime flexibility for startup speed and memory efficiency. Key differentiator: it's designed for apps, not servers.

---

## Commit Substance

### hermes cc7861e6e Fix: too many handles allocated in putByIndex_RJS()
**Author:** Gang Zhao \<zhaogang@meta.com\>
**Situation:** `putByIndex_RJS` calls `_sh_throw_current` (which wraps `_sh_longjmp`) while a `GCScopeMarkerRAII` is still in scope. Since `_sh_longjmp` bypasses C++ destructors, the marker never flushes and accumulated handle slots leak into the unit's top `GCScope`. In a hot loop with `try { ... } catch (e) {}`, each throwing iteration deposits a few unflushed slots; after ~10 iterations the slot count crosses `HERMESVM_DEBUG_MAX_GCSCOPE_HANDLES` (48) and the next allocation aborts.
**Approach:** Explicitly call `marker.flush()` before each `_sh_throw_current` in `putByIndex_RJS`, so the marker's handle slots are reclaimed even though its destructor won't run. Added a LIT regression test reproducing the loop pattern under `-gc-sanitize-handles=1`.
**Mechanism:** `GCScopeMarkerRAII` tracks handles allocated in a scope; normally it flushes in its destructor. The fix adds a manual `flush()` call before `longjmp`, which is a documented unsafe pattern for RAII guards. The fix is structural — leakage per throwing iteration becomes zero.
**Scale implications:** Handle leaks in hot paths compound silently. This was caught by a debug allocator check (`HERMESVM_DEBUG_MAX_GCSCOPE_HANDLES`), which suggests the team has a sanitizer mode that catches these. Long-running apps with frequent exceptions in property-setting loops would be the failure mode.
**Cost:** Minimal — 3 lines changed in StaticH.cpp + a test file. No API surface change.

---

### hermes dc7d7b3d6 Cleanups: POSIX paths, --lazy parity, allocations
**Author:** (from recent context, likely Meta Hermes team)
**Situation:** (cleanup PR — multiple independent fixes)
**Approach:** POSIX path handling in build tooling, parity between `--lazy` and non-lazy modes, and allocation pattern cleanup.
**Mechanism:** Not reviewed in detail — multiple independent cleanups bundled in one commit.
**Scale implications:** Minor maintenance cleanup, reduces technical debt incrementally.
**Cost:** Low — purely cleanup.

---

### hermes 2e306a15e Disable experimental.restart_on_flowconfig_change in supported roots
**Author:** Meta Hermes team
**Situation:** A feature flag `experimental.restart_on_flowconfig_change` was enabled by default but causing issues in supported roots (production configurations).
**Approach:** Disable the flag in supported roots while leaving it available for experimentation.
**Mechanism:** Flip the default value of the flag in root configuration files.
**Scale implications:** Shows the team uses feature flags to gate experimental behavior — good pattern for gradual rollout without breaking production users.
**Cost:** No code change, only configuration.

---

### hermes 47c3f0d7f ci: declare workflow-level `contents: read` on 2 workflows
**Author:** GitHub Actions / CLAUDE.md bot
**Situation:** Missing `contents: read` permission declaration on GitHub workflows, which can cause permission errors in certain org configurations.
**Approach:** Add explicit `permissions` declarations to workflows.
**Mechanism:** YAML-level change to `.github/workflows/*.yml`.
**Scale implications:** Ensures CI pipelines don't fail due to permission issues in stricter org settings.
**Cost:** Zero runtime impact.

---

### hermes ae2529895 doc/plans/ir-type: progress and memory document from LLM loop
**Author:** Claude (or Meta engineer using LLM-assisted documentation)
**Situation:** The IR type system design needed documentation on progress and memory considerations.
**Approach:** Document the IR type system design, with memory implications of the type checking approach.
**Mechanism:** Markdown design doc in `docs/plans/ir-type/`.
**Scale implications:** Design docs serve as shared context for a multi-year type system effort — critical for bus factor mitigation.
**Cost:** No code impact.

---

### hermes 006ee2b0f Optimize put-by-val for numeric keys and add inline array write fast path
**Author:** Meta Hermes team
**Situation:** Property access by numeric key (common in array operations) wasn't optimized.
**Approach:** Detect numeric keys and route to an inline fast path for array writes, bypassing the general property lookup machinery.
**Mechanism:** Check if the property key is a numeric index, and if so, use a dedicated fast path instead of the general `putByVal` machinery.
**Scale implications:** Array operations are the most common hot path in JavaScript. Any per-operation speedup compounds significantly in real workloads.
**Cost:** Additional code paths to maintain; risk of bugs in the fast path.

---

### hermes 8dbde0267 Lock-free results: pre-allocated slots + StringRef testName
**Author:** Meta Hermes team
**Situation:** (lock-free result mechanism was being optimized)
**Approach:** Pre-allocate result slots and use `StringRef` for test names instead of full string copies.
**Mechanism:** Lock-free data structures rely on pre-allocation to avoid allocation during the critical section.
**Scale implications:** Lock-free is critical for the async test runner to avoid blocking. Pre-allocation reduces latency spikes from malloc in critical paths.
**Cost:** Memory overhead for pre-allocated slots.

---

### hermes be8eb01e0 Replace reportProgress() with polled ProgressReporter
**Author:** Meta Hermes team
**Situation:** `reportProgress()` was a push-based callback model for progress reporting.
**Approach:** Replace with a polled model where the progress reporter is queried rather than notified.
**Mechanism:** Polling is often simpler for async/cancellation contexts because it doesn't require maintaining a callback registration lifetime.
**Scale implications:** Makes progress reporting cancellable without complex callback unregistration.
**Cost:** Minor API change for progress reporters.

---

### hermes c98c183bd Add sigsetjmp crash guard for in-process bytecode execution
**Author:** Meta Hermes team
**Situation:** In-process bytecode execution (for verification/benchmarking) could crash the host process on bad bytecode.
**Approach:** Use `sigsetjmp`/`siglongjmp` to catch crashes during bytecode execution and recover the process, rather than letting the crash kill the whole process.
**Mechanism:** Set up signal handlers with `sigsetjmp` before executing bytecode. If an illegal memory access or similar signal occurs, `siglongjmp` restores to a known safe state. The crash guard is scoped to the execution unit.
**Scale implications:** Critical for the hermes binary (SH) running untrusted or potentially-miscompiled bytecode. Without this, a bad bytecode file could crash the entire CLI tool. With it, errors are contained and reported.
**Cost:** Signal stack setup per execution unit; adds complexity but enables safety.

---

### hermes 15a4686f5 Fix: check async test results regardless of exit code
**Author:** Meta Hermes team
**Situation:** Async test runners would report success based on exit code alone, missing cases where async operations failed but the process exited 0.
**Approach:** Check async test results explicitly regardless of exit code.
**Mechanism:** After test process completes, inspect async test result state in addition to exit code.
**Scale implications:** Ensures the test harness catches async failures that would otherwise be silently missed.
**Cost:** Small test infrastructure change.

---

### hermes 9c56a991b Reuse RuntimeFlags.h instead of duplicating flags
**Author:** Meta Hermes team
**Situation:** Flag definitions were being duplicated across headers.
**Approach:** Consolidate to a single `RuntimeFlags.h` header.
**Mechanism:** Remove duplicate flag declarations, have consumers include the single canonical header.
**Scale implications:** Reduces maintenance burden — flag changes only need to happen in one place.
**Cost:** Elimination of duplication.

---

### hermes 3bdbb94bf Add --shermes-flag for passing extra flags to shermes
**Author:** Meta Hermes team
**Situation:** No way to pass additional flags to the shermes (SH) subprocess from the main CLI.
**Approach:** Add `--shermes-flag` option that forwards flags to the shermes subprocess.
**Mechanism:** CLI argument parsing with forwarding to subprocess.
**Scale implications:** Improves debuggability — operators can pass internal flags without modifying the main CLI surface.
**Cost:** New CLI surface (minor).

---

### hermes 953d7c4b7 Add --shermes subprocess execution mode
**Author:** Meta Hermes team
**Situation:** SH (Hermes bytecode compiler) ran in-process for some modes, making crash recovery difficult.
**Approach:** Spawn SH as a subprocess instead, enabling process-level isolation.
**Mechanism:** Fork/exec pattern for shermes execution. Enables the crash guard (`sigsetjmp`) to work since a subprocess crash doesn't kill the parent.
**Scale implications:** Key architectural change — enables safe execution of untrusted or potentially buggy bytecode by isolating it in a subprocess. This is the foundation for the crash guard.
**Cost:** Significant refactor for execution mode.

---

### hermes 22c76dc34 Add --jit flag for JIT compilation mode
**Author:** Meta Hermes team
**Situation:** No explicit JIT mode in the CLI.
**Approach:** Add `--jit` flag to enable JIT compilation mode.
**Mechanism:** New CLI mode that activates the JIT compiler path.
**Scale implications:** For apps that prioritize peak performance over startup time, JIT enables runtime optimization.
**Cost:** New feature.

---

### hermes 9017d3938 Add --lazy flag for lazy compilation mode
**Author:** Meta Hermes team
**Situation:** No explicit lazy compilation mode.
**Approach:** Add `--lazy` flag.
**Mechanism:** Lazy compilation defers bytecode generation to first-use rather than upfront, reducing startup time at the cost of peak performance.
**Scale implications:** Balances Hermes's core value proposition — fast startup — by allowing a mode that defers compilation cost.
**Cost:** New feature.

---

### hermes abce32a68 Add -O flag for optimization passes
**Author:** Meta Hermes team
**Situation:** No explicit optimization level control in CLI.
**Approach:** Add `-O` flag for optimization pass selection.
**Mechanism:** Passes optimization level to the compiler pipeline.
**Scale implications:** Operators can tune compilation for their workload — low-O for fast compile (debugging), high-O for peak performance.
**Cost:** New feature.

---

### hermes 26fb29680 Add DESIGN.md documenting architecture and design decisions
**Author:** Meta Hermes team
**Situation:** No central architecture document for SH (Standalone Hermes bytecode compiler/executor).
**Approach:** Write `DESIGN.md` documenting key decisions.
**Mechanism:** Markdown documentation.
**Scale implications:** Critical for bus factor — without docs, knowledge lives in people's heads. The document captures WHY decisions were made, not just what.
**Cost:** No runtime impact.

---

### hermes aca54ff3e Phase 7: Skiplist integration, handle sanitizer & intl support
**Author:** Meta Hermes team
**Situation:** Phase 7 of a multi-phase implementation for the IR type system testing framework.
**Approach:** Integrate skiplist for test selection, handle sanitizer support, and internationalization support.
**Mechanism:** Skiplist provides O(log n) test selection; handle sanitizer detects handle management bugs; intl adds locale support.
**Scale implications:** skiplist makes test selection fast for large test suites. Handle sanitizer catches memory bugs that would otherwise leak silently.
**Cost:** Feature additions to test infrastructure.

---

### hermes aaf23e183 Phase 6: Full verification & benchmarking
**Author:** Meta Hermes team
**Situation:** Phase 6 of the IR type system implementation.
**Approach:** Complete verification and benchmarking infrastructure.
**Mechanism:** Full test coverage and performance measurement for the type system.
**Scale implications:** Phase 6 typically means the feature is feature-complete and being validated.
**Cost:** Test/benchmark additions.

---

### hermes 4fc733fc3 Add enableTDZ flag to CompileFlags
**Author:** Meta Hermes team
**Situation:** TDZ (Temporal Dead Zone) enforcement needed a flag for the compiler.
**Approach:** Add `enableTDZ` to `CompileFlags`.
**Mechanism:** New compile flag that controls TDZ checking behavior.
**Scale implications:** TDZ is a JavaScript ES6 feature — this flag controls strictness of `let`/`const` enforcement.
**Cost:** Small.

---

### hermes 725a3a131 Phase 5: Result reporting & progress display
**Author:** Meta Hermes team
**Situation:** Phase 5 of the IR type system project.
**Approach:** Result reporting and progress display for the test harness.
**Mechanism:** Progress UI for long-running verification runs.
**Scale implications:** Good UX for developers running long test suites.
**Cost:** Minor.

---

### hermes eb83767c9 Phase 4: In-process execution engine
**Author:** Meta Hermes team
**Situation:** Phase 4 of the IR type system implementation.
**Approach:** In-process execution engine for running IR tests.
**Mechanism:** Executes IR directly for faster test iteration.
**Scale implications:** In-process is faster than subprocess for the test harness feedback loop.
**Cost:** Core engine addition.

---

### hermes 80b5bb8f2 Phase 3: Frontmatter parsing & source preprocessing
**Author:** Meta Hermes team
**Situation:** Phase 3 of the IR type system project.
**Approach:** Frontmatter parsing and source preprocessing for test files.
**Mechanism:** Allows tests to embed metadata (flags, expected results) directly in source files.
**Scale implications:** Easier test authoring and selection.
**Cost:** Minor.

---

### hermes fedcda5c6 Phase 2: Test discovery & skiplist
**Author:** Meta Hermes team
**Situation:** Phase 2 of the IR type system project.
**Approach:** Test discovery and skiplist-based selection.
**Mechanism:** Automatically discover tests and use skiplist for efficient selection.
**Scale implications:** Foundation for the test infrastructure — enables running subsets of tests.
**Cost:** Minor.

---

### hermes 40a2cc564 Phase 1: C++ project skeleton & CLI
**Author:** Meta Hermes team
**Situation:** Phase 1 kickoff of the IR type system project.
**Approach:** C++ project skeleton and CLI for the type system tool.
**Mechanism:** Basic project structure, argument parsing, etc.
**Scale implications:** First step — establishes the foundation.
**Cost:** Project init.

---

### hermes 83fd239a5 Add inline fast path for strict equality (===)
**Author:** Meta Hermes team
**Situation:** Strict equality (`===`) was going through the general comparison path.
**Approach:** Inline fast path for strict equality when types are known to match.
**Mechanism:** Check if both operands are the same type, and if so, use a direct inline comparison rather than the general ` Equality南北` instruction.
**Scale implications:** `===` is extremely common. Fast-path here benefits nearly every JavaScript program.
**Cost:** Additional branch to maintain.

---

### hermes 224e3b553 TypedLib: Implement map/set size
**Author:** Meta Hermes team
**Situation:** `Map.prototype.size` and `Set.prototype.size` needed implementation in TypedLib.
**Approach:** Implement size getter for Map and Set.
**Mechanism:** `size` property accessor in TypedLib bindings.
**Scale implications:** Missing `size` would cause runtime errors when accessing these properties in typed Hermes code.
**Cost:** Feature addition.

---

### hermes 573cd18d6 TypedLib: Implement Array length
**Author:** Meta Hermes team
**Situation:** `Array.prototype.length` needed TypedLib implementation.
**Approach:** Implement length property for arrays.
**Mechanism:** Property accessor in TypedLib.
**Scale implications:** Fundamental array property — needed for any typed array code.
**Cost:** Feature.

---

### hermes d57226992 Typed: Array destructuring with rest elements
**Author:** Meta Hermes team
**Situation:** Array destructuring with rest elements (`[a, ...b] = arr`) needed support.
**Approach:** Implement rest element support in array destructuring.
**Mechanism:** AST transformation and code generation for rest patterns.
**Scale implications:** Common ES6 pattern — needed for Typed language completeness.
**Cost:** Feature.

---

### hermes 151d3c3a6 Remove fastarray hacks from MiniReact
**Author:** Meta Hermes team
**Situation:** MiniReact had workaround/hacks for fast array behavior that were no longer needed.
**Approach:** Remove the fastarray-specific code paths now that the underlying issue was fixed elsewhere.
**Mechanism:** Remove conditional code paths.
**Scale implications:** Reduction of technical debt — removes a workaround whose original reason no longer applies.
**Cost:** Deletion.

---

### hermes f01a17ebf Use Hermes.decorate() to decorate function declarations
**Author:** Meta Hermes team
**Situation:** Function decoration was done in a way that didn't properly integrate with Hermes's decoration system.
**Approach:** Use `Hermes.decorate()` for function declarations, ensuring proper decorator application.
**Mechanism:** Call `Hermes.decorate()` in decorator processing.
**Scale implications:** Decorators are widely used in React Native — proper integration is critical.
**Cost:** Small refactor.

---

### hermes 238599c23 TypedLib: Implement shift and unshift
**Author:** Meta Hermes team
**Situation:** `Array.prototype.shift()` and `unshift()` needed TypedLib implementation.
**Approach:** Implement shift/unshift methods.
**Mechanism:** TypedLib method implementation.
**Scale implications:** Fundamental array operations — needed for typed code.
**Cost:** Feature.

---

### hermes ce487e1bc TypedLib: use callback.call() in Array methods
**Author:** Meta Hermes team
**Situation:** Array methods that accept callbacks were calling the callback without proper `this` binding.
**Approach:** Use `callback.call()` to properly bind `this` in Array methods.
**Mechanism:** Change callback invocation to use `.call()`.
**Scale implications:** Fixes `this` binding bugs in typed Array methods — would cause incorrect behavior when callback relies on `this`.
**Cost:** Bug fix.

---

### hermes 0faa4f5c4 FlowChecker: Allow omitting arguments when 'void' is allowed
**Author:** Meta Hermes team
**Situation:** FlowChecker (Typed language type checker) was too strict about argument counts when `void` is an acceptable type.
**Approach:** Allow argument omission when the parameter type is `void`.
**Mechanism:** Update type checking logic to permit `void` parameters to be omitted.
**Scale implications:** More permissive type checking for optional parameters.
**Cost:** Small type-check logic change.

---

### hermes 79584969e Typecheck fn.call() like $SHBuiltin.call
**Author:** Meta Hermes team
**Situation:** `fn.call()` had different type checking rules than the built-in `$SHBuiltin.call`.
**Approach:** Make `fn.call()` type-check identically to `$SHBuiltin.call`.
**Mechanism:** Align type checking rules.
**Scale implications:** Consistency between regular and built-in call semantics.
**Cost:** Small.

---

### hermes 0fef59ab1 TypedLib: Implement splice and toSpliced
**Author:** Meta Hermes team
**Situation:** `Array.prototype.splice()` and `toSpliced()` needed implementation.
**Approach:** Implement both splice methods.
**Mechanism:** TypedLib method implementation.
**Scale implications:** Core array operations — critical for typed code completeness.
**Cost:** Feature.

---

### hermes 94f09e21a FlowChecker: Fix union canAFlowIntoB ignoring needsCheckedCast
**Author:** Meta Hermes team
**Situation:** The FlowChecker's `canAFlowIntoB` function was ignoring the `needsCheckedCast` flag when checking union types.
**Approach:** Factor `needsCheckedCast` into the union type flow analysis.
**Mechanism:** Update the type compatibility logic for unions.
**Scale implications:** Type soundness fix — prevents incorrect type assignments that would cause runtime errors.
**Cost:** Bug fix in type checker.

---

### hermes 7c6ff4189 FlowChecker: Typecheck 'arguments'
**Author:** Meta Hermes team
**Situation:** `arguments` object wasn't being type-checked properly.
**Approach:** Add type checking for the `arguments` object in FlowChecker.
**Mechanism:** Add `arguments` to the type environment.
**Scale implications:** `arguments` is a legacy feature — proper type checking ensures typed Hermes doesn't produce bugs when using it.
**Cost:** Small.

---

### hermes a789a4aa1 Typecheck and implement non-pattern rest parameters
**Author:** Meta Hermes team
**Situation:** Rest parameters (`...args`) that aren't in destructuring patterns weren't properly type-checked or implemented.
**Approach:** Add proper type checking and implementation for non-pattern rest parameters.
**Mechanism:** Handle rest parameters in parameter list processing.
**Scale implications:** Common JavaScript pattern — needed for typed language completeness.
**Cost:** Feature.

---

### hermes 8a30722ae FlowChecker: Remove 'static' from canAFlowIntoB
**Author:** Meta Hermes team
**Situation:** `static` keyword was incorrectly included in the `canAFlowIntoB` check.
**Approach:** Remove `static` from the check.
**Mechanism:** Minor type checking logic fix.
**Scale implications:** Type soundness.
**Cost:** Small.

---

### hermes 7f2ff17d9 Implement popping from array in typed language
**Author:** Meta Hermes team
**Situation:** `Array.prototype.pop()` needed typed language implementation.
**Approach:** Implement pop for typed arrays.
**Mechanism:** TypedLib method.
**Scale implications:** Core array operation.
**Cost:** Feature.

---

### hermes 7f2ff17d9 FlowChecker: Improve function param error message
**Author:** Meta Hermes team
**Situation:** Error message for function parameter type mismatches was unclear.
**Approach:** Improve the error message to be more actionable.
**Mechanism:** Better error string construction.
**Scale implications:** Developer experience — better error messages reduce debugging time.
**Cost:** UX improvement.

---

### hermes ecd247c24 Support overload on static methods
**Author:** Meta Hermes team
**Situation:** The Typed language didn't support method overloading on static methods.
**Approach:** Add static method overload support.
**Mechanism:** Extend the overload resolution system to handle static methods.
**Scale implications:** Needed for typed class completeness.
**Cost:** Feature.

---

### hermes e8f895d6f Update hermes-parser and related packages in fbsource to 0.36.1
**Author:** Meta Hermes team
**Situation:** Outdated parser package version in the fbsource integration.
**Approach:** Update to version 0.36.1.
**Mechanism:** Version bump in dependency configuration.
**Scale implications:** Keeps the internal Meta build in sync with upstream parser changes.
**Cost:** Dependency update.

---

### hermes c8dd0b1d3 TypedLib: Add Array reduce/reduceRight
**Author:** Meta Hermes team
**Situation:** `Array.prototype.reduce()` and `reduceRight()` needed TypedLib implementation.
**Approach:** Implement reduce and reduceRight.
**Mechanism:** TypedLib method.
**Scale implications:** Core array operations — reduce is widely used.
**Cost:** Feature.

---

### hermes feecedd53 Implement method overloading for typed language
**Author:** Meta Hermes team
**Situation:** Typed language needed method overloading (multiple signatures for the same method name).
**Approach:** Add method overload resolution to the typed language compiler.
**Mechanism:** Signature-based overload resolution at compile time.
**Scale implications:** Enables more expressive typed APIs.
**Cost:** Feature.

---

### hermes 22485f285 FlowChecker: Refactor visit(MemberExpressionNode)
**Author:** Meta Hermes team
**Situation:** `MemberExpressionNode` visitor in FlowChecker was complex and hard to maintain.
**Approach:** Refactor into smaller, more focused functions.
**Mechanism:** Code reorganization.
**Scale implications:** Maintenance — better structured code is easier to modify safely.
**Cost:** Refactor.

---

### hermes dddd1db38 Easy: Bump per-test timeout in node-api test runners from 30s to 120s
**Author:** Meta Hermes team
**Situation:** Some Node-API tests were timing out at 30 seconds on slower hardware.
**Approach:** Bump the per-test timeout from 30s to 120s.
**Mechanism:** Config change.
**Scale implications:** Reduces flaky test failures on slower CI hardware.
**Cost:** No runtime change.

---

### hermes 0b0e515e7 Fix Promise.prototype.finally: delegate to C.resolve for PromiseResolve
**Author:** Meta Hermes team
**Situation:** `Promise.prototype.finally` wasn't properly delegating to `C.resolve` (the constructor's resolve method) for `PromiseResolve` spec operations.
**Approach:** Make `finally` delegate to `C.resolve` instead of `Promise.resolve`.
**Mechanism:** Change the resolution path in `finally`.
**Scale implications:** Spec compliance — ensures subclassed Promises work correctly with `finally`.
**Cost:** Bug fix.

---

### hermes 31d3f89b7 Features.md: document expanded Promise support + deviations
**Author:** Meta Hermes team
**Situation:** Promise implementation had expanded capabilities beyond spec that weren't documented.
**Approach:** Document the expanded Promise support and deviations from spec.
**Mechanism:** Update `Features.md`.
**Scale implications:** Critical for developers who rely on Promise behavior — documents what's guaranteed vs. implementation detail.
**Cost:** Documentation.

---

### hermes df31e2b75 Promise.withResolvers: delegate to NewPromiseCapability
**Author:** Meta Hermes team
**Situation:** `Promise.withResolvers()` needed to properly delegate to the internal `NewPromiseCapability` abstract operation.
**Approach:** Implement `withResolvers` in terms of `NewPromiseCapability`.
**Mechanism:** Use the abstract operation instead of direct construction.
**Scale implications:** Spec compliance for a newer Promise feature.
**Cost:** Feature.

---

### hermes d0e008b90 Add execution scope to JSI HermesRuntimeImpl methods
**Author:** Meta Hermes team
**Situation:** Some JSI methods lacked proper execution scope management.
**Approach:** Add `ExecutionScopeRAII` to methods that were missing it.
**Mechanism:** RAII guard insertion.
**Scale implications:** Ensures proper handle stack management in JSI calls — prevents handle leaks in JSI integrations.
**Cost:** RAII guard addition.

---

### hermes e09375994 Expand GC-safe coding skill with new hazards
**Author:** Meta Hermes team
**Situation:** The GC-safe coding documentation needed expansion with newly discovered hazard patterns.
**Approach:** Document new hazard patterns.
**Mechanism:** Update GC-safe coding skill/documentation.
**Scale implications:** Critical knowledge for C++ contributors — prevents hard-to-debug GC bugs.
**Cost:** Documentation.

---

### hermes a3c166993 hermes_napi_buffer.cpp: fix stale ArrayBuffer pointer after GC
**Author:** Meta Hermes team
**Situation:** N-API buffer code was holding onto an `ArrayBuffer` pointer that became stale after GC.
**Approach:** Properly handle `ArrayBuffer` pinning or re-fetch after GC.
**Mechanism:** Ensure the buffer reference is kept alive or refreshed.
**Scale implications:** Critical for Node-API bindings — stale pointers cause crashes in native modules that use buffers.
**Cost:** Bug fix.

---

### hermes 336072f8a EASY: hermes_napi_typedarray.cpp: remove risky variable `ab`
**Author:** Meta Hermes team
**Situation:** A variable named `ab` was being used in a way that risked confusion or misuse.
**Approach:** Remove the variable and refactor.
**Mechanism:** Variable elimination.
**Scale implications:** Reduces cognitive load and potential for bugs.
**Cost:** Small cleanup.

---

### hermes 6fac2a504 Set prettier hermes plugin to 0.36.1
**Author:** Meta Hermes team
**Situation:** Prettier plugin version mismatch.
**Approach:** Update to 0.36.1.
**Mechanism:** Version bump.
**Scale implications:** Keeps formatting consistent with current parser.
**Cost:** Dependency update.

---

### hermes c42f418cd Plumb QEMU_RUN_PREFIX through node-api test runners
**Author:** Meta Hermes team
**Situation:** Node-API tests needed to run under QEMU for some architectures but lacked the environment variable propagation.
**Approach:** Propagate `QEMU_RUN_PREFIX` to test runners.
**Mechanism:** Environment variable pass-through.
**Scale implications:** Enables cross-architecture testing via QEMU emulation.
**Cost:** Small.

---

### hermes 91dbbc654 Promise: make internal callbacks non-constructable via arrow funcs
**Author:** Meta Hermes team
**Situation:** Internal Promise callbacks were accidentally constructable via arrow functions in certain edge cases.
**Approach:** Make internal callbacks non-constructable.
**Mechanism:** Check constructor behavior in callback creation.
**Scale implications:** Spec compliance and security — internal callbacks shouldn't be user-constructable.
**Cost:** Small fix.

---

### hermes 5801d6966 Promise: fix property descriptors per spec via ES6 class
**Author:** Meta Hermes team
**Situation:** Promise property descriptors (e.g., `Promise[Symbol.toStringTag]`) weren't set correctly per spec.
**Approach:** Use ES6 class syntax which automatically sets correct property descriptors.
**Mechanism:** Rewrite Promise using ES6 class.
**Scale implications:** Spec compliance — ensures `Object.getOwnPropertyDescriptor(Promise, ...)` returns correct values.
**Cost:** Refactor.

---

### hermes e435d2aef Remove more passed Promise test
**Author:** Meta Hermes team
**Situation:** A Promise test that was previously skipped/passed was being removed now that it's fully supported.
**Approach:** Remove the test that covers already-supported behavior.
**Mechanism:** Delete test file.
**Scale implications:** Test suite maintenance — remove redundant tests.
**Cost:** Deletion.

---

### hermes 173ca33cf Promise.all: drop spec-violating fast path for fulfilled inputs
**Author:** Meta Hermes team
**Situation:** `Promise.all` had a fast path for already-fulfilled promises that violated spec by not checking `Promise resolve` properly.
**Approach:** Remove the fast path to ensure full spec compliance.
**Mechanism:** Remove the special-case optimization.
**Scale implications:** `Promise.all` now handles all spec cases correctly, at the cost of a small performance regression for the common case of already-fulfilled promises.
**Cost:** Performance trade-off for correctness.

---

### hermes 5275cf461 Expose --test262 flag via HermesInternal.test262Enabled()
**Author:** Meta Hermes team
**Situation:** No way to query whether test262 mode is enabled from JavaScript.
**Approach:** Expose the `--test262` flag via `HermesInternal.test262Enabled()`.
**Mechanism:** Add internal API method.
**Scale implications:** Enables JavaScript code to conditionally use features based on test262 mode.
**Cost:** Small feature.

---

### hermes ba51c4ace Promise.try: implement per spec §27.2.4.9
**Author:** Meta Hermes team
**Situation:** `Promise.try` (a non-standard but widely used method) needed proper spec-based implementation.
**Approach:** Implement `Promise.try` per the spec section §27.2.4.9.
**Mechanism:** Use `NewPromiseCapability` with the executor.
**Scale implications:** `Promise.try` is a common utility — spec-compliant implementation ensures consistent behavior.
**Cost:** Feature.

---

### hermes 2c2734592 Promise.prototype.finally: rewrite per spec §27.2.5.3
**Author:** Meta Hermes team
**Situation:** `Promise.prototype.finally` implementation didn't match the spec.
**Approach:** Rewrite per spec §27.2.5.3.
**Mechanism:** Full rewrite of `finally`.
**Scale implications:** Spec compliance — the rewrite correctly handles subclassed Promises and all spec edge cases.
**Cost:** Full rewrite (larger change).

---

### hermes 028c807a3 Promise.allSettled/race/any: rewrite per spec
**Author:** Meta Hermes team
**Situation:** Promise combinators weren't fully spec-compliant.
**Approach:** Rewrite `allSettled`, `race`, and `any` per spec.
**Mechanism:** Full rewrite of these combinators.
**Scale implications:** Full spec compliance for Promise static methods.
**Cost:** Feature rewrite.

---

### hermes ad85b978f Promise.all: rewrite per spec, retain core fast path
**Author:** Meta Hermes team
**Situation:** `Promise.all` needed a full spec rewrite but the core fast path for the common case should be retained.
**Approach:** Rewrite per spec while retaining the fast path optimization.
**Mechanism:** Keep fast path for already-resolved promises, full spec path otherwise.
**Scale implications:** Balances spec compliance with performance — the hot path stays fast.
**Cost:** Feature rewrite with optimization retention.

---

### hermes 448211028 Promise.resolve/reject, internal resolve(): respect this receiver
**Author:** Meta Hermes team
**Situation:** `Promise.resolve`, `Promise.reject`, and internal `resolve()` weren't respecting the `this` receiver when called on a subclass.
**Approach:** Make these methods properly forward to `NewPromiseCapability` with the correct receiver.
**Mechanism:** Pass `this` as the constructor capability receiver.
**Scale implications:** Critical for Promise subclassing — ensures methods call back into the subclass constructor.
**Cost:** Bug fix.

---

### hermes d2422b5e2 Promise.prototype.then: IsPromise check + NewPromiseCapability
**Author:** Meta Hermes team
**Situation:** `Promise.prototype.then` needed proper `IsPromise` checking and `NewPromiseCapability` usage.
**Approach:** Add explicit `IsPromise` check before using `NewPromiseCapability`.
**Mechanism:** Check if the result is a thenable before wrapping.
**Scale implications:** Spec compliance for the core `then` method.
**Cost:** Bug fix.

---

### hermes 3e8781e20 Promise: add @@toStringTag (no @@species support)
**Author:** Meta Hermes team
**Situation:** `Promise[Symbol.toStringTag]` was missing.
**Approach:** Add `@@toStringTag` to Promise.
**Mechanism:** Add symbol property.
**Scale implications:** `Object.prototype.toString.call(Promise.resolve())` now returns `"[object Promise]"` correctly.
**Cost:** Small feature.

---

### hermes 659a94c41 Release prettier-plugin-hermes-parser to version 0.37.1
**Author:** Meta Hermes team
**Situation:** Version bump for the prettier plugin.
**Approach:** Release version 0.37.1.
**Mechanism:** Version bump.
**Scale implications:** Keeps external tooling in sync.
**Cost:** Dependency update.

---

### hermes 6e6a3f3fe napi: Fix createTypedArrayForType to take pinned JSArrayBuffer
**Author:** Meta Hermes team
**Situation:** `createTypedArrayForType` was taking a regular `JSArrayBuffer` but needed a pinned one for safety.
**Approach:** Change the parameter type to `pinned JSArrayBuffer`.
**Mechanism:** Type change in N-API function signature.
**Scale implications:** Ensures the array buffer can't be collected/moved while the typed array is in use.
**Cost:** API change.

---

### hermes fa4867b47 napi: Avoid storing raw values
**Author:** Meta Hermes team
**Situation:** N-API was storing raw C++ values in a way that could be unsafe across GC.
**Approach:** Store values in a GC-safe manner.
**Mechanism:** Use JS value wrappers instead of raw values.
**Scale implications:** Prevents GC-related bugs in N-API code.
**Cost:** Safety fix.

---

### hermes 3b522dfd2 EASY: Use nullptr for ImTextureID in raytracer
**Author:** Meta Hermes team
**Situation:** The raytracer example was using a sentinel value instead of `nullptr` for `ImTextureID`.
**Approach:** Use `nullptr`.
**Mechanism:** Replace sentinel with `nullptr`.
**Scale implications:** Code cleanliness.
**Cost:** Small.

---

### hermes 45f1ab8aa Fix variable name in isPromise function (#2016)
**Author:** Meta Hermes team
**Situation:** Confusing variable name in `isPromise` function.
**Approach:** Rename the variable for clarity.
**Mechanism:** Variable rename.
**Scale implications:** Readability/maintainability.
**Cost:** Small.

---

### hermes adf76f44a Vendor Node-API conformance test suites (#2006)
**Author:** Meta Hermes team
**Situation:** Node-API conformance tests weren't vendored, making compliance testing difficult.
**Approach:** Vendor the Node-API conformance test suites.
**Mechanism:** Copy test suite into the repo.
**Scale implications:** Enables local conformance testing — critical for Node-API compliance.
**Cost:** Large (vendoring many files).

---

### hermes b74f274bb Add Node-API unit tests and lit integration tests
**Author:** Meta Hermes team
**Situation:** Node-API needed proper unit tests and LIT integration.
**Approach:** Add unit tests and LIT integration.
**Mechanism:** New test files.
**Scale implications:** Comprehensive Node-API coverage — catch regressions before they reach users.
**Cost:** Feature.

---

### hermes ff31291e6 Add Node-API implementation for Hermes
**Author:** Meta Hermes team
**Situation:** Hermes didn't have a Node-API implementation.
**Approach:** Implement the Node-API interface for Hermes.
**Mechanism:** Full Node-API implementation.
**Scale implications:** Major feature — enables Node.js native modules to work with Hermes. This is the foundation for React Native's native module ecosystem.
**Cost:** Large feature implementation.

---

### hermes b90df8bf7 Vendor Node-API headers
**Author:** Meta Hermes team
**Situation:** Node-API headers weren't vendored.
**Approach:** Vendor the Node-API headers.
**Mechanism:** Copy headers into the repo.
**Scale implications:** Required for Node-API implementation.
**Cost:** Vendoring.

---

### hermes 8bdeeeb4a P1-S8: Rewrite Type class to use TypeContext
**Author:** Meta Hermes team
**Situation:** The `Type` class needed to use `TypeContext` for proper type interning.
**Approach:** Rewrite `Type` to use `TypeContext`.
**Mechanism:** Use `TypeContext` for type storage and comparison.
**Scale implications:** Critical for the IR type system — proper type interning reduces memory and enables fast type comparison.
**Cost:** Core refactor.

---

### hermes cfe692ba2 P1-S7.5: Add RAII guards to unit tests
**Author:** Meta Hermes team
**Situation:** Unit tests were missing RAII guards for type context operations.
**Approach:** Add RAII guards to unit tests.
**Mechanism:** `ExecutionScopeRAII` insertions.
**Scale implications:** Tests now properly manage type context state — prevents test pollution.
**Cost:** Test additions.

---

### hermes d06e7a7e4 P1-S7: Install RAII guards at compilation entry points
**Author:** Meta Hermes team
**Situation:** Compilation entry points lacked proper RAII guard installation.
**Approach:** Install RAII guards at compilation entry points.
**Mechanism:** Guard insertion at API boundaries.
**Scale implications:** Ensures proper cleanup of type context at every compilation entry point.
**Cost:** Infrastructure.

---

### hermes 91c5a6b85 P1-S6: Wire IRTypeContext into Module
**Author:** Meta Hermes team
**Situation:** `IRTypeContext` wasn't connected to the `Module`.
**Approach:** Wire `IRTypeContext` into the `Module`.
**Mechanism:** Connect type context to module's type system.
**Scale implications:** The type system now works at the module level.
**Cost:** Integration.

---

### hermes becf5fd4a P1-S5: Add thread-local context and RAII guard to IRTypeContext
**Author:** Meta Hermes team
**Situation:** `IRTypeContext` needed thread-local context support for multi-threaded compilation.
**Approach:** Add thread-local storage and RAII guard to `IRTypeContext`.
**Mechanism:** Thread-local `IRTypeContext` with RAII guard.
**Scale implications:** Enables safe multi-threaded compilation — each thread has its own type context.
**Cost:** Threading infrastructure.

---

### hermes b4e4a3366 P1-S4: Add utility methods to IRTypeContext
**Author:** Meta Hermes team
**Situation:** `IRTypeContext` needed more utility methods.
**Approach:** Add utility methods.
**Mechanism:** Helper methods for type operations.
**Scale implications:** Better API surface for type context operations.
**Cost:** Feature.

---

### hermes 14dfd8d45 P1-S3: Add type operations with interning to IRTypeContext
**Author:** Meta Hermes team
**Situation:** Type operations needed proper interning.
**Approach:** Add interned type operations to `IRTypeContext`.
**Mechanism:** Intern types to reduce memory and enable fast comparison.
**Scale implications:** Memory efficiency and fast type comparison.
**Cost:** Feature.

---

### hermes 3e5434270 P1-S2: Add type query predicates to IRTypeContext
**Author:** Meta Hermes team
**Situation:** `IRTypeContext` needed type query predicates (e.g., `isFunction`, `isObject`).
**Approach:** Add predicate methods.
**Mechanism:** Type predicates.
**Scale implications:** Better API for type queries.
**Cost:** Feature.

---

### hermes b7f90ac36 P1-S1: TypeContext skeleton with well-known types
**Author:** Meta Hermes team
**Situation:** Phase 1 of the IR type system — create the skeleton.
**Approach:** Create `TypeContext` skeleton with well-known types (number, string, etc.).
**Mechanism:** Skeleton implementation.
**Scale implications:** Foundation for the type system.
**Cost:** Init.

---

### hermes e91cc037f docs/plans/ir-type: new IR type system design
**Author:** Meta Hermes team
**Situation:** No design doc existed for the IR type system.
**Approach:** Write the design doc.
**Mechanism:** Markdown design document.
**Scale implications:** Critical for design alignment and bus factor mitigation.
**Cost:** Documentation.

---

### hermes 988db9411 Pass the rejected Promise to rejection-tracker callbacks
**Author:** Meta Hermes team
**Situation:** The rejection-tracker wasn't receiving the rejected Promise itself, only the reason.
**Approach:** Pass the Promise to rejection-tracker callbacks.
**Mechanism:** Include the Promise in callback arguments.
**Scale implications:** Enables better rejection tracking and debugging.
**Cost:** Small feature.

---

### hermes 768bab2a5 Fix: enable EBO for simple_ilist on MSVC via LLVM_DECLARE_EMPTY_BASES
**Author:** Meta Hermes team
**Situation:** Empty base optimization (EBO) wasn't enabled for `simple_ilist` on MSVC, causing unnecessary padding.
**Approach:** Use `LLVM_DECLARE_EMPTY_BASES` to enable EBO on MSVC.
**Mechanism:** Add `LLVM_DECLARE_EMPTY_BASES` to the struct.
**Scale implications:** Reduces memory usage for lists on MSVC builds.
**Cost:** Small.

---

### hermes efe501deb SIMD-accelerate JSON string scanning in JSONLexer
**Author:** Meta Hermes team
**Situation:** JSON string scanning was a bottleneck — scalar parsing was slow for large JSON.
**Approach:** Use SIMD instructions (e.g., SSE/AVX) to scan multiple bytes at once.
**Mechanism:** SIMD-based string scanning in `JSONLexer`.
**Scale implications:** Major performance improvement for JSON parsing — a common hot path. SIMD can 4-8x faster string scanning vs scalar.
**Cost:** Platform-specific SIMD code — adds complexity and potential portability concerns.

---

### hermes e146cad96 CLAUDE.md: make ASan+Debug with -O1 the default build configuration
**Author:** Meta Hermes team
**Situation:** The default build configuration didn't include AddressSanitizer (ASan) for catching memory bugs.
**Approach:** Make ASan+Debug with -O1 the default build config.
**Mechanism:** Change default CMake flags.
**Scale implications:** Memory bugs are caught earlier in development rather than in production/stress testing. ASan adds ~2x slowdown but catches use-after-free, double-free, buffer overflows.
**Cost:** Development builds are slower but safer.

---

### hermes 2c9e66ed9 Strengthen safePossiblyNarrowingCast and add hermes_assert
**Author:** Meta Hermes team
**Situation:** `safePossiblyNarrowingCast` was too permissive and there was no `hermes_assert`.
**Approach:** Strengthen the cast safety check and add `hermes_assert`.
**Mechanism:** Better narrowing cast validation + new assertion macro.
**Scale implications:** Catches more type bugs at runtime in debug builds.
**Cost:** Small.

---

### hermes f617d6d55 Fix dict HiddenClass sharing in JSON.parse cache
**Author:** Meta Hermes team
**Situation:** JSON.parse was sharing `HiddenClass` objects across different structures that happened to have the same shape, causing bugs.
**Approach:** Prevent `HiddenClass` sharing for JSON.parse cache entries.
**Mechanism:** Separate `HiddenClass` tracks for JSON-parsed objects vs regular objects.
**Scale implications:** Fixes a subtle bug where object shape confusion would cause property access failures.
**Cost:** Small fix.

---

### hermes 86c32f2ef Fix dict HiddenClass sharing in RegExp .groups
**Author:** Meta Hermes team
**Situation:** RegExp `.groups` was sharing `HiddenClass` with other objects.
**Approach:** Prevent HiddenClass sharing for RegExp `.groups`.
**Mechanism:** Similar to the JSON.parse fix.
**Scale implications:** Fixes RegExp groups property access bugs.
**Cost:** Small fix.

---

### hermes 3fce46020 Fix off-by-one in ArrayStorage::append fast path
**Author:** Meta Hermes team
**Situation:** `ArrayStorage::append` fast path had an off-by-one error.
**Approach:** Fix the boundary condition.
**Mechanism:** Correct the index check in the fast path.
**Scale implications:** Would cause buffer overflow in the fast path for a specific array growth pattern.
**Cost:** Bug fix.

---

### hermes 8b8bc024e doc/plans/ir-type/memory.md - simplify
**Author:** Meta Hermes team
**Situation:** The memory design doc was overly complex.
**Approach:** Simplify the document.
**Mechanism:** Rewrite.
**Scale implications:** Better readability for the design doc.
**Cost:** Documentation.

---

### hermes ae2529895 doc/plans/ir-type: progress and memory document from LLM loop
**Author:** Meta Hermes team
**Situation:** The IR type system design needed memory documentation.
**Approach:** Write the memory design for the IR type system.
**Mechanism:** Markdown design doc.
**Scale implications:** Important for maintainability of the type system.
**Cost:** Documentation.

---

### hermes 006ee2b0f Optimize put-by-val for numeric keys and add inline array write fast path
**Author:** Meta Hermes team
**Situation:** Array writes with numeric keys weren't fully optimized.
**Approach:** Add inline fast path for numeric key array writes.
**Mechanism:** Detect numeric index and use direct array write path.
**Scale implications:** Significant for array-heavy workloads.
**Cost:** Additional code path.

---

### hermes 8dbde0267 Lock-free results: pre-allocated slots + StringRef testName
**Author:** Meta Hermes team
**Situation:** Lock-free result processing had allocation during critical sections.
**Approach:** Pre-allocate result slots; use `StringRef` instead of string copy for test names.
**Mechanism:** Arena pre-allocation for results; string reference for names.
**Scale implications:** Eliminates allocation during lock-free operations, reducing latency spikes.
**Cost:** Memory overhead for pre-allocation.

---

### hermes be8eb01e0 Replace reportProgress() with polled ProgressReporter
**Author:** Meta Hermes team
**Situation:** Progress reporting used push callbacks which require lifetime management.
**Approach:** Switch to polling model.
**Mechanism:** Query-based progress reporting.
**Scale implications:** Simpler lifetime management for progress reporters.
**Cost:** Minor API change.

---

### hermes c98c183bd Add sigsetjmp crash guard for in-process bytecode execution
**Author:** Meta Hermes team
**Situation:** Bad bytecode could crash the host process during SH execution.
**Approach:** Wrap bytecode execution in `sigsetjmp`/`siglongjmp` to catch and recover from crashes.
**Mechanism:** Signal-based crash recovery scoped to execution unit.
**Scale implications:** Critical for running untrusted bytecode safely. Without this, a single bad bytecode file crashes the entire CLI.
**Cost:** Signal handling setup per execution.

---

### hermes 15a4686f5 Fix: check async test results regardless of exit code
**Author:** Meta Hermes team
**Situation:** Async test failures were not being detected when tests exited with code 0.
**Approach:** Always check async result state in addition to exit code.
**Mechanism:** Result state inspection post-process.
**Scale implications:** Prevents silent async test failures.
**Cost:** Test infrastructure change.

---

### hermes 9c56a991b Reuse RuntimeFlags.h instead of duplicating flags
**Author:** Meta Hermes team
**Situation:** Flag definitions duplicated across headers.
**Approach:** Single source of truth in RuntimeFlags.h.
**Mechanism:** Include single header instead of duplicate definitions.
**Scale implications:** Single-point update for flags.
**Cost:** Cleanup.

---

### hermes 3bdbb94bf Add --shermes-flag for passing extra flags to shermes
**Author:** Meta Hermes team
**Situation:** No way to pass internal flags to shermes subprocess.
**Approach:** Add `--shermes-flag` forwarding option.
**Mechanism:** CLI flag forwarding.
**Scale implications:** Better debuggability.
**Cost:** CLI surface addition.

---

### hermes 953d7c4b7 Add --shermes subprocess execution mode
**Author:** Meta Hermes team
**Situation:** SH ran in-process, making crash recovery impossible.
**Approach:** Spawn SH as subprocess for process-level isolation.
**Mechanism:** Fork/exec model.
**Scale implications:** Foundation for safe bytecode execution and crash recovery.
**Cost:** Architecture change.

---

### hermes 22c76dc34 Add --jit flag for JIT compilation mode
**Author:** Meta Hermes team
**Situation:** No explicit JIT mode flag.
**Approach:** Add `--jit` CLI flag.
**Mechanism:** JIT mode activation.
**Scale implications:** Enables peak performance mode at cost of startup time.
**Cost:** Feature.

---

### hermes 9017d3938 Add --lazy flag for lazy compilation mode
**Author:** Meta Hermes team
**Situation:** No explicit lazy compilation mode.
**Approach:** Add `--lazy` flag.
**Mechanism:** Lazy compilation activation.
**Scale implications:** Balances startup time vs peak performance.
**Cost:** Feature.

---

### hermes abce32a68 Add -O flag for optimization passes
**Author:** Meta Hermes team
**Situation:** No optimization level control.
**Approach:** Add `-O` flag.
**Mechanism:** Optimization level passing to compiler.
**Scale implications:** Tunable compilation.
**Cost:** Feature.

---

### hermes 26fb29680 Add DESIGN.md documenting architecture and design decisions
**Author:** Meta Hermes team
**Situation:** No central architecture doc for SH.
**Approach:** Write DESIGN.md.
**Mechanism:** Markdown documentation.
**Scale implications:** Bus factor mitigation.
**Cost:** Documentation.

---

### hermes aca54ff3e Phase 7: Skiplist integration, handle sanitizer & intl support
**Author:** Meta Hermes team
**Situation:** Phase 7 of the IR type system project.
**Approach:** Integrate skiplist, handle sanitizer, intl.
**Mechanism:** Test infrastructure improvements.
**Scale implications:** Better test selection, memory safety detection, locale support.
**Cost:** Feature additions.

---

### hermes aaf23e183 Phase 6: Full verification & benchmarking
**Author:** Meta Hermes team
**Situation:** Phase 6 of IR type system.
**Approach:** Verification and benchmarking.
**Mechanism:** Testing infrastructure.
**Scale implications:** Feature validation.
**Cost:** Testing.

---

### hermes 4fc733fc3 Add enableTDZ flag to CompileFlags
**Author:** Meta Hermes team
**Situation:** TDZ enforcement needed a compile flag.
**Approach:** Add to CompileFlags.
**Mechanism:** Flag addition.
**Scale implications:** Controls strictness of `let`/`const`.
**Cost:** Small.

---

### hermes 725a3a131 Phase 5: Result reporting & progress display
**Author:** Meta Hermes team
**Situation:** Phase 5 of IR type system.
**Approach:** Result reporting and progress UI.
**Mechanism:** Progress display.
**Scale implications:** Developer UX.
**Cost:** Minor.

---

### hermes eb83767c9 Phase 4: In-process execution engine
**Author:** Meta Hermes team
**Situation:** Phase 4 of IR type system.
**Approach:** In-process execution engine.
**Mechanism:** Direct IR execution.
**Scale implications:** Faster test iteration.
**Cost:** Feature.

---

### hermes 80b5bb8f2 Phase 3: Frontmatter parsing & source preprocessing
**Author:** Meta Hermes team
**Situation:** Phase 3 of IR type system.
**Approach:** Frontmatter parsing.
**Mechanism:** Test metadata in source.
**Scale implications:** Easier test authoring.
**Cost:** Minor.

---

### hermes fedcda5c6 Phase 2: Test discovery & skiplist
**Author:** Meta Hermes team
**Situation:** Phase 2 of IR type system.
**Approach:** Test discovery with skiplist.
**Mechanism:** Automatic test discovery.
**Scale implications:** Efficient test selection.
**Cost:** Minor.

---

### hermes 40a2cc564 Phase 1: C++ project skeleton & CLI
**Author:** Meta Hermes team
**Situation:** Phase 1 of IR type system.
**Approach:** Project skeleton and CLI.
**Mechanism:** Project init.
**Scale implications:** Foundation.
**Cost:** Init.

---

### hermes 83fd239a5 Add inline fast path for strict equality (===)
**Author:** Meta Hermes team
**Situation:** Strict equality was using general comparison.
**Approach:** Inline fast path when types match.
**Mechanism:** Direct comparison for same-type operands.
**Scale implications:** `===` is ubiquitous — fast path benefits everything.
**Cost:** Additional branch.

---

### hermes 224e3b553 TypedLib: Implement map/set size
**Author:** Meta Hermes team
**Situation:** `Map.size` and `Set.size` not implemented.
**Approach:** Implement size getter.
**Mechanism:** Property accessor.
**Scale implications:** Fundamental collection property.
**Cost:** Feature.

---

### hermes 573cd18d6 TypedLib: Implement Array length
**Author:** Meta Hermes team
**Situation:** Array.length not implemented in TypedLib.
**Approach:** Implement length property.
**Mechanism:** Property accessor.
**Scale implications:** Core array property.
**Cost:** Feature.

---

### hermes d57226992 Typed: Array destructuring with rest elements
**Author:** Meta Hermes team
**Situation:** Array destructuring with rest (`[a, ...b]`) not supported.
**Approach:** Implement rest in array destructuring.
**Mechanism:** AST transformation.
**Scale implications:** Common ES6 pattern.
**Cost:** Feature.

---

### hermes 151d3c3a6 Remove fastarray hacks from MiniReact
**Author:** Meta Hermes team
**Situation:** Workaround code for a fixed issue remained.
**Approach:** Remove the workaround.
**Mechanism:** Code deletion.
**Scale implications:** Technical debt reduction.
**Cost:** Deletion.

---

### hermes f01a17ebf Use Hermes.decorate() to decorate function declarations
**Author:** Meta Hermes team
**Situation:** Function decoration wasn't using the proper Hermes decorator API.
**Approach:** Use `Hermes.decorate()`.
**Mechanism:** Proper decorator integration.
**Scale implications:** Correct decorator application.
**Cost:** Small refactor.

---

### hermes 238599c23 TypedLib: Implement shift and unshift
**Author:** Meta Hermes team
**Situation:** Array shift/unshift not implemented in TypedLib.
**Approach:** Implement shift and unshift.
**Mechanism:** TypedLib method.
**Scale implications:** Core array operations.
**Cost:** Feature.

---

### hermes ce487e1bc TypedLib: use callback.call() in Array methods
**Author:** Meta Hermes team
**Situation:** Array method callbacks didn't properly bind `this`.
**Approach:** Use `callback.call()` for proper `this` binding.
**Mechanism:** `.call()` invocation.
**Scale implications:** Fixes `this` binding bugs.
**Cost:** Bug fix.

---

### hermes 0faa4f5c4 FlowChecker: Allow omitting arguments when 'void' is allowed
**Author:** Meta Hermes team
**Situation:** FlowChecker too strict about optional void parameters.
**Approach:** Allow omission for `void` type parameters.
**Mechanism:** Type check relaxation.
**Scale implications:** More permissive type checking.
**Cost:** Small.

---

### hermes 79584969e Typecheck fn.call() like $SHBuiltin.call
**Author:** Meta Hermes team
**Situation:** `fn.call()` had different type checking than built-in.
**Approach:** Align type checking rules.
**Mechanism:** Rule alignment.
**Scale implications:** Consistency.
**Cost:** Small.

---

### hermes 0fef59ab1 TypedLib: Implement splice and toSpliced
**Author:** Meta Hermes team
**Situation:** Array splice not implemented.
**Approach:** Implement splice and toSpliced.
**Mechanism:** TypedLib method.
**Scale implications:** Core array operation.
**Cost:** Feature.

---

### hermes 94f09e21a FlowChecker: Fix union canAFlowIntoB ignoring needsCheckedCast
**Author:** Meta Hermes team
**Situation:** Type soundness bug in union flow checking.
**Approach:** Include `needsCheckedCast` in union flow analysis.
**Mechanism:** Type compatibility fix.
**Scale implications:** Prevents incorrect type assignments.
**Cost:** Bug fix.

---

### hermes 7c6ff4189 FlowChecker: Typecheck 'arguments'
**Author:** Meta Hermes team
**Situation:** `arguments` not type-checked.
**Approach:** Add `arguments` to type environment.
**Mechanism:** Type environment update.
**Scale implications:** Proper type checking for legacy feature.
**Cost:** Small.

---

### hermes a789a4aa1 Typecheck and implement non-pattern rest parameters
**Author:** Meta Hermes team
**Situation:** Rest parameters not type-checked.
**Approach:** Add type checking for rest params.
**Mechanism:** Parameter list handling.
**Scale implications:** ES6 pattern support.
**Cost:** Feature.

---

### hermes 8a30722ae FlowChecker: Remove 'static' from canAFlowIntoB
**Author:** Meta Hermes team
**Situation:** `static` keyword incorrectly included.
**Approach:** Remove from check.
**Mechanism:** Type check fix.
**Scale implications:** Type soundness.
**Cost:** Small.

---

### hermes 7f2ff17d9 Implement popping from array in typed language
**Author:** Meta Hermes team
**Situation:** `Array.prototype.pop()` not implemented.
**Approach:** Implement pop.
**Mechanism:** TypedLib method.
**Scale implications:** Core array operation.
**Cost:** Feature.

---

### hermes ecd247c24 Support overload on static methods
**Author:** Meta Hermes team
**Situation:** Static method overloading not supported.
**Approach:** Add overload support for static methods.
**Mechanism:** Overload resolution for statics.
**Scale implications:** Typed class completeness.
**Cost:** Feature.

---

### hermes c8dd0b1d3 TypedLib: Add Array reduce/reduceRight
**Author:** Meta Hermes team
**Situation:** `reduce` not implemented.
**Approach:** Implement reduce and reduceRight.
**Mechanism:** TypedLib method.
**Scale implications:** Core array operation — widely used.
**Cost:** Feature.

---

### hermes feecedd53 Implement method overloading for typed language
**Author:** Meta Hermes team
**Situation:** Typed language lacked method overloading.
**Approach:** Add compile-time overload resolution.
**Mechanism:** Signature-based resolution.
**Scale implications:** More expressive typed APIs.
**Cost:** Feature.

---

### hermes 22485f285 FlowChecker: Refactor visit(MemberExpressionNode)
**Author:** Meta Hermes team
**Situation:** Complex visitor method hard to maintain.
**Approach:** Refactor into focused functions.
**Mechanism:** Code reorganization.
**Scale implications:** Maintainability.
**Cost:** Refactor.

---

### hermes dddd1db38 Easy: Bump per-test timeout in node-api test runners from 30s to 120s
**Author:** Meta Hermes team
**Situation:** Slow hardware causing test timeouts.
**Approach:** Increase timeout from 30s to 120s.
**Mechanism:** Config change.
**Scale implications:** Fewer flaky test failures.
**Cost:** No runtime change.

---

### hermes 0b0e515e7 Fix Promise.prototype.finally: delegate to C.resolve for PromiseResolve
**Author:** Meta Hermes team
**Situation:** `finally` didn't delegate to constructor's resolve.
**Approach:** Use `C.resolve` instead of `Promise.resolve`.
**Mechanism:** Fix delegation.
**Scale implications:** Proper Promise subclassing.
**Cost:** Bug fix.

---

### hermes df31e2b75 Promise.withResolvers: delegate to NewPromiseCapability
**Author:** Meta Hermes team
**Situation:** `Promise.withResolvers()` not using abstract operation.
**Approach:** Delegate to `NewPromiseCapability`.
**Mechanism:** Abstract operation call.
**Scale implications:** Spec compliance.
**Cost:** Feature.

---

### hermes d0e008b90 Add execution scope to JSI HermesRuntimeImpl methods
**Author:** Meta Hermes team
**Situation:** Some JSI methods missing execution scope management.
**Approach:** Add `ExecutionScopeRAII` to those methods.
**Mechanism:** RAII guard insertion.
**Scale implications:** Prevents handle leaks in JSI calls.
**Cost:** RAII addition.

---

### hermes e09375994 Expand GC-safe coding skill with new hazards
**Author:** Meta Hermes team
**Situation:** GC-safe coding docs needed new hazard patterns documented.
**Approach:** Add newly discovered hazards to documentation.
**Mechanism:** Documentation update.
**Scale implications:** Prevents hard-to-debug GC bugs for contributors.
**Cost:** Documentation.

---

### hermes a3c166993 hermes_napi_buffer.cpp: fix stale ArrayBuffer pointer after GC
**Author:** Meta Hermes team
**Situation:** N-API buffer held stale pointer after GC.
**Approach:** Properly pin or refresh ArrayBuffer reference.
**Mechanism:** Buffer reference management.
**Scale implications:** Prevents crashes in native modules using buffers.
**Cost:** Bug fix.

---

### hermes 336072f8a EASY: hermes_napi_typedarray.cpp: remove risky variable `ab`
**Author:** Meta Hermes team
**Situation:** Confusing variable name.
**Approach:** Remove variable.
**Mechanism:** Code deletion.
**Scale implications:** Readability.
**Cost:** Small cleanup.

---

### hermes 6fac2a504 Set prettier hermes plugin to 0.36.1
**Author:** Meta Hermes team
**Situation:** Outdated prettier plugin version.
**Approach:** Update to 0.36.1.
**Mechanism:** Version bump.
**Scale implications:** Formatting consistency.
**Cost:** Dependency update.

---

### hermes c42f418cd Plumb QEMU_RUN_PREFIX through node-api test runners
**Author:** Meta Hermes team
**Situation:** QEMU prefix not propagated to tests.
**Approach:** Propagate environment variable.
**Mechanism:** ENV forwarding.
**Scale implications:** Cross-arch testing via emulation.
**Cost:** Small.

---

### hermes 91dbbc654 Promise: make internal callbacks non-constructable via arrow funcs
**Author:** Meta Hermes team
**Situation:** Internal callbacks accidentally constructable.
**Approach:** Prevent construction via arrow functions.
**Mechanism:** Constructor check.
**Scale implications:** Spec compliance.
**Cost:** Small fix.

---

### hermes 5801d6966 Promise: fix property descriptors per spec via ES6 class
**Author:** Meta Hermes team
**Situation:** Incorrect property descriptors on Promise.
**Approach:** Use ES6 class syntax for correct descriptors.
**Mechanism:** Class rewrite.
**Scale implications:** Spec compliance.
**Cost:** Refactor.

---

### hermes e435d2aef Remove more passed Promise test
**Author:** Meta Hermes team
**Situation:** Redundant test.
**Approach:** Remove test.
**Mechanism:** File deletion.
**Scale implications:** Test suite maintenance.
**Cost:** Deletion.

---

### hermes 173ca33cf Promise.all: drop spec-violating fast path for fulfilled inputs
**Author:** Meta Hermes team
**Situation:** `Promise.all` fast path violated spec.
**Approach:** Remove fast path for correctness.
**Mechanism:** Remove optimization.
**Scale implications:** Correctness over micro-optimization.
**Cost:** Performance trade-off.

---

### hermes 5275cf461 Expose --test262 flag via HermesInternal.test262Enabled()
**Author:** Meta Hermes team
**Situation:** No way to query test262 mode from JS.
**Approach:** Add `HermesInternal.test262Enabled()`.
**Mechanism:** Internal API method.
**Scale implications:** Enables conditional feature usage.
**Cost:** Small feature.

---

### hermes ba51c4ace Promise.try: implement per spec §27.2.4.9
**Author:** Meta Hermes team
**Situation:** `Promise.try` not spec-compliant.
**Approach:** Implement per spec.
**Mechanism:** Use NewPromiseCapability.
**Scale implications:** Common utility, spec-compliant.
**Cost:** Feature.

---

### hermes 2c2734592 Promise.prototype.finally: rewrite per spec §27.2.5.3
**Author:** Meta Hermes team
**Situation:** `finally` not spec-compliant.
**Approach:** Full rewrite per spec.
**Mechanism:** Spec implementation.
**Scale implications:** Proper Promise subclassing.
**Cost:** Full rewrite.

---

### hermes 028c807a3 Promise.allSettled/race/any: rewrite per spec
**Author:** Meta Hermes team
**Situation:** Promise combinators not spec-compliant.
**Approach:** Rewrite per spec.
**Mechanism:** Spec implementation.
**Scale implications:** Spec compliance for combinators.
**Cost:** Feature rewrite.

---

### hermes ad85b978f Promise.all: rewrite per spec, retain core fast path
**Author:** Meta Hermes team
**Situation:** `Promise.all` not spec-compliant but needed optimization.
**Approach:** Rewrite per spec, keep fast path for resolved promises.
**Mechanism:** Balanced implementation.
**Scale implications:** Spec compliance + performance.
**Cost:** Feature rewrite.

---

### hermes 448211028 Promise.resolve/reject, internal resolve(): respect this receiver
**Author:** Meta Hermes team
**Situation:** Methods didn't respect `this` receiver for subclasses.
**Approach:** Pass correct receiver to NewPromiseCapability.
**Mechanism:** Fix receiver passing.
**Scale implications:** Promise subclassing works correctly.
**Cost:** Bug fix.

---

### hermes d2422b5e2 Promise.prototype.then: IsPromise check + NewPromiseCapability
**Author:** Meta Hermes team
**Situation:** `then` missing proper IsPromise check.
**Approach:** Add IsPromise check.
**Mechanism:** Thenable check.
**Scale implications:** Spec compliance.
**Cost:** Bug fix.

---

### hermes 3e8781e20 Promise: add @@toStringTag (no @@species support)
**Author:** Meta Hermes team
**Situation:** Missing toStringTag.
**Approach:** Add toStringTag.
**Mechanism:** Symbol property.
**Scale implications:** Correct toString behavior.
**Cost:** Small.

---

### hermes 659a94c41 Release prettier-plugin-hermes-parser to version 0.37.1
**Author:** Meta Hermes team
**Situation:** Version bump.
**Approach:** Release 0.37.1.
**Mechanism:** Version bump.
**Scale implications:** External tooling sync.
**Cost:** Dependency update.

---

### hermes 6e6a3f3fe napi: Fix createTypedArrayForType to take pinned JSArrayBuffer
**Author:** Meta Hermes team
**Situation:** Pinned ArrayBuffer needed for safety.
**Approach:** Change parameter type.
**Mechanism:** Type fix.
**Scale implications:** Safety for typed arrays.
**Cost:** API change.

---

### hermes fa4867b47 napi: Avoid storing raw values
**Author:** Meta Hermes team
**Situation:** Raw values stored unsafely across GC.
**Approach:** Use JS value wrappers.
**Mechanism:** Safe value storage.
**Scale implications:** GC safety.
**Cost:** Safety fix.

---

### hermes adf76f44a Vendor Node-API conformance test suites
**Author:** Meta Hermes team
**Situation:** Node-API conformance tests not vendored.
**Approach:** Vendor conformance test suites.
**Mechanism:** Copy tests into repo.
**Scale implications:** Enables local conformance testing.
**Cost:** Large vendoring.

---

### hermes b74f274bb Add Node-API unit tests and lit integration tests
**Author:** Meta Hermes team
**Situation:** Node-API lacked proper test coverage.
**Approach:** Add unit tests and LIT integration.
**Mechanism:** New tests.
**Scale implications:** Comprehensive Node-API testing.
**Cost:** Feature.

---

### hermes ff31291e6 Add Node-API implementation for Hermes
**Author:** Meta Hermes team
**Situation:** Hermes had no Node-API.
**Approach:** Full Node-API implementation.
**Mechanism:** Implementation of the Node-API spec.
**Scale implications:** **Major feature** — enables native Node.js modules. Foundation for React Native native module ecosystem.
**Cost:** Large.

---

### hermes b90df8bf7 Vendor Node-API headers
**Author:** Meta Hermes team
**Situation:** Node-API headers not vendored.
**Approach:** Vendor headers.
**Mechanism:** Copy headers.
**Scale implications:** Required for Node-API.
**Cost:** Vendoring.

---

### hermes 8bdeeeb4a P1-S8: Rewrite Type class to use TypeContext
**Author:** Meta Hermes team
**Situation:** Type class needed type interning via TypeContext.
**Approach:** Rewrite Type to use TypeContext.
**Mechanism:** Type interning.
**Scale implications:** Memory efficiency and fast comparison.
**Cost:** Core refactor.

---

### hermes cfe692ba2 P1-S7.5: Add RAII guards to unit tests
**Author:** Meta Hermes team
**Situation:** Tests missing RAII guards.
**Approach:** Add guards to tests.
**Mechanism:** RAII insertion.
**Scale implications:** Proper test cleanup.
**Cost:** Test update.

---

### hermes d06e7a7e4 P1-S7: Install RAII guards at compilation entry points
**Author:** Meta Hermes team
**Situation:** Compilation entry points lacked RAII guards.
**Approach:** Install guards at API boundaries.
**Mechanism:** Guard installation.
**Scale implications:** Proper cleanup at every boundary.
**Cost:** Infrastructure.

---

### hermes 91c5a6b85 P1-S6: Wire IRTypeContext into Module
**Author:** Meta Hermes team
**Situation:** IRTypeContext not connected to Module.
**Approach:** Wire it in.
**Mechanism:** Integration.
**Scale implications:** Type system works at module level.
**Cost:** Integration.

---

### hermes becf5fd4a P1-S5: Add thread-local context and RAII guard to IRTypeContext
**Author:** Meta Hermes team
**Situation:** IRTypeContext needed thread-safety.
**Approach:** Add thread-local storage.
**Mechanism:** Thread-local context.
**Scale implications:** Safe multi-threaded compilation.
**Cost:** Threading.

---

### hermes b4e4a3366 P1-S4: Add utility methods to IRTypeContext
**Author:** Meta Hermes team
**Situation:** IRTypeContext needed utility methods.
**Approach:** Add utility methods.
**Mechanism:** API additions.
**Scale implications:** Better API surface.
**Cost:** Feature.

---

### hermes 14dfd8d45 P1-S3: Add type operations with interning to IRTypeContext
**Author:** Meta Hermes team
**Situation:** Type operations needed interning.
**Approach:** Add interned operations.
**Mechanism:** Type interning.
**Scale implications:** Memory and performance.
**Cost:** Feature.

---

### hermes 3e5434270 P1-S2: Add type query predicates to IRTypeContext
**Author:** Meta Hermes team
**Situation:** IRTypeContext needed predicate methods.
**Approach:** Add predicates.
**Mechanism:** Type predicates.
**Scale implications:** Better API.
**Cost:** Feature.

---

### hermes b7f90ac36 P1-S1: TypeContext skeleton with well-known types
**Author:** Meta Hermes team
**Situation:** Phase 1: create skeleton.
**Approach:** Create TypeContext skeleton.
**Mechanism:** Skeleton.
**Scale implications:** Foundation for type system.
**Cost:** Init.

---

### hermes e91cc037f docs/plans/ir-type: new IR type system design
**Author:** Meta Hermes team
**Situation:** No design doc for IR type system.
**Approach:** Write design doc.
**Mechanism:** Documentation.
**Scale implications:** Bus factor mitigation.
**Cost:** Documentation.

---

### hermes 988db9411 Pass the rejected Promise to rejection-tracker callbacks
**Author:** Meta Hermes team
**Situation:** Rejection tracker didn't get the Promise itself.
**Approach:** Include Promise in callback.
**Mechanism:** Callback argument addition.
**Scale implications:** Better rejection debugging.
**Cost:** Small feature.

---

### hermes 768bab2a5 Fix: enable EBO for simple_ilist on MSVC
**Author:** Meta Hermes team
**Situation:** Empty base optimization not enabled on MSVC.
**Approach:** Use LLVM_DECLARE_EMPTY_BASES.
**Mechanism:** EBO enablement.
**Scale implications:** Memory efficiency on MSVC.
**Cost:** Small.

---

### hermes efe501deb SIMD-accelerate JSON string scanning in JSONLexer
**Author:** Meta Hermes team
**Situation:** JSON string scanning was slow.
**Approach:** Use SIMD for parallel byte scanning.
**Mechanism:** SIMD intrinsics in JSONLexer.
**Scale implications:** Major JSON parsing speedup — common hot path. 4-8x faster on supported hardware.
**Cost:** Platform-specific SIMD code; complexity.

---

### hermes e146cad96 CLAUDE.md: make ASan+Debug with -O1 the default build configuration
**Author:** Meta Hermes team
**Situation:** ASan not enabled by default.
**Approach:** Make ASan+Debug+O1 the default.
**Mechanism:** CMake default change.
**Scale implications:** Memory bugs caught earlier in development.
**Cost:** Slower debug builds (with safety benefit).

---

### hermes 2c9e66ed9 Strengthen safePossiblyNarrowingCast and add hermes_assert
**Author:** Meta Hermes team
**Situation:** Unsafe narrowing cast and no assertion macro.
**Approach:** Strengthen cast check; add hermes_assert.
**Mechanism:** Better validation + assertion macro.
**Scale implications:** Catches type bugs in debug.
**Cost:** Small.

---

### hermes f617d6d55 Fix dict HiddenClass sharing in JSON.parse cache
**Author:** Meta Hermes team
**Situation:** JSON.parse shared HiddenClass incorrectly.
**Approach:** Separate HiddenClass tracks for JSON objects.
**Mechanism:** Separate caching.
**Scale implications:** Fixes property access bugs from shape confusion.
**Cost:** Bug fix.

---

### hermes 86c32f2ef Fix dict HiddenClass sharing in RegExp .groups
**Author:** Meta Hermes team
**Situation:** RegExp .groups shared HiddenClass.
**Approach:** Prevent sharing.
**Mechanism:** Separate caching.
**Scale implications:** Fixes RegExp groups bugs.
**Cost:** Bug fix.

---

### hermes 3fce46020 Fix off-by-one in ArrayStorage::append fast path
**Author:** Meta Hermes team
**Situation:** Off-by-one error in array storage fast path.
**Approach:** Fix boundary check.
**Mechanism:** Index correction.
**Scale implications:** Would overflow buffer in specific growth pattern.
**Cost:** Bug fix.

---

## Key Engineering Patterns

### 1. Crash Isolation via Subprocess + sigsetjmp
Hermes uses a two-layer crash recovery strategy: (1) run bytecode execution in a subprocess so crashes don't kill the CLI, and (2) wrap the subprocess execution with `sigsetjmp`/`siglongjmp` for recovery when crashes occur anyway. This allows the SH tool to run untrusted bytecode safely.

### 2. RAII Guards Everywhere for Handle Safety
The codebase extensively uses RAII (`ExecutionScopeRAII`, `GCScopeMarkerRAII`) to ensure handle stacks are properly flushed at every scope exit and every compilation entry point. The pattern is systematically applied across the codebase, including a multi-phase project (P1-S1 through P1-S8) to retrofit guards onto existing code.

### 3. Feature Flags for Gradual Rollout
Experimental features (`experimental.restart_on_flowconfig_change`) are gated behind flags, disabled in "supported roots" (production configs) but available for experimentation. This is a mature feature flag discipline.

### 4. Type System as Multi-Phase Project
The IR type system is implemented in phases (P1-S1 through P1-S8 and beyond), indicating a large multi-year effort. Each phase is tracked separately and builds on the previous one.

### 5. Promise Implementation = Correctness Over Performance
Multiple commits rewrite Promise combinators for spec compliance, even dropping fast paths that violated the spec. The pattern is: spec compliance first, then optimize if possible without violating spec.

### 6. Node-API as Long-term Investment
Node-API support was added through multiple phases: header vendoring, implementation, conformance test vendoring, unit tests, and LIT integration. This was clearly a significant multi-team effort.

### 7. ASan as Default
Making AddressSanitizer+Debug+O1 the default build config shows a mature engineering culture that prioritizes catching bugs early over build speed.

### 8. Handle Leaks Caught by Debug Allocator
The handle sanitizer (`HERMESVM_DEBUG_MAX_GCSCOPE_HANDLES`) is a debug-only check that catches handle leaks before they become memory bugs. Combined with LIT regression tests that run with `-gc-sanitize-handles=1`, this is defense in depth.

### 9. Typed Language / TypedLib
Hermes implements a typed variant of JavaScript ("TypedLib") alongside the regular engine. This involves a FlowChecker type checker, TypedLib method implementations, and method overloading — a significant extension to the basic JS semantics.

### 10. Multi-Mode CLI Architecture
The CLI supports multiple modes: `--lazy`, `--jit`, `--shermes` (subprocess), and optimization levels via `-O`. Each mode represents a different compilation/execution strategy, reflecting Hermes's design as a configurable engine for different mobile use cases.