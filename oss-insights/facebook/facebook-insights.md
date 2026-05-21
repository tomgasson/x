# Facebook Repos — OSS Insights

*Analysis based on last ~300 commits per repo, shallow cloned May 2026*

---

## facebook/hhvm

### Commit History Pattern

HHVM shows a mature, multi-language codebase (C++, Hack, PHP, Rust, Python) in active maintenance. The repo acts as a **monorepo bridge** — commits land first on internal fbcode, then get shipped to GitHub via an automated sync bot (Facebook GitHub Bot). This shows in:
- `fbshipit-source-id` trailers on nearly every commit
- `Differential Revision: D...` Phabricator links
- Facebook-internal email addresses on author fields

### Key Engineering Insights

**1. Rust Migration is Active and Ongoing**
Several recent commits by Harsh Chokshi (`hchok@meta.com`) are migrating HHVM's Thrift codegen templates from legacy mstch to a new Whisker templating system. This is a multi-commit, multi-phase migration:
- `Migrate Rust const codegen to Whisker` — template file deletions and rewrites
- `Extract Rust RPC helper macros into partials` — extracting macro call sites into explicit partial calls
- `Migrate structimpl template to Whisker structured handles`
- `Migrate Rust serialized value read/write helpers to Whisker partials`

Pattern: Each commit is self-contained and reviewed by `dtolnay` (David Tolnay), Meta's Rust lead. The migrations are done incrementally — old files deleted only after all call sites are updated.

**2. AI-Assisted Coding is Being Reviewed and Merged**
One commit (`Eliminate temporary std::string allocations in RegexMatchCache`) was explicitly authored by an AI tool called "Paladin Peel-the-Onion", with a human engineer (Gaetano Mendola) reviewing and publishing it. The commit message explicitly notes: *"NOTE: A human engineer reviewed, validated, and published this diff."* This is a transparent pattern — not hiding AI assistance, but making the human review chain explicit.

**3. Performance Work is Precise and Measured**
Performance commits are specific and narrow:
- `Eliminate temporary std::string allocations in RegexMatchCache` — targets a hot path, uses Paladin Rule 22 (unnecessary allocations) as the trigger
- `Use PrivacyLib propagation data in ThriftServiceMethodNameVirtualPolicyEnforcer` — gated behind killswitches, uses profiling data

**4. Deprecation is Systematic**
Flags are removed only after confirmed deployment:
- `Remove enable_abstract_method_optional_parameters flag` — "This feature has been deployed for over a year now"
- This shows a culture of feature flags with hard removal timelines, not open-ended experiments

**5. Revert Discipline is Strong**
Reverts reference the original Differential and commit:
- `Revert D98825163: table-based serialization support` — references original Phabricator diff, provides original commit changeset
- This makes the revert auditable and reversible

### Architectural Decisions

- **Thrift-first design**: Many recent changes center on Thrift serialization performance and correctness
- **Oxidized components**: Hack typechecker components being rewritten in Rust (the `oxidized/` directory)
- **killswitch-gated experiments**: New features land behind killswitches (`PLKS::THRIFT_CONTEXT_PROP_FROM_PRIVACYLIB`) before full rollout

### Refactor Patterns

- Whisker templating migration (C++ mstch → Rust-native Whisker partials)
- Static Initialization Order Fiasco (SIOF) fixes — `get_name<T, Id>()` function templates replacing fragile inline variables
- Provenance tracking through UNSAFE_CAST using flow analysis (O(1) instead of walking the reason graph)

### Test Strategy

- Tests are co-located with source (`.php.exp`, `.hhvm` test fixtures)
- Extended reason test suites with explicit `.exp` files for expected output regression testing

### Failure & Recovery

- SIOF crashes manifest as SEGV during static initialization before `main()` — caught via special `get_name<T>()` on-demand computation
- Facebook GitHub Bot commits are automated sync points, not code changes

---

## facebook/hermes

### Commit History Pattern

Hermes is Meta's JavaScript engine, and the recent commits show **two distinct streams**:

1. **Rohan Patil and Gang Zhao** — C++ runtime/VM optimizations
2. **Aakash Patel** — TypedLib/Flow type system work

The repo also ships to npm (hermes-engine, hermes-parser) and has OSS/Internal parity concerns.

### Key Engineering Insights

**1. Inline Fast Paths for Hot Operations**
Multiple optimizations by Rohan Patil focus on:
- `Optimize put-by-val for numeric keys` — codegen emits narrowest specialized path, inline f64→u32 conversion
- `Add inline fast path for strict equality (===)` — same-object-bits check, NaN handling, +0/-0 correctness
- Pattern: Start with out-of-line helper, then inline the hot path when profiling data supports it

**2. C++ Test Runner Parallelization**
Gang Zhao has been building a C++ test runner that replicates the Python runner's behavior with 8x speedup:
- Lock-free results via pre-allocated slots (no mutex, each worker writes to unique index)
- `sigsetjmp/siglongjmp` crash guard for in-process bytecode execution — catches SIGSEGV/SIGABRT and converts to test failure
- `--shermes` subprocess execution mode, `--jit`/`--lazy`/ `-O` flags matching Python runner
- Reuses `RuntimeFlags.h` instead of duplicating flags

This is significant: a rewrite in C++ for speed, with full feature parity to the Python baseline.

**3. TypedLib: Building a Type-Safe Standard Library**
Aakash Patel is building `lib/TypedLib/` — typed versions of JS builtins:
- `Array.length` as a getter on `Array<T>` instead of FlowChecker special case
- `shift/unshift/splice/toSpliced` implementations
- `map/set` size implementations
- `Typed: Array destructuring with rest elements` — Flow type system extension for `...rest` binding
- Pattern: Each builtin lands as a separate commit, tests in `test/hermes/flow/` directory

**4. Typed Mode is a First-Class Feature**
Typed mode gets dedicated treatment:
- `FlowChecker: Allow omitting arguments when 'void' is allowed`
- `FlowChecker: Typecheck 'arguments'` — restricts `arguments` to `arguments.length` in typed functions
- `Typecheck fn.call() like $SHBuiltin.call` — mirrors validation logic
- `Support overload on static methods` — extends existing overload infrastructure

### Architectural Decisions

- **Typed vs Untyped split**: TypedLib and typed mode represent a distinct execution path, not just annotations
- **RAII helpers for threading**: `ProgressReporter` as polling helper owning its own thread
- **Crash isolation**: siglongjmp approach lets test runner survive crashes in individual tests

### Performance Approach

- Codegen specialization for common cases (numeric keys, strict equality)
- Pre-allocation + unique-index writing instead of mutex-protected vector append
- subprocess compilation mode for sandboxed execution

### Commit Hygiene

- Commits by the same author (`Gang Zhao`) are often grouped in the log — suggesting stacked diffs or batch commits
- Differential Revisions link every non-bot commit to internal review
- Design documents (`DESIGN.md`) added when significant architectural work lands

---

## facebook/flow

### Commit History Pattern

Flow's recent commits show **heavy investment in the Rust port** (`rust_port/` directory) alongside continued TypeScript compatibility work. The OSS version on GitHub lags slightly behind internal deployments (`.flowconfig` updates with version bumps show internal-to-OSS sync points).

### Key Engineering Insights

**1. Rust Port is Accelerating**
Multiple commits show active Rust port work:
- `TCP_NODELAY for Flow Rust port local transports` — 1-3ms steady telemetry vs ~40ms floor before
- `[flow][oxidation] LSP related mass fixes` — async file watcher notifications, scheduling fixes, gc removal
- `Fix silent stale-mergebase bug after EdenFS LostChanges` — EdenFS-specific edge case
- Deps updates in `rust_port/Cargo.lock` for `ctor`, `static_interner`

