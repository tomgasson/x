# Rust Rules of the Road

A curated guide to writing Rust that is correct, fast, maintainable, and that you won't regret in six months.

---

## 1. Threading

**Ownership is your friend.**
- Rust's type system makes data races a compile error, not a runtime surprise. Lean into this.
- `Arc<T>` for shared ownership across threads. `Arc<Mutex<T>>` (or `RwLock`) when you need interior mutability.
- Avoid `Mutex` unless you must; `RwLock` for read-heavy workloads.

**Prefer message passing over shared state.**
- Channels (`std::sync::mpsc`, `crossbeam-channel`) for communication between threads.
- Keep the "share memory by communicating" philosophy: one thread owns the data, others borrow via messages.

**Spawning**
- Use `spawn` for fire-and-forget. Use `JoinHandle` if you need to wait for completion or propagate panics.
- Never leak `JoinHandle` without calling `.detach()` or `.wait()` — goroutines (threads) will be dropped silently.

**Thread locals**
- `thread_local!` for per-thread global state. Use sparingly — it makes reasoning about state harder.
- Consider whether you actually want a dedicated thread (e.g., a background worker) instead.

**Stack sizes**
- Default stack is large enough for most things. If you're spawning thousands of threads, consider `--threadstack` or use a thread pool instead.

---

## 2. Concurrency

**Parallelism ≠ Concurrency**
- Parallelism: divide work across cores (`rayon`, `tokio::task::spawn_blocking`).
- Concurrency: manage overlapping tasks (async/await, channels).
- Don't reach for threads when async is clearer.

**Async Rust (`tokio`, `async-std`)**
- `.await` inside `async fn` only — never block inside an async context.
- Use `tokio::task::spawn_blocking` to call blocking sync code (DB drivers, file I/O) from async.
- Keep the async executor moving — don't hold a future across `.await` points if you don't need to.

**Backpressure**
- Always think about what happens when a channel fills up, a queue overflows, or work arrives faster than you can process.
- Use bounded channels and handle `.send()` failures explicitly.

**Shutting down cleanly**
- Use `broadcast` or `CancellationToken` to signal shutdown across tasks.
- Don't just drop tasks — drain your queues, finish in-flight work, then exit.

---

## 3. Performance

**Profile before you optimize.**
- `cargo flamegraph` (or `perf` + `inferno`) to find real bottlenecks.
- `cargo bench` with criterion for microbenchmarks.
- Look at hot paths in I/O, allocation, and lock contention — not algorithm clever-ness.

**Avoiding allocations in hot paths**
- Stack-allocate where possible: small fixed-size buffers, stack arrays.
- Use `SmallString` / `SmallVec` (from `smallvec` crate) to avoid heap allocation for small workloads.
- Reuse buffers — `object_pool` patterns, or a simple `Vec` with `.clear()` + `.reserve()`.
- `String` vs `&str`: pass `&str` across function boundaries unless you need ownership.

**Iterators over loops**
- Prefer iterator chains (`.map().filter().collect()`) — the compiler can vectorize and unroll them better than manual index loops.
- But don't over-chained — readability matters.

** SIMD and CPU-specific intrinsics**
- Only for hot paths measured with profiler evidence. `std::simd` (Nightly) or `packed_simd` crate.
- Usually the bottleneck is algorithmic, not SIMD-level.

**Know your `no_std` options**
- `heapless` crate for no-std environments — collections without allocator.
- `staticvec` for stack-backed arrays with compile-time size limits.

---

## 4. Memory (Alloc, Free, etc.)

**The allocator is configurable.**
- `#[global_allocator]` lets you swap in `jemallocator`, `mimalloc`, or a custom allocator.
- `jemalloc` often wins for multi-threaded workloads with many allocations.
- Test with `mimalloc` incontainerised environments — better cache locality.

**Box, Rc, Arc — choose precisely.**
- `Box<T>` — sole ownership, heap-allocated, cheap to move.
- `Rc<T>` — single-threaded reference counting. Use only when you need shared ownership without threads.
- `Arc<T>` — thread-safe shared ownership. Add `Mutex` or `RwLock` for interior mutability.
- Never mix `Rc` and `Arc` in the same codebase unless you have a very specific reason.

**Leak an allocation only intentionally.**
- `Box::leak` for long-lived globals that never need to be freed (e.g., a compile-time string pool).
- `std::mem::forget` — usually wrong; use `ManuallyDrop` if you need to prevent drop.

**Avoid `unsafe` unless you need it.**
- When you do use `unsafe`, wrap it in a safe abstraction immediately. The unsafe block should be as small as possible.
- Use `unsafe` for: interfacing with hardware, building custom reference-counted types, FFI, zero-copy parsing with lifetime guarantees.
- Document why the unsafe block is safe with a comment referencing invariants.

**Stacks over heaps when possible.**
- Rust makes this easy: locals are stack-allocated by default.
- `const` data is stored in the binary's read-only section — no allocation at all.

**Drop order matters.**
- Struct fields drop in declaration order — not reverse order like C++.
- `ManuallyDrop` to control when something drops (important in FFI, async cleanup).

---

## 5. Data Structures

**Use the right collection.**
- `Vec<T>` — dynamic array, your default most of the time.
- `VecDeque<T>` — double-ended queue, for ring-buffer patterns and BFS.
- `LinkedList<T>` — rarely needed; worse cache locality than Vec.
- `HashMap<K, V>` / `BTreeMap<K, V>` — map types. `BTreeMap` for ordered iteration, `HashMap` for key lookup speed.
- `HashSet<T>` / `BTreeSet<T>` — set types.
- `SmallVec<[T; N]>` — stack-allocated inline storage for small collections.
- `IndexMap<K, V>` — map that preserves insertion order (like Python dict).

**Struct layout for cache efficiency.**
- Put fields of the same size together (`u64`, `u64`, then `u32`, `u32`).
- `#[repr(C)]` for FFI, `#[repr(Rust)]` for idiomatic layout.
- `#[repr(packed)]` — use only when you have a hard requirement; it disables alignment optimizations.

**Derive thoughtfully.**
- `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]` — these are free and useful.
- `Debug` is always worth deriving (or manually implementing) — don't ship `{:?}` with a placeholder.
- Never `Clone` unless you mean it — it signals "I actually need a copy."

**Avoid `enum` with data in niche positions.**
- `Option<&T>` is as efficient as `&T` with a null pointer niche — the compiler uses the null bit pattern.
- Niche layout optimization is great when you know it applies; don't force it.

---

## 6. Alignment

**Rust aligns struct fields by default — trust it.**
- The default alignment rules are good for most workloads.
- Only reach for `#[repr(packed)]` or manual padding when you have measured evidence.

**Explicit padding.**
- When you need precise control (serialization formats, FFI), use `#[repr(C)]` structs with explicit padding fields (`_pad: [u8; N]`).
- Name padding fields with `_pad` or `_reserved` prefix — the underscore suppresses unused warnings.

**Memory layout for SIMD.**
- Arrays of struct (`AoS`) vs struct of array (`SoA`) — for vectorized code, `SoA` is almost always faster.
- Use `bytemuck` for safe transmutation between types when doing zero-copy reinterpreting.

**Alignment of slices.**
- `std::alloc::alloc` with specific alignment requirements when you need aligned allocations (SIMD loads/stores).
- `align_to` on slices to safely read misaligned data without UB.

---

## 7. Logging

**Structured logging — always.**
- `tracing` (and `tracing-subscriber`) for the modern Rust ecosystem — `log` facade if you need compatibility.
- Log levels: `ERROR` / `WARN` / `INFO` / `DEBUG` / `TRACE`. Use them consistently.

**Log the right things.**
- At `INFO`: significant state transitions, startup, shutdown, job completion.
- At `DEBUG`: entering/leaving important functions, loop iteration counts, queue depths.
- At `TRACE`: raw data dumps, every message sent/received.
- At `ERROR`/`WARN`: anything that is wrong but the program can continue.

**Context is everything.**
- Include `request_id`, `user_id`, `session_id` in logs so you can correlate.
- Use `tracing::info_span` to attach structured context to spans.
- Never log secrets, passwords, tokens, or PII.

**Output.**
- JSON logs for machine parsing (`tracing-subscriber` with `fmt::json`).
- Pretty logs (`FmtTelemetry::pretty()`) for local development.
- Never use `println!` in library code — use the `log` crate facade so callers control the backend.

**Rate limiting.**
- If a log line fires on every request at 10k qps, it will destroy your disks.
- Use `tracing` rate limiting or a sampling approach for hot paths.

---

## 8. Error Handling

**`Result<T, E>` for fallible operations. Always.**
- Never swallow errors with `unwrap()` in production code paths.
- `?` operator to propagate — use it liberally.
- Return `Result` from `fn` when the caller needs to decide what to do on failure.

**Error types.**
- Use the `thiserror` crate to define structured errors that implement `std::error::Error`.
- One error type per subsystem — don't proliferate tiny error enums.
- Wrap lower-level errors with `.context()` (from `anyhow` or `thiserror`).

**`anyhow` vs `thiserror`.**
- `thiserror`: when you're defining the error type for a library or a well-scoped subsystem.
- `anyhow`: in application code where you just need to propagate any error without careful matching.
- Don't mix both in the same crate — pick one and be consistent.

**Panic only for programmer errors.**
- `panic!` for "this should never happen" — invalid input, violated invariants.
- Use `unwrap()` only in tests, or where the context makes failure impossible (constant evaluation).
- `expect()` in tests is fine — it documents assumptions.

**Backtraces.**
- `RUST_BACKTRACE=1` in production — capture backtraces on error paths.
- Log the full error chain with `e.chain()` (`anyhow`).

**No errors as control flow.**
- Don't use `Result` to implement early return — that's what `?` is for.
- Don't use `panic` for validation — return `Result` and let the caller decide.

---

## 9. Program Structure

**Module system is your friend.**
- File tree mirrors module tree. `mod foo;` in `main.rs` reads from `foo.rs` or `foo/mod.rs`.
- Use `pub mod` to expose modules; keep internals private by default.
- Group by responsibility, not by type (`db/` module, not `types/` + `models/` split).

**Code per binary is small. Code per crate is focused.**
- A binary should mostly be wiring: config loading, dependency wiring, main loop.
- Business logic lives in library crates — testable, reusable.

**Separate fast paths from slow paths.**
- I/O, network, DB calls: async or blocking but always clearly separated.
- Don't mix blocking and async in the same function.

**Traits for polymorphism.**
- `trait` definitions in library crates; `impl` in implementation crates.
- `dyn Trait` only when you need runtime polymorphism — it has a vtable cost.
- Prefer generic `impl Trait` for compile-time dispatch.

**Constants and statics.**
- `const` for compile-time evaluation — use it for magic numbers that are truly constant.
- `static` for process-global mutable state (with `Mutex` or atomic types).
- `lazy_static` / `once_cell::sync::Lazy` for expensive globals initialized once.

**The dependency graph.**
- Keep core domain types in a crate with no external dependencies (besides `std`).
- Name crates by what they do: `auth-lib`, `http-middleware`, not `utilities` or `helpers`.

---

## 10. Daemons

**Run as a daemon, not just "in the background."**
- `systemd` (or `launchd` on macOS) to manage lifecycle: restart on crash, log rotation, socket activation.
- Use `tracing` with a systemd journal sink (`tracing-journald`).

**PID files are obsolete.** Let the service manager handle this.

**Graceful shutdown.**
- Listen for `SIGTERM`. When received:
  1. Stop accepting new work.
  2. Wait for in-flight work to complete (with a timeout).
  3. Flush and close logs, DB connections, file handles.
  4. Exit cleanly with code 0.
- `SIGKILL` = hard stop — you have no chance to clean up. Only happen if graceful shutdown times out.

**Configurable resources.**
- Number of worker threads, queue depths, timeouts — all should be configurable via env vars or config file, not hardcoded.
- `serde` + `config` crate for hierarchical configuration.

**Health check endpoint.**
- `GET /health` returning 200 with `{"status": "ok"}` — essential for orchestration (Kubernetes, systemd).
- Include basic checks: DB connectivity, Redis, downstream services.

**UID/GID switching.**
- Run as non-root. Use `setuid`/`setgid` after binding to privileged ports.

**Daemonization pattern.**
- `fork()` + `setsid()` is rarely needed in modern Rust — just run in the foreground under systemd.
- Double-fork is a Unix artifact from before systemd.

---

## 11. Signal Handling

**Never ignore `SIGTERM`.**
- It's the standard signal for "shut down gracefully." Always handle it.

**`SIGINT` (Ctrl+C).**
- On first Ctrl+C, trigger graceful shutdown.
- On second Ctrl+C within a short window, `SIGKILL` yourself — the user really wants to stop now.

**`SIGHUP`.**
- Traditionally: reload config. Useful for long-running daemons.
- Document clearly: does your app support config hot-reload via SIGHUP or not?

**Signal handling in async context.**
- `tokio::signal::ctrl_c()` for async signal handling.
- Use a `CancellationToken` shared across tasks to propagate shutdown signals.
- Never do blocking operations inside a signal handler — do minimal work (set a flag) and let the main loop handle the rest.

**`SIGPIPE`.**
- Default behavior (termination) is often correct. If you ignore it, you'll get broken pipe errors when writing to dead connections.
- Rust handles `SIGPIPE` correctly by default in most cases — you rarely need to touch it.

**Avoid complex signal logic.**
- Keep signal handlers minimal: write to a channel, set an atomic bool, flip a flag.
- All real work happens in the main loop or dedicated task.

---

## 12. Saved State / Caching

**Serialization format first.**
- `serde` + `JSON` for human-readable; `bincode`, `MessagePack`, or `protobuf` for binary.
- `serde_json` is slow for large data — consider `simd-json` or `rkyv` (zero-copy) for hot paths.
- `RON` for Rust-specific serialized state (preserves more type info than JSON).

**Write-ahead log (WAL).**
- For state that must survive crashes: write mutations to a WAL before applying them.
- On restart, replay the WAL to recover state.
- `sled` or `rocksdb` give you this for free with their write-ahead log.

**Checkpoints.**
- Periodic full snapshots (e.g., every N seconds or N mutations) + WAL for delta.
- On restart: load latest snapshot, then replay WAL deltas.

**Cache invalidation.**
- TTL-based caches with clear expiration.
- Use versioning on keys if you need safe invalidation.
- `dashmap`, ` ConcurrentHashMap` (from ` ConcurrentMap` crate) for in-memory caches.
- `moka` crate — high-performance, TTL-supporting, async-friendly.

**Resilience.**
- Never assume the cache is available. Cache misses should fall through to the source of truth.
- Graceful degradation: if Redis is down, serve from local cache or compute directly.

**Avoid stale reads.**
- If you cache mutable state, make invalidation explicit.
- `RwLock` for read-heavy caches; `Mutex` for write-heavy.

**Persistence of async state.**
- `tokio::spawn` + persistence is tricky — use a dedicated actor/state machine pattern.
- Persist actor state at natural boundaries (after processing a batch, before shutdown).

---

## Bonus Categories

### FFI (Foreign Function Interface)

**Rust talks to C and nothing else reliably.**
- `#[repr(C)]` for all FFI structs. No Rust-specific layout.
- Use `bindgen` to generate bindings from C headers automatically.
- Never pass Rust types across FFI boundaries — only `#[repr(C)]` types, raw pointers, or primitives.
- `libloading` crate for dynamic library loading.

### Async Runtime Choice

**`tokio` is the industry standard for async Rust.**
- Multi-threaded runtime for CPU-heavy async workloads.
- `#[tokio::main]` macro for simple single-threaded setups.
- `async-std` exists but `tokio` has far more production battle-testing.

**Use the right executor for the job.**
- IO-bound: async + `tokio`.
- CPU-bound: `rayon` (parallel iterators) or `tokio::task::spawn_blocking`.
- Mixed: both, wired together.

### Testing

**Unit tests go in the same file, behind `#[cfg(test)]`.**
- `#[test]` for test functions.
- `#[quickcheck]` or `proptest` for property-based testing.
- `#[tokio::test]` for async tests.

**Integration tests live in `tests/` directory.**
- Test the public API, not internals.
- Use tempdir crates or in-memory DBs for isolation.

**Fuzz testing.**
- `cargo-fuzz` with `libfuzzer-sys` for targeted fuzzing.
- Good targets: parsers, serialization, state machine transitions.

### Dependency Management

**Pin your dependencies in production.**
- `Cargo.lock` is committed — it's your reproducibility guarantee.
- `cargo update` + review the diff before upgrading in production.

**Keep `rust-toolchain` file in the repo.**
- Pin to a specific toolchain version: `rust-toolchain` file with `channel = "1.80.0"`.
- Use `rustup` to manage toolchains.

**Watch for `unsafe` in dependencies.**
- `cargo geiger` to count unsafe usage across your dependency tree.
- Prefer pure-Rust alternatives where performance is acceptable.

### Build Times

**Split monorepos into smaller crates** to parallelize compilation.
- `cargo check --lib` for fast iteration.
- `sccache` or `ccache` for CI — Rust compilation is expensive.
- `cargo-build-script` for code generation that runs at build time.

---

## General Principles

1. **Make it work, make it safe, make it fast — in that order.** Correctness is never a trade-off.
2. **Push work to compile time.** Types, const generics, compile-time evaluation — catch errors before you run.
3. **Design for failure.** Every external resource (DB, HTTP, disk) can fail. Handle it explicitly.
4. **Own your dependencies.** Every `cargo add` is a security and maintenance commitment. Audit them.
5. **Write the doc comment first.** `/// What this does and when to use it` forces clarity.
6. **Clippy is not optional.** `cargo clippy -- -D warnings` in CI. It's free advice.
7. **Format before commit.** `cargo fmt` + `cargo check` in a pre-commit hook.
8. **Naming is design.** `process_batch` vs `handle_batch` vs `drain_batch` — pick one and be consistent.
9. **The crate root is the public API.** Everything else is implementation detail.
10. **When in doubt, lean toward explicitness.** Implicit behaviour, hidden allocations, automatic coercions — they're all fine until they're not.