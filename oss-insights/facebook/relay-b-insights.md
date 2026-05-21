# Relay — Group B: RSC / Server Components

## Alison Lee — RSC Entry Points (server-side preloading + hydration)

### facebook/relay c26dc44a — Add serverPreloadQuery and useQueryFromServer to rsc_EXPERIMENTAL
**Author:** Alison Lee <alis0n@meta.com>
**Situation:** RSC apps needed a path for server-fetched data to land in the Relay store on the client without a round-trip query.
**Approach:** Synchronous `serverPreloadQuery` returns a `PreloadedQueryRef` with data as a Promise — callers can't await it (prevents accidental RSC render blocking). Client `useQueryFromServer` uses React 19's `use()` to unwrap, publishing to store without notifying subscribers (avoids state-update-during-render errors).
**Mechanism:** Staleness threshold (default 30s) triggers refetch if server data too old. `defaultGetDataID` exported via `relay-runtime.__internal` for normalization without reaching private fields.
**Scale implications:** Server/client split requires careful staleness semantics — too stale defeats purpose, too fresh creates races with concurrent mutations.
**Cost:** Experimental surface — must be carefully backwards-compatible as RSC adoption grows.

---

### facebook/relay 510cf1ea — Add serverReadFragment to rsc_EXPERIMENTAL
**Author:** Alison Lee <alis0n@meta.com>
**Situation:** RSC needed a server-side equivalent of `useFragment` — reading fragment data from the store populated by a prior server-side fetch.
**Approach:** `serverReadFragment` delegates to `waitForFragmentData`. Available as a pre-bound method on `ServerEnvironment` returned by `createServerEnvironment`.
**Mechanism:** Reading complement to writing (`serverPreloadQuery`). RSC data flow: server preload → server render → client hydration.
**Scale implications:** Completes the server/client data symmetry for RSC use cases.
**Cost:** Experimental. Depends on React 19 `cache()` semantics.

---

### facebook/relay 8a2fff47 — Experimental serverFetchQuery for data fetching in RSC
**Author:** Alison Lee <alis0n@meta.com>
**Situation:** React Server Components needed to execute GraphQL from the server without a client round-trip (which defeats RSC purpose).
**Approach:** `createServerEnvironment` wraps an Environment factory in `React.cache()` for per-request isolation. Returns `ServerEnvironment` with pre-bound `serverFetchQuery` — no prop threading needed.
**Mechanism:** `React.cache()` ensures each server request gets its own isolated environment. Pre-bound methods eliminate environment prop drilling in deeply nested RSC trees.
**Scale implications:** Per-request isolation is critical for multi-user server environments.
**Cost:** Experimental. React 19 dependency.

---

### facebook/relay 8e86732f — Expose ToSetDefinition trait and set_type_from_definition
**Author:** Janette Cheng <jcheng@meta.com>
**Situation:** SchemaSet's internal type representation needed to be accessible outside the module for derived schema operations.
**Approach:** Expose `ToSetDefinition` trait and `set_type_from_definition` in the public API.
**Mechanism:** Trait-based conversion between schema representations.
**Scale implications:** Enables downstream libraries to build derived schema operations without reimplementing conversion logic.
**Cost:** Adds API surface — must be maintained going forward.

---

## Tianyu Yao — @defer/@stream and Server-to-Client (S2C)

### facebook/relay 75207873 — Phase 1+2: Hoist normalization to network layer
**Author:** Tianyu Yao <skyyao@meta.com>
**Situation:** `@defer`/`@stream` required normalization engine to handle incremental response chunks. Existing architecture assumed single-shot responses.
**Approach:** Hoist normalization to the network layer so it processes streaming responses as they arrive.
**Mechanism:** Network layer passes partial responses to normalization engine progressively, rather than buffering the complete response first.
**Scale implications:** Streaming GraphQL enables large payloads without single-response limits. Critical for live queries and large datasets.
**Cost:** Significant refactor of the network-to-store pipeline. Every boundary change risks regressions.

---

### facebook/relay e4f7cf19 — Add S2C execution to Scope
**Author:** Tianyu Yao <skyyao@meta.com>
**Situation:** S2C (Server-to-Client) resolvers needed execution support within the scope abstraction. Results needed special handling at normalization time to correctly reconstruct client-side data.
**Approach:** Add `handleS2CExecutions`, `readInto`, and `createRootFragmentNode` to scope.
**Mechanism:** S2C resolvers execute on server; their serialized results flow through the GraphQL response and need correct routing at normalization time.
**Scale implications:** S2C enables server-side field resolution with client-side caching — avoids N+1 without sacrificing Relay's store model.
**Cost:** Adds complexity to scope abstraction. Must track which data came from S2C vs regular resolution.

---

### facebook/relay f61d5575 — Phase 3: @defer/@stream support in NormalizationEngine
**Author:** Tianyu Yao <skyyao@meta.com>
**Situation:** Phase 1+2 laid the infrastructure; Phase 3 completes NormalizationEngine support — chunk handling, cursor management, fragment boundary reconciliation.
**Approach:** Complete the full lifecycle for deferred/streaming responses.
**Mechanism:** Handles boundary conditions: late-arriving data, fragment completion tracking, cursor advancement.
**Scale implications:** Makes `@defer` usable in production — without correct normalization, deferred fragments crash or silently drop data.
**Cost:** High. NormalizationEngine is one of Relay's most critical paths. Every query is affected.

---

### facebook/relay 49b74be8 — Emit has_s2c_resolvers on operations with S2C resolvers
**Author:** Tianyu Yao <skyyao@meta.com>
**Situation:** Runtime needed to know whether an operation contains S2C resolvers to apply the correct execution path.
**Approach:** Compiler detects S2C resolver fields and emits a `has_s2c_resolvers` flag during code generation. Runtime checks the flag to determine execution strategy.
**Mechanism:** Codegen-level flag emission, runtime flag check.
**Scale implications:** Correct flag emission is critical — wrong flag = skipped data or wrong execution path.
**Cost:** Codegen concern. Schema changes that introduce S2C resolvers must correctly update the flag.

---

### facebook/relay 10172f1f — Don't emit has_s2c_resolvers flag for Query-rooted resolvers
**Author:** Tianyu Yao <skyyao@meta.com>
**Situation:** S2C resolvers rooted at Query type don't need the flag — they go through the standard server path and don't require special client routing.
**Approach:** Exclude Query-rooted S2C resolvers from flag emission.
**Mechanism:** Codegen filter skips S2C resolvers where parent type is Query.
**Scale implications:** Prevents unnecessary flag on common cases, avoids runtime path confusion.
**Cost:** Codegen change only — lower risk than runtime changes.