**2. TypeScript Compatibility is a Priority**
George Zahariev (`gkz@meta.com`) is the primary driver:
- `[flow][tslib] Support \`this\` type in interfaces` — complex rebinding through `extends`/`implements`/generics
- `[flow][tslib] Support mapped-type key remapping (\`as\` clause)` — `{[K in Source as NewKey]: Value}`
- `[flow][tslib] Support optionality removal (\`-?\`)` in mapped types
- `[flow][tslib] Add node_modules/@types/ module resolution fallback`
- `[flow][tslib] Support declare methods in class bodies for ambient contexts`

Each of these is a multi-month language feature being implemented for TS parity.

**3. Test Infrastructure Modernization**
- `declare var` → `declare const/let` mechanical sweep in `newtests/`
- Snapshots re-recorded after mechanical changes
- Integration tests using `.input`/`.expected` file pairs in `rust_port/crates/flow_server_monitor/`

**4. Internal Deployment Sync Points**
Flow versions are bumped and synced:
- `Deploy 0.314.0 to xplat`
- `Deploy 0.313.0 to xplat`
- `Deploy 0.312.1 to xplat`
- Internal version numbers (`0.314.0`) higher than OSS (`0.72.0`) — large internal/external version divergence

### Architectural Decisions

- **EdenFS-aware file watching** — special handling for `hg.transaction` events that get coalesced by Watchman
- **Third-party Rust deps managed separately** — `dtolnay` reviews third-party bumps but internal `static_interner` upgrades are done by `Neil Mitchell`
- **Mapped types with modifiers**: `-readonly`, `+readonly`, `-?`, `+?` all behind `experimental.tslib_syntax` flag

### Refactor Patterns

- `rust_port/` code is being mass-fixed in single commits ("LSP related mass fixes")
- Libdef modernization: `$NonMaybeType<T>` → `NonNullable<T>` etc. in `core.js` and `react.js`

---

## facebook/relay

### Commit History Pattern

Relay is Meta's GraphQL client. Recent work shows **active Rust compiler development** alongside React Server Components (RSC) experimentation. The repo has strong release discipline with explicit version bumps and blog post changelogs.

### Key Engineering Insights

**1. RSC (React Server Components) is a Major Focus**
Alison Lee (`alis0n@meta.com`) is building RSC support:
- `Experimental serverFetchQuery for data fetching in RSC` — `createServerEnvironment` wrapper with React.cache()
- `Add serverReadFragment to rsc_EXPERIMENTAL` — server-side equivalent of useFragment
- `Add serverPreloadQuery and useQueryFromServer to rsc_EXPERIMENTAL` — server-side preload + client hydration
- Pattern: server executes query synchronously, returns PreloadedQueryRef with Promise inside, client uses `use()` to unwrap

**2. Rust Compiler is Mature and Active**
The Rust compiler (`compiler/crates/relay-compiler/`) has substantial recent work:
- Watchman integration: `Use watchman clock to synchronize file subscription and write events`
- `Detect external modifications to generated artifacts`
- `Surface compiler crashes to meerkat clients`
- `Add incremental compilation test: subscription root type rename`
- `Fix clippy::unnecessary_sort_by` by generatedunixname author

**3. SchemaSet Architecture Work**
Matt Mahoney (`mmahoney@meta.com`) has done substantial schema modeling work:
- `Split location-based info from is-present/extension info` — orthogonal concerns
- `Clarify how to construct SchemaSet from base + extension SDL sources` — API refactor
- `Round-trip extension-only types as \`extend X { ... }\` instead of \`X { ... }\``
- `Regression test + fix for mismatched coordinate merging`

**4. Dependency Update Automation**
- `facebook-github-bot` creates automated Cargo.lock updates via `create-pull-request` GitHub action
- `dependabot[bot]` handles npm dependency updates for website/
- `dtolnay` handles Rust dependency updates (`lsp-types`, `ctor`)

**5. CI Hygiene**
- Explicit handling of orphaned generated artifacts when tests are deleted
- "Check working directory status" step catches uncommitted generated file removals

### Architectural Decisions

- **Schema coordinate-based tracking** — types tracked by their coordinate (location + definition), enabling precise round-tripping
- **RSC uses `use()` from React 19** — asynchronousunwrap pattern, staleness threshold (default 30s) for client refetch
- **Incremental compilation** — watchman clock-based synchronization, artifact writer with external modification detection

### Breaking Changes Handling

- Relay 21 blog post documents all breaking changes for upgrade path
- Version bumps in Cargo.toml and package.json together with yarn.lock
- `serverPreloadQuery` uses synchronous execution to prevent accidentally blocking page rendering

---

## Cross-cutting: Facebook Engineering Culture

### 1. Differential First, GitHub Second
Every meaningful commit links to a Phabricator Differential revision. The GitHub commit is the *output* of an internal review process, not the origin. This means:
- Internal review discipline is high — changes are discussed in Differential before shipping
- The `fbshipit-source-id` maps GitHub commits back to the original fbcode commit
- Automation (Facebook GitHub Bot) handles the sync, not humans manually re-writing history

### 2. Feature Flags with Hard Removal Timelines
Flags are not permanent abstractions:
- Flags ship with a "deployed for over a year" removal policy
- Multiple feature flags are actively being removed in recent commits
- This prevents flag debt accumulation

### 3. Bot Authors for Mechanical Updates
- `facebook-github-bot` handles Cargo.lock, npm lockfile updates
- `dependabot[bot]` handles OSS dependency updates
- `generatedunixname*` authors handle internal mechanical changes (version bumps, etc.)
- This frees engineers to focus on meaningful changes

### 4. AI-Assisted Coding with Explicit Human Review
At least one repo (hhvm) shows explicit AI authorship with human review and publication. The pattern:
- AI tool generates draft
- Human engineer reviews, validates, publishes
- The human review chain is documented in the commit message
- This is framed as "human in the loop" not "AI replacement"

### 5. Rust as the Preferred Systems Language
Across all four repos, Rust is being used for:
- HHVM: Thrift codegen templating, oxidized Hack compiler components
- Flow: Rust port of the typechecker and server monitor
- Relay: GraphQL compiler rewrite from OCaml/JS to Rust

The `dtolnay` (David Tolnay) author appears across repos as reviewer for Rust changes — suggesting centralized Rust leadership.

### 6. Performance Optimization is Empirical
Performance work is driven by measurement, not speculation:
- Hot path identification (RegexMatchCache allocations, put-by-val for numeric keys)
- Explicit performance claims in commit messages ("8x speedup over Python runner")
- New features (inline fast paths) follow the pattern: out-of-line → profile → inline the hot case

### 7. TypeScript Compatibility as Competitive Feature
Flow and Hermes both invest heavily in TS compatibility:
- Mapped type modifiers (`as`, `-?`, `-readonly`)
- `this` type in interfaces
- `declare methods` in ambient class bodies
- This is framed as keeping Meta's tools competitive with TS ecosystem

### 8. Oxidation (OCaml→Rust) as Long-Term Trend
Flow's Rust port is the most visible example, but HHVM's `oxidized/` directory shows the same pattern:
- Incremental rewrites, not big-bang migrations
- Rust code calls OCaml code where not yet ported
- Integration tests verify behavior matches original

### 9. Testing is Co-located and Regression-Focused
- Test fixtures next to source files (`.php.exp`, `.hhvm`)
- Snapshot-based regression testing for expected outputs
- Explicit regression tests added alongside bug fixes

### 10. Internal Version > OSS Version
Flow shows this clearly (internal 0.314.0 vs OSS 0.72.0). The repos are maintained internally first and exported to GitHub, not the reverse. The OSS version is a subset, not a superset.

---

*Generated by OSS archaeology sub-agent. Repos cloned May 2026. Shallow clone (500 commits) — earlier history not analyzed.*