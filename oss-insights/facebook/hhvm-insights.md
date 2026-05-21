# HHVM / fbthrift OSS Insights

Analysis of facebook/hhvm commits from 2023–2025. This repo spans the HHVM runtime, the Hack language compiler/typechecker, and the fbthrift RPC framework. ~30,000 commits in this period.

---

## Major Themes

### 1. Whisker Codegen Migration (2024–2026)
The dominant architectural change: replacing `mstch` mustache-template-based codegen with `Whisker`, a structured-handle templating system. This plays out across dozens of commits.

**The situation before:** mstch templates operated on implicit context — templates received a blob of data and extracted what they needed via mustache expressions. This made templates fragile, hard to reason about, and difficult to optimize.

**The approach:** Whisker uses explicit structured handles. Code generation properties are explicit named values passed to templates, not implicit context extraction. Rust codegen properties migrated first (constant values, struct impl, union impl, enum templates, typedef templates, service functions), then C++ metadata codegen.

**The mechanism:** Each migration removes a `.mustache` file and replaces it with Whisker partials/properties. The final large cleanup removed `module_metadata.cpp.mustache` entirely — metadata generation moved to library code with no codegen needed.

**Scale implications:** Codegen is the multiplier for all Thrift/Hack development. When codegen is easier to reason about, every future change is cheaper. The team was willing to invest in dozens of migration commits because the long-term payoff is developer velocity across the entire codebase.

---

### 2. Performance Engineering — TDigest Quantile Estimation
A sustained optimization campaign around TDigest (percentile estimation), showing what micro-optimization looks like at scale.

**The situation:** TDigest is used for distributed percentile estimation. The merge path was repeatedly profiled and optimized across multiple commits.

**The approach:** Series of targeted improvements:
- `ff9dea18b7f` — Replace `std::sort` after merge with insertion sort as-you-go. Most arrays are already sorted; only perturbations need fixing. Avoids O(n log n) worst case.
- `da9b11fae0d` — Avoid heap allocation in TDigest merging by moving size dispatching outside `double_radix_sort()` and keeping small-array working buffer on stack.
- `38b919d66ac` — Replace intermediate centroids buffer + in-place merge with a streaming heap-based merge. Both faster and simpler.
- `80799536b38` — Deduplicate merge code between digest-to-digest and digest-to-values merging via a helper class.
- `709997c2715` — Add `folly::down_heap()` — a missing primitive for the merge pattern where you consume the top of a heap, modify it, and need to restore the heap invariant without a pop+push round-trip.

**What this teaches:** Performance work here is not one-shot — it's iterative refinement. Each commit makes the previous one look obviously suboptimal in hindsight. The team measures carefully (benchmark names, before/after nanoseconds) and the commit messages document the specific benchmark that motivated each change.

---

### 3. OpEncode Rollout — Serialization Performance
`op::encode` is a more maintainable, generically-defined serialization path that outperforms hand-generated code on most benchmarks.

**The situation:** Hand-generated per-struct serialization code was fast but expensive to maintain and optimize. Each struct had its own write path that couldn't benefit from shared infrastructure improvements.

**The approach (c6963985792):** Roll out `op::encode` for structs that don't use `deprecated_terse_writes` or `cpp.lazy`. The decision was data-driven: 58% of benchmarks faster, 33% slower, 9% neutral. The mean speedup was 3.8% for the winners. Biggest win: `write_BigListBigInt` at 47% faster.

**Complications — varint code bloat (e45193f9259b):** After rolling out inlining, disassembly analysis revealed that `Encode<i64_t>` (which expands to zigzag + BMI2 pdep varint encoding, ~80 bytes) was being inlined at every call site. For structs with many i64 fields, this caused icache pressure. Fix: add `NOINLINE` specifically on the i64 encode path. The data is precise: i16 and i32 benefit from full inlining (16-23% faster), but i64 suffers from it. Applying `NOINLINE` to i16/i32 caused a +79% regression — so the scope of the fix matters enormously.

**Force-inlining to close the gap (f66a2434c5f1):** Separately, the `for_each_field_id` lambda chain was NOT being inlined — 3 layers of wrapping caused ~18 cycles of overhead per field. Solution: replace lambdas with `FOLLY_ALWAYS_INLINE` functors throughout the call chain, enabling the compiler to inline everything into a monolithic function body.

**What this teaches:** Serialization is a hot path where small inlining decisions can mean 10-20% swings. The team uses disassembly analysis as a first-class tool, not just benchmarks. They also don't assume a fix works until measured — the NOINLINE decision required a full re-benchmark of all four integer sizes.

---

### 4. Security — Heap Buffer Overflow in THttpParser (8f8a66024617)
**The vulnerability:** `THttpParser.cpp` used a `uint32_t` buffer size that doubles on each reallocation. After 22 doublings from 1024 bytes, the value reaches `0x80000000`. The next doubling overflows to `0`, causing `realloc(ptr, 1)` to allocate 1 byte while `httpBufLen_` retains ~1.5GB. A malicious server could trigger a wild pointer write.

**The fix:** Add `httpBufSize_ > UINT32_MAX / 2` guard before each doubling operation. Both vulnerable sites (`THttpParser::getReadBuffer()` and `THttpTransport::refill()`) were fixed. The fix throws `TTransportException(CORRUPTED_DATA)` — matching the existing pattern.

**Notable:** This was flagged as an **AI-generated fix** in the commit message, with explicit "Do NOT ship without manual review" warning. The commit includes a three-dimensional confidence scoring (Security 95/100, Functionality 95/100, Performance 96/100) with composite 95/100 above a 75 threshold. The approach chosen was overflow guard over type widening because it was minimal and didn't change the class API.

**Scale implications:** Thrift HTTP transport is a network-facing attack surface. The fact that the overflow guard approach was chosen over `size_t` widening shows a preference for minimal changes over cleaner-but-larger refactors, even for security issues.

---

### 5. io_uring Support — Async I/O Modernization (2025–2026)
**The situation:** AsyncSocket had epoll-based I/O. io_uring provides a superior async I/O interface on Linux with better syscall batching.

**The approach (95081f5cff9, 2c3c06004434, cb8f9ef6c96c, d339af48e49d, a94129087211, and more):** Series of commits enabling native io_uring support in AsyncSocket, adding StopTLS support over io_uring, fixing StopTLS integration issues, and adding the IoUringZeroCopyBufferPool with a simple ringbuf. The work was incremental — each commit enabled a piece.

**Scale implications:** io_uring adoption is a multi-quarter project touching the core I/O path. The integration tests are the gating factor for rollout confidence.

---

### 6. Rust Codegen Infrastructure
**The situation:** Rust Thrift code generation used mstch templates with `rust_mstch_program` context properties.

**The approach (34ed5af4f51c, d453aedfe7f9, c03878ed062, bcad1ee1abe7, c5dd231238e2, fb6e890de1e, 895e1174dce4, 08430604efce, and many more):** Systematic migration to Whisker structured handles. Each commit migrates one template or property group. Commits frequently include "add missing moves" corrections and "avoid emitting trailing spaces" polish.

**What this teaches:** Large migrations are done commit-by-commit with each step being independently testable and reversible. The revert of one commit (e.g., D98264517) is always possible without destabilizing the whole effort. This is how you do risky refactoring in a high-velocity codebase.

---

### 7. Async/Signal Handling — Race Conditions and Lifecycle Bugs
**Example 1 (32fc11d173e):** Race condition where operations that never started (run() never called) could be cleaned up either normally or via `drain()` on shutdown. Neither path was aware of the other — whichever ran first caused a crash in the other. Fix: add checks to keep data structures in sync.

**Example 2 (47dcf85665f):** Server crash during `Sink onFinalResponse()` processing. The issue was `finalPayload.payload == nullptr` when compression was enabled. Fix: skip compression for null payloads.

**Example 3 (2b3557d5c787):** Generated client code called `releaseWriteHeaders()` which destructively moves headers out of RpcOptions. On retry, subsequent attempts had no headers. Fix: use `getWriteHeaders()` (non-destructive) instead.

**Scale implications:** Lifecycle bugs — objects that are partially constructed, or operations that never start, or resources that are double-freed — are the dominant failure mode at scale. The HHVM/thrift codebase has been running in production for years; these bugs survive because they require specific timing to trigger.

---

### 8. Named Arguments in Hack (81144c312ab9)
**The situation:** Named parameter *usage* (call sites) was gated behind a file-level attribute. This added friction for a feature whose attribute no longer served any purpose.

**The approach:** Remove the usage gating while keeping the attribute for function *definition*. The attribute was originally added out of caution that named arguments might not be handled correctly in some code paths — that concern proved unfounded.

**What this teaches:** Feature flags that were added as "temporary" safeguards sometimes outlive their purpose. The cost of keeping them is ongoing friction for users; the cost of removing them is a migration burden. Here, the team judged the migration burden was worth it for improved ergonomics.

---

### 9. Flaky Tests — A Persistent Background Task
Many commits are specifically about fixing flaky tests:
- `8cb3eec80e5` — `test_queue_timeout` flakiness due to 100ms queue timeout being too tight for 10,000 concurrent operations. Fix: increase to 1.0s.
- `8e8d7a4efd1` — `deadlock_detector` tests.
- `fb6e890de1e4` — `test_bidi_service_str_request_concurrently` flakiness due to queue timeout.
- `2e180feaa611` — Bidi concurrency test fixed by reducing load.

**What this teaches:** Flaky tests are never "done." They require ongoing attention, and the fixes are often just adjusting timeouts or reducing load — not deep bugs but environmental pressures that emerge at scale.

---

### 10. RNG Replacement — Xoshiro (56550e14405)
**The situation:** Folly used SFMT19937 as its random number generator.

**The approach:** Replace with Xoshiro256++, which is faster, higher quality, and more lightweight. Benchmark results showed clear improvement.

**What this teaches:** Even foundational primitives like RNG get revisited when better algorithms become available. This isn't emergency work — it's ongoing engineering investment in better primitives.

---

## Per-Commit Substance

### hhvm 858904dcff8 — Lower to ValCollection for vec literals
**Author:** Wilfred Hughes  
**Situation:** Literal `vec[$whatever]` had two different representations depending on code path — hh and hackc saw different IR for the same source. Naming-based conversion was fragile.
**Approach:** Lower directly to `aast.ValCollection` in the emitter, removing the naming-based indirection. This unifies the representation.
**Mechanism:** Changes in `emit_expression.rs` and `lowerer.rs` to construct `ValCollection` directly rather than relying on naming conventions.
**Scale implications:** Unifying the IR representation means future optimizations can apply uniformly. Bugs where one path saw different data than another are eliminated.

---

### fbthrift c6963985792 — Roll out op::encode where supported
**Author:** Shai Szulanski  
**Situation:** Hand-generated serialization was fast but unmaintainable. Generic `op::encode` was better but wasn't used everywhere due to missing `deprecated_terse_writes` and `cpp.lazy` support.
**Approach:** Data-driven rollout. Analyzed 67 benchmarks, found 58% improved, 33% regressed. Rolled out only where safe.
**Mechanism:** Feature flag in codegen: emit `op::encode` calls only for structs not using the two unsupported features.
**Scale implications:** Serialization touches every Thrift RPC. A 3-5% improvement here has outsized impact because it's paid on every request.

---

### fbthrift e45193f9259b — NOINLINE Encode<i64_t>
**Author:** Shai Szulanski  
**Situation:** After inlining rollout, disassembly showed `Encode<i64_t>` with BMI2 varint encoding (~80 bytes) was being duplicated at every call site, causing icache bloat. OpMixed::write was 965 bytes vs codegen's 742 bytes.
**Approach:** Targeted `NOINLINE` only on the i64 path after comprehensive benchmarking showed only i64 was hurt by inlining. i16/i32 actually benefit from inlining.
**Mechanism:** Added `FOLLY_NOINLINE` to `Encode<i64_t>`. Benchmarked all four integer sizes in two contexts to avoid overfitting to one benchmark.
**Scale implications:** This is the kind of optimization that only matters at very high throughput. At 1M requests/second, icache pressure from duplicated 80-byte sequences is measurable. At lower scale it's irrelevant.
**Cost:** Single type specialization requires tracking which types need it. If a new integer-like type is added, someone has to remember to check this.

---

### fbthrift f66a2434c5f1 — Force-inline for_each_field_id
**Author:** Shai Szulanski  
**Situation:** After StructEncode rollout, disassembly showed 3 layers of non-inlined lambda wrapping causing ~18 cycles overhead per field. For a 4-field struct, 4 explicit `call` instructions to lambda instantiations.
**Approach:** Replace all lambdas in the `for_each_field_id` call chain with `FOLLY_ALWAYS_INLINE` functors.
**Mechanism:** Changes in `Get.h` (shared infrastructure) and `Encode.h`. The fix lives once and benefits all callers.
**Scale implications:** Same pattern as the NOINLINE fix but in the opposite direction — this is about ensuring the compiler can inline through abstraction layers. The pattern of "lambda = hidden non-inlineable call" is a common performance trap in modern C++.

---

### fbthrift ff9dea18b7f — Optimize final sort in TDigest::merge
**Author:** Giuseppe Ottaviano  
**Situation:** TDigest merge produced sorted centroids but numerical errors could perturb the order, so `std::sort` was called as a fixup. `std::sort` doesn't special-case already-sorted arrays in libstdc++.
**Approach:** Do insertion sort as you go. Since perturbations are small and arrays are sorted >99% of the time, one comparison per step replaces O(n log n) sort.
**Mechanism:** During merge, maintain sorted order incrementally rather than fixing it at the end.
**Scale implications:** TDigest is used in distributed percentile estimation — every merge is on a hot path. O(n) instead of O(n log n) on the critical path.
**Cost:** More complex code with a data-dependent branch. Worth it only because it was measured to be faster.

---

### fbthrift da9b11fae0d — Avoid allocations in TDigest merging
**Author:** Giuseppe Ottaviano  
**Situation:** `double_radix_sort()` allocated a working buffer of 18kB for bucket counts, even for small arrays where stack allocation would suffice.
**Approach:** Move size dispatching outside `double_radix_sort()` so small arrays don't pay the allocation. Small arrays use a stack buffer.
**Mechanism:** The allocation was for the bucket counts, not the data. By checking size before entering the radix path, small arrays avoid it entirely.
**Scale implications:** TDigest merging happens continuously in production at Meta scale. Eliminating an 18kB allocation per merge is meaningful when merges are happening at high frequency.
**Cost:** The code path is now split — different code for small vs large. This adds complexity that must be maintained.

---

### fbthrift 38b919d66ac — Streaming heap merge for TDigest
**Author:** Giuseppe Ottaviano  
**Situation:** `merge()` accumulated all centroids into an intermediate array, then ran an in-place recursive merge sort. This required a large allocation and a relatively slow merge step.
**Approach:** Replace with a streaming merge using a heap. Merge two sorted lists by always pulling the smaller centroid from the top of a heap.
**Mechanism:** Classic textbook algorithm applied to the TDigest domain. Works because each input digest is already sorted.
**Scale implications:** Faster and simpler is rare in performance work. The fact that the streaming approach wins suggests the previous code was over-engineered for the problem.
**Cost:** The heap-based approach has O(n log k) complexity where k is the number of digests being merged — better than the allocation+sort approach.

---

### fbthrift 709997c2715 — folly::down_heap()
**Author:** Giuseppe Ottaviano  
**Situation:** Merging sorted sets using a heap requires popping the top, modifying it, and pushing it back. When the modified top is still the smallest (common when sets are skewed), this is wasteful — two heap operations when one would suffice.
**Approach:** Add `down_heap()` — the inverse of `sift_up`/`percolate_up`. After modifying the top element, call `down_heap()` to restore the heap invariant in O(log n).
**Mechanism:** New function in folly's heap utilities. Published as a general-purpose primitive.
**Scale implications:** Any merge-of-sets pattern benefits. TDigest was the motivation but the primitive is general.
**Cost:** New API surface. The team judged the narrow use-case worth it because the function is self-contained and the pattern recurs.

---

### fbthrift 8f8a66024617 — Fix heap_buffer_overflow in THttpParser
**Author:** Andrew Calvano  
**Situation:** uint32_t buffer size doubles on realloc. After 22 doublings from 1024, next doubling overflows to 0. realloc(ptr, 1) allocates 1 byte while code thinks it has ~1.5GB. Wild pointer allows remote code execution.
**Approach:** Add `httpBufSize_ > UINT32_MAX / 2` guard before doubling. Throws CORRUPTED_DATA. Chosen over type widening (size_t) because it doesn't change the API.
**Mechanism:** Two sites fixed: THttpParser::getReadBuffer() and THttpTransport::refill(). The refill() site had NO overflow protection at all.
**Scale implications:** Network-facing attack surface. A malicious server can exploit any Thrift client using HTTP transport.
**Cost:** Minimal — one comparison per realloc. The fix doesn't prevent the buffer from growing to 2GB, only from wrapping.

---

### fbthrift 95081f5cff97 — Enable AsyncSocket native io_uring support
**Author:** David Wei  
**Situation:** AsyncSocket used epoll. io_uring provides better syscall batching and lower latency on Linux.
**Approach:** Enable native io_uring support via a flag. Integration tests were the gating factor for rollout.
**Mechanism:** Add `AsyncSocket` native io_uring path, with support for zero-copy receive via IoUringZeroCopyBufferPool.
**Scale implications:** io_uring is a fundamental I/O improvement at the OS interface level. It benefits every async socket operation.
**Cost:** The complexity of supporting two I/O backends (epoll + io_uring) indefinitely.

---

### fbthrift 47dcf85665f — Fix crash during Sink onFinalResponse()
**Author:** Michal Kaczmarek  
**Situation:** Server crashes during `Sink onFinalResponse()` processing when compression is enabled and `finalPayload.payload == nullptr`. Reproducible with a specific exception-throwing test scenario.
**Approach:** Skip compression for null payloads. The crash occurs because compression code path doesn't handle null.
**Mechanism:** Guard added in Sink processing to check for null before invoking compression.
**Scale implications:** This is a deployment-time crash triggered by a specific combination of settings (CompressionConfig + exception in first response).
**Cost:** Simple null check with early return. Minimal but the bug had been causing production crashes.

---

### fbthrift 2b3557d5c787 — Use getWriteHeaders() not releaseWriteHeaders()
**Author:** Anish Aggarwal  
**Situation:** Generated client code called `releaseWriteHeaders()` which destructively moves headers out of RpcOptions. On retry with exponential backoff, the first attempt consumed headers and subsequent retries sent empty headers.
**Approach:** Change to `getWriteHeaders()` which returns a non-destructive copy.
**Mechanism:** One-line change in generated codegen template. Headers are now preserved across retry attempts.
**Scale implications:** Retry logic is critical for production reliability. Broken retry headers could cause silent failures where retries don't have the same context as the original request.
**Cost:** The template generates slightly more copy-on-write code. But correctness outweighs the minor overhead.

---

### fbthrift 56550e14405 — Xoshiro RNG in Folly
**Author:** Sadique Hussain  
**Situation:** SFMT19937 was the existing RNG. Xoshiro256++ is faster, higher quality, and more lightweight.
**Approach:** Implement Xoshiro256++ alongside SFMT19937, then replace usage.
**Mechanism:** New header `folly/random/xoshiro256pp.h` with the full implementation and tests.
**Scale implications:** RNG is used everywhere in the codebase. Better quality at better speed has broad impact.
**Cost:** New code to maintain, but it's a well-established algorithm.

---

### fbthrift 79e6a7f81280 — Reproducible floating-point summations
**Author:** Richard Barnes  
**Situation:** Floating-point summation is order-dependent. Different computation orders produce different results — a problem for reproducible builds and testing.
**Approach:** Implement ReproducibleFloatingAccumulator using binned floating-point arithmetic (adapted from ReproBLAS). Tunable accuracy via FOLD parameter.
**Mechanism:** Single-pass algorithm with 2*FOLD floats of memory overhead (FOLD=3 is sufficient for most cases). Handles NaN, infinity, overflow, underflow reproducibly.
**Scale implications:** Reproducibility matters for distributed computation where results from different machines must be combinable.
**Cost:** ~3x slower than naive summation for 1M elements, but Kahan compensation is 13x slower. The overhead is acceptable for the accuracy and reproducibility guarantees.

---

### fbthrift 32fc11d173e — Fix race condition in lifecycle
**Author:** Jay Edgar  
**Situation:** Operations that never started (run() never called) could be cleaned up two ways — normally or via drain() on shutdown. whichever ran first caused a crash in the other.
**Approach:** Add synchronization checks so both cleanup paths are aware of each other's state.
**Mechanism:** State tracking to ensure the cleanup path that wins is the only one that runs.
**Scale implications:** Race conditions in lifecycle management are particularly insidious because they require very specific timing to reproduce. They tend to appear in production under high load.
**Cost:** Added state and checks increase complexity of the shutdown path.

---

### fbthrift 1b4094364400 — Validate --config keys
**Author:** Max Heiber  
**Situation:** Invalid `--config` keys were silently ignored. Users could mistype a key and not realize their config wasn't taking effect.
**Approach:** Validate keys at parse time. On invalid keys, write to the hh client log file with a "did you mean" suggestion. Don't fail fast — invalid keys can happen during releases and rollouts.
**Mechanism:** Builds on ServerConfig validation infrastructure. Logs a warning rather than crashing.
**Scale implications:** Config mismatches between client and server versions can cause subtle bugs that are hard to diagnose. Surfacing them early helps.
**Cost:** The decision to log rather than fail fast reflects a philosophy: don't break production workflows for what might be a transient mismatch.

---

### fbthrift 81144c312ab9 — Allow named arguments without file-level attribute
**Author:** Max Heiber  
**Situation:** Named parameter *usage* was gated behind a file-level attribute. The attribute was added as a safety measure during rollout but no longer served a purpose.
**Approach:** Remove the usage gating. Keep the attribute for function *definition* (requiring it to declare named parameters).
**Mechanism:** Parser and type checker changes to allow named argument syntax everywhere, not just in attributed files.
**Scale implications:** Named parameters improve code readability and reduce bugs from argument order mistakes. Removing the gate makes them broadly accessible.
**Cost:** Some existing code that defined functions with named parameters may now be called differently. But usage was already gated so this is a liberalization, not a breaking change.

---

### fbthrift 699d85400ad6 — Table-based serialization support
**Author:** TJ Yin  
**Situation:** Existing serialization used a field-by-field approach. Table-based serialization uses a schema table for more efficient encoding.
**Approach:** Large multi-commit effort to add table-based serialization support. The commit messages note several reverts and re-lands, indicating this was a complex rollout.
**Mechanism:** Adds a new serialization path using schema tables. Rollout happened in multiple passes, with some reverts due to test failures.
**Scale implications:** Table-based serialization can significantly reduce serialization overhead for large structs. But the risk of rolling it out broadly caused the team to proceed cautiously with multiple reverts.
**Cost:** The number of reverts and "redo" commits in this series shows the cost of this kind of structural change — it touched many layers and required multiple iterations.

---

### fbthrift 72a50777001b — Remove metadata mustache codegen
**Author:** TJ Yin  
**Situation:** After schema-to-metadata was implemented as library code, the mustache-based codegen for metadata generation was no longer needed.
**Approach:** Delete `module_metadata.cpp.mustache` (261 lines removed) and related generated files. C++ adapter and aliasing metadata also removed.
**Mechanism:** Schema-to-metadata conversion is now purely library code. No template rendering required.
**Scale implications:** Removing codegen reduces the build pipeline. Less codegen = fewer failure modes and faster builds.
**Cost:** The deleted files had been maintained for years. The commit is small (3 lines changed showing deletions) but represents the culmination of a multi-year migration.

---

## Engineering Principles Observed

### 1. Measurement before optimization, again and again
Every performance commit includes specific benchmark results. The TDigest series has 5 commits, each refining the previous. The OpEncode rollout used 67 benchmarks to decide which structs to migrate. The NOINLINE fix benchmarked all four integer sizes in two contexts. The team doesn't guess — they measure.

### 2. Minimal API changes preferred over clean refactors
For the security fix (THttpParser), type widening to `size_t` was rejected in favor of an overflow guard because it didn't change the class API. The tradeoff: slightly uglier code vs. no API migration burden. The team consistently chooses less change over cleaner code.

### 3. Reverts are cheap and expected
Multiple commits in this period were reverts of previous changes (e.g., "Back out D84278317", "Back out D67605161"). The version control culture treats a revert as a normal tool, not an admission of failure. This enables more aggressive rollout of risky changes with the safety net of easy reversion.

### 4. Disassembly as a first-class tool
The team uses `objdump` / disassembly analysis to understand performance regressions. The NOINLINE fix and the force-inlining fix both started from disassembly inspection, not from benchmark scores alone. This is a high-skill, high-leverage practice that's rare outside of performance-critical codebases.

### 5. Flaky tests are a continuous tax
A surprising number of commits are fixing flaky tests. The fixes are usually simple (increase timeout, reduce load) but the underlying issue — that the test environment has resource pressure that doesn't exist in ideal conditions — never fully goes away. At scale, the test environment approaches production pressure, triggering failures that don't appear in development.

### 6. Codegen is leverage
The Whisker migration took dozens of commits and years of effort. But it pays off on every future change — when codegen is easier to understand and modify, every new feature or refactoring is cheaper. The team was willing to invest in this because they understood codegen as infrastructure, not just code.

### 7. Async I/O is a long-term bet
io_uring support was developed across multiple years with multiple commits. The integration surface is large (AsyncSocket, ClientFactory, zero-copy buffers, StopTLS). This kind of infrastructure investment doesn't pay off quickly but positions the codebase for better performance on modern Linux systems for years to come.

---

## Last Processed SHA
`d911b10bc976d0a931c2cc54314dc6a966d8a24a` (2026-04-12, most recent in analysis window)