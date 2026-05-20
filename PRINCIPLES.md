# Engineering Principles from OSS Archaeology

*Synthesized from 9 Meta/frontend OSS repositories: facebook/hhvm, facebook/hermes, facebook/flow, facebook/relay, facebook/pyrefly, facebook/sapling, facebook/buck2, oxc-project/oxc, rolldown/rolldown, wild-linker/wild.*

---

## 1. Refactors Are Tracked, Numbered, and Bounded

The most consistent pattern across all repos: large refactors are broken into numbered, self-contained commits with clear subject prefixes.

**Examples:**
- pyrefly's ErrorBuilder migration: `[1/16]` through `[16/16]` — Rebecca Chen
- pyrefly's `box_patterns` removal: one directory per commit over 3 days — Neil Mitchell
- Sapling's VFS/no_follow infrastructure: one primitive per commit, stacked

**Why this matters:**
- Each commit is independently compilable and testable
- Failure recovery is trivial: bisect to the last good commit
- The numbering scheme is a tracking tool, not customer-facing
- Reviewers can approve chunks in order without cognitive overload

**Lesson:** Any refactor that touches more than ~3 files in different subsystems should be planned as a numbered series. Don't batch them into one "big refactor" commit.

---

## 2. Feature Flags Have Hard Removal Timelines

Across hhvm and pyrefly, flags are not open-ended experiments — they have explicit removal windows.

**Examples:**
- `enable_abstract_method_optional_parameters`: removed after "feature has been deployed for over a year now"
- `box_patterns`: framed explicitly as "pyrefly is now stable Rust" — the feature flag is now unnecessary

**Why this matters:**
- Flags accumulate. Every flag is a cognitive burden and a branch in the conditional logic
- Teams that don't remove flags end up with a flag graveyard that makes every future change harder
- A flag that's always "on" for 2+ years is not a flag — it's dead code that nobody will ever remove

**Lesson:** Feature flags should have an owner and a removal date at creation time. If a flag stays on for more than 12 months without a removal plan, treat it as technical debt.

---

## 3. Performance Work Is Micro-Specific, Not Micro-Optimized

Every performance commit in OXC, Hermes, Rolldown, and Buck2 follows the same pattern: identify one specific bottleneck, fix exactly that, cite the measurement.

**Examples:**
- OXC: `Eliminate unnecessary iterator clone in outermost_paren_parent` — removed one `.cloned()` chain
- OXC: `Pre-allocate Vec for unresolved_references` — avoids ~13 reallocations for a ~5k reference TS file
- Hermes: `Optimize put-by-val for numeric keys` — inline f64→u32 conversion on the hot path
- Hermes: `Add inline fast path for strict equality (===)` — same-object-bits check, NaN handling, +0/-0 correctness
- Rolldown: `N-API string zero-copy via ArcStr` — eliminates string clone on JS-native boundary

**Why this matters:**
- Micro-optimizations without measurement are guesswork
- Broad "this is faster" claims are hard to verify and revert
- One-at-a-time changes make bisect useful when something regresses

**Lesson:** Every performance commit should answer: *what specifically was slow, what specifically was changed, and how was it measured?* If you can't answer all three, don't commit it.

---

## 4. Architecture Evolves Through a Series of Instrumented Experiments

Big architectural shifts in all repos don't land as a single PR. They follow a pattern: prototype → isolate in a feature flag → gather data → roll out.

**Examples:**
- Sapling: NanoDag introduced as a new data structure, then wired into linelog, then cache-optimized, then tested — across ~10 commits
- Buck2: `StarlarkPagable` trait for out-of-core build graph paging — introduced as a trait, implemented incrementally across components
- Hermes: C++ test runner rewrite from Python — built with full feature parity to Python baseline before switching

**Why this matters:**
- Large architectural changes that aren't split are nearly impossible to review, revert, or bisect
- Keeping the old system running in parallel while the new one is built reduces risk
- Each step is independently verifiable

**Lesson:** When you know the architecture needs to change, design the migration as a series of staged changes where each stage is a valid, working state of the system.

---

## 5. Failure Is Normal; Reverts Are First-Class

Every mature repo has a pattern of reverting commits — and the revert discipline is precise.

**Examples:**
- hhvm: `Revert D98825163: table-based serialization support` — reverts reference the original Differential and commit, making the revert auditable and reversible
- pyrefly: Flaky tests are disabled immediately (`re-disabled a flaky test`), then re-enabled after the fix
- pyrefly: CRLF/LF cross-platform bug required adding platform-aware test coverage after the failure

**Why this matters:**
- A culture that reverts freely ships faster — engineers aren't afraid to try things because they know a revert is cheap
- Reverts that reference the original commit are easy to undo when the fix lands
- Post-mortems that include specific reproduction steps become regression tests

**Lesson:** Build the revert into the workflow. If a commit can't be easily reverted, that's a signal the change is too large or too coupled. Reverts should be a normal, boring part of development.

---

## 6. Tests Are Co-Located and Named After the Feature They Cover

Across all repos, tests live next to the code they test. Test file names describe the feature, not the testing framework.

**Examples:**
- pyrefly: `test/narrow.rs`, `test/calls.rs`, `test/generic_basic.rs` — named after feature areas
- pyrefly: Bug fix commits include a regression test in the same commit
- OXC: Generated linter runner tests auto-generated, not manually edited
- Sapling: Linelog tests co-located with the linelog implementation

**Why this matters:**
- When a test lives next to the code, engineers find it when they change the code
- When test names describe features (not "test_1", "test_2"), they serve as documentation
- Regression tests added alongside bug fixes prevent the same bug from returning

**Lesson:** The test-to-code ratio matters less than test-to-feature mapping. A test named `test_narrowing_for_subscript_symmetry` is worth more than 20 generic "test_type_checker.py" entries.

---

## 7. AI Assistance Is Disclosed, Not Hidden

HHVM had a commit explicitly authored by "Paladin Peel-the-Onion" with a human engineer (Gaetano Mendola) who reviewed, validated, and published it. The commit message explicitly notes this.

**Why this matters:**
- Disclosure builds trust with reviewers and future maintainers
- It creates an auditable chain: AI generated, human reviewed, human published
- It normalizes AI as a productivity tool without hiding the human accountability

**Lesson:** If you use AI to generate code, disclose it in the commit message. The question isn't whether to use AI — it's how to make the human accountability chain explicit.

---

## 8. Rust Projects Use Specific Idioms That Are Worth Emulating

**Pre-allocation over push-in-loop:**
```rust
// OXC pattern: carry stats upfront for pre-allocation
SemanticBuilder::with_stats // pre-reserves Vec capacity
Vec::with_capacity(n)       // when size is known
```

**Node-type restriction as perf lever (OXC pattern):**
```rust
// Only run a linter rule on the specific AstKind it needs
let AstKind::CallExpression(..) = node.kind() else { return };
```
Instead of running on all nodes and checking type at runtime.

**ArcStr for zero-copy string sharing across boundaries:**
```rust
// Rolldown/Buck2 pattern
ArcStr  // shared ownership, no clone on crossing JS-native boundary
```

**Once guards for initialization:**
```rust
std::sync::Once  // prevent double-init of runtime subsystems
```

**Lesson:** In large Rust codebases, the performance differences between "close to the metal" and "accidentally quadratic" come down to these patterns. Learn them before you need them.

---

## 9. Cross-Cutting Changes Require Cross-Layer Thinking

Several multi-commit sequences showed a pattern where a change needed to touch multiple layers simultaneously:

- pyrefly: TypedDict error messages required touching error display, context construction, expression handling, AND solve logic — all in one cross-cutting improvement
- Sapling: MERGE_RESOLUTION_OVERRIDE was threaded through 5+ layers — VFS, server, API, config, tests
- Buck2: `RelativePath` correctness refactor touched the type system, the evaluator, and every call site

**Why this matters:**
- Engineers who can hold the entire stack in their head are rare and expensive
- Cross-cutting changes that land in one PR are hard to review
- Changes that land across multiple PRs in layers need coordination

**Lesson:** For cross-cutting changes, write the architecture document first, then land each layer in dependency order, with integration tests at the boundary. Don't mix the architecture change with the implementation.

---

## 10. Contributor Concentration Is a Signal

In all repos, a small number of contributors drive the majority of commits:

- Sapling: Jun Wu drives ~50+ of every 400 commits — sustained infrastructure focus
- Wild: David Lattimore drives ~47% of commits — single-owner focus
- Buck2: Neil Mitchell drives cross-repo parity (Starlark bytes type, Rust patterns)
- Hermes: Rohan Patil and Gang Zhao drive runtime/VM; Aakash Patel drives type system

**Why this matters:**
- High contributor concentration is a bus-factor risk and a code-quality signal
- It often means one person owns an area that nobody else understands
- It's also a sign of deliberate specialization: "Rohan owns the VM" enables deep, sustained work

**Lesson:** Track contributor concentration per area. When one person owns >70% of commits in a critical subsystem, that's a risk to mitigate through documentation, code review ownership, and deliberate delegation — not a problem to celebrate.

---

## 11. Dependency Updates Are Bot-Automated and Batched

Across pyrefly, Sapling, Buck2: automated "Updating hashes" bot commits keep dependencies current without polluting the commit history with noise. The bot commits are clearly attributed and don't mix with human-authored changes.

**Why this matters:**
- Dependency drift is a security and compatibility risk
- If humans have to update deps manually, they won't do it often enough
- Automated batching keeps noise contained to a dedicated commit stream

**Lesson:** Automate dependency updates. Use a bot account, batch them, and separate them from the human-authored commit stream. Review the diffs separately from feature work.

---

## 12. Commit Hygiene: The Message Is the Contract

All repos show consistent commit message discipline:

- Conventional format: `type(scope): description (#PR)` — enables automated changelog generation
- PR numbers in messages enable traceability
- Detailed commit body with root cause analysis on complex fixes
- Cross-repo commits (Neil Mitchell across pyrefly/buck2) show up clearly via consistent authorship

**Why this matters:**
- When you're bisecting at 2am to find a regression, you read commit messages
- Messages that say "fix bug" or "update" are worthless
- Messages that say "fix: prevent exponential memory blowup in dict literal type inference by adding call-boundary context handling" are actionable

**Lesson:** A commit message should answer: what changed, why, and what was the trigger? If a future engineer reading only your commit message can't understand the context, the message is incomplete.

---

## 13. Scale Challenges Are Addressed Incrementally With Evidence

The specific scale challenges identified and addressed:

- **Exponential memory blowup in type inference** (pyrefly): Fixed with call-boundary context handling — a specific algorithmic fix, not a workaround
- **Incremental analysis for large codebases** (pyrefly): Module range computation moved to binding time; Final status change detection for incremental exports
- **Out-of-core build graph paging** (Buck2): `StarlarkPagable` trait — a specific architectural solution to a specific memory problem
- **DirEntryCache for module finder** (pyrefly): Directory lookup caching for large codebases

**Lesson:** Scale problems are not solved by "add more hardware." They're solved by understanding the specific data structure or algorithm that's O(n²) and fixing exactly that. Measure first, fix the specific bottleneck, measure again.

---

## 14. Experimental Components Are Kept Isolated Until Proven

pyrefly's `alt/` directory is a complete rewrite of the type checker, kept separate from the main implementation. Neil Mitchell's `box_patterns` removal touched both `alt/` and non-`alt/` code — the `alt/` path was becoming the default.

**Why this matters:**
- New implementations that share code with the old one get stuck in the old abstraction
- Isolated rewrites can evolve faster because they don't have to maintain compatibility
- Once proven, they can replace the old path incrementally

**Lesson:** If you're rewriting a core component, keep it isolated until it reaches feature parity. The integration surface is where rewrites die.

---

## 15. Breaking Changes Are Incremental and Flagged in Advance

No repo lands a breaking change without:

1. A feature flag that makes the new behavior opt-in
2. Release notes that describe the change
3. A migration path documented
4. An owner assigned for the migration

**Why this matters:**
- Large systems with many consumers (Buck2/Starlark, Hermes/JS engine, OXC/linter ecosystem) can't absorb sudden breaking changes
- Giving consumers a flag to test against before the breaking change lands prevents surprise
- Incremental breaking changes allow partial migrations

**Lesson:** In any project with external consumers, breaking changes should be planned across at least 2 releases: one that adds the flag, one that removes the old behavior. Never remove something in the same release you introduce it.

---

## Open Questions / Things to Investigate Further

1. **How does Sapling handle the Git/Mercurial interoperability edge cases?** The repo is Meta's bridge between internal Mononoke and external Git — the VFS work suggests ongoing security hardening
2. **How does Buck2's Starlark interpreter compare to Bazel's?** The `StarlarkPagable` architecture seems specifically designed to handle large build graphs — worth comparing
3. **How does OXC's linter codegen scale to 885 rules?** The narrow-runner pattern is interesting from a compiler engineering perspective
4. **What's the bus factor on the Wild WASM port?** David Lattimore at 47% contributor share is a concentration risk worth monitoring
