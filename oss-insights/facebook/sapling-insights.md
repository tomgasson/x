# Sapling (facebook/sapling) — Engineering Insights 2023–2025

Sapling is Facebook/Meta's Mercurial-based source control system, comprising an EdenFS filesystem daemon (C++), a Mononoke backend server stack (Rust), and a Sapling CLI (Python/Rust). This analysis covers 50 substantive commits sampled from the 2023–2025 history, focusing on architectural changes, performance work, security hardening, and significant reworks.

---

## 🔬 Coroutine Migration — getDigestHash / getSHA1 Pipeline

**Commits:** `55c032d` `ed5c993` `37e5d12` `b653eb3` `d86b0f7` (phase 11)

- **Author:** Jane Zhang (@jazhang)
- **Situation:** The EdenFS inode layer had several Thrift handler methods still on legacy futures/ImmediateFuture pipelines. The team wanted to consolidate onto C++20 coroutines.
- **Approach:** Migrate in dependency order: leaf (ObjectStore) → TreeInode → VirtualInode → EdenServiceHandler. Each layer exposes `co_get<Operation>` that chains to the next via `.semi()` bridges during migration, then replaces the bridge with a direct `co_await` once the callee is migrated. Config-gated via `enableCoroutinesPhase11`.
- **Mechanism:** Used `co_applyToVirtualInode` with `get_if` chains (avoiding `match` which captures coroutine lambdas). Unlocked `LockedState` before `co_await` to avoid suspension deadlocks. Added benchmarks (getSHA1 benchmark following getBlake3 pattern) for regression tracking.
- **Scale:** Affects all inode stat/contents paths — every file read or SHA1/digest lookup through EdenFS.
- **Cost:** High review investment (multi-commit stack), significant test coverage added per layer.

---

## 📦 WAL Streaming-Merge (fscatalog)

**Commit:** `d40958c` — Spencer Burakowski (@sburakowski)

- **Author:** Spencer Burakowski
- **Situation:** The `FsInodeCatalog` WAL replay was deserializing each operation in isolation and merging into an `OverlayDir`. For large working copies the overlay rebuild was expensive.
- **Approach:** Introduced `WalDelta` — a streaming-merge variant of WAL replay that pre-collapses same-name operations before constructing the final map, using `PathMapMutator` directly instead of going through full `OverlayDir`. Collapse rules: `ADD→REMOVE=REMOVE`, `REMOVE→ADD=ADD`, `ADD→MATERIALIZE=MATERIALIZE` (hash cleared).
- **Mechanism:** Stops on first malformed/torn entry and ignores the tail. ENOENT on WAL file returns an empty map — same crash-safety guarantees as replayWal.
- **Scale:** Affects all EdenFS mount-time inode catalog initialization.
- **Cost:** 405 lines added across catalog and tests.

---

## 🚫 No-Follow VFS — Symlink Safety Overhaul

**Commits:** `8e59f752` `cbf42a0d` `a286cdd` `3113efd` `0cbd87a` `aaf7039` `67f1535e` `37d104ab` `b3c79733` `11f627a6` `f5072ab9` `5c35b98e` `2da9a38` `e13f6ddf` `55af680d` `9032ffd1` `0420fa6e` `4094e022` `8bb31fe0` `8960a066` `f81de59b` `472a6cac` (excerpted)

- **Author:** Jun Wu (@quark), multiple reviewers
- **Situation:** Sapling's path-based filesystem APIs in Python and Rust could write through symlinks — a TOCTOU auditing risk when concurrent checkouts are running.
- **Approach:** Migrate all path-based APIs to VFS (Virtual File System) primitives that enforce no-follow semantics. Linux uses `RESOLVE_NO_SYMLINKS`, macOS uses `O_NOFOLLOW_ANY`. Optimize O(components) syscalls to O(1) via the no-follow flag on `open`. On Windows, reject NTFS ADS paths, tighten file creation, and disable the RepoPath→CheckedRelPath fast path.
- **Mechanism:** Python `bindings.vfs` module exposing a thin wrapper around Rust `vfs::VFS`. Working copy traversal switched to no-follow VFS walk. Rust `util/no_follow` primitives used across the tree to avoid writing through symlinks. Errors normalized to `NotFound` rather than `AccessDenied` for symlink-follow attempts.
- **Scale:** Every Sapling filesystem operation — clone, status, commit, checkout.
- **Cost:** Multi-year multi-person effort; extremely extensive test coverage added.

---

## 🔐 Restricted Paths — Incremental ACL Enforcement Rollout

**Commits:** `0e9bcae` `f8beeb02` `56a40497` `70eb0ad08` `dd266afd` `1013f604` `c991865b` `475d5d5` `471030e6` `b4ebf923` `31c59dae` `93e22cee` `ed6a7837` `8a21c64c`

- **Author:** Gustavo Avena (@gustavoavena), YousefSalama, lmvasquezg
- **Situation:** Server-side augmented tree ACL enforcement needed to roll out incrementally — Eden fetches info from the server even without `.slacl` files. The existing `trees` endpoint was hardcoded to `AclManifestMode::Authoritative`, ignoring the config.
- **Approach:** Multiple diffs: extract augmented child metadata shaping, rename constructors, introduce `AccessEnforcementOutcome` typed enum, carry typed `RestrictedPathsAuthorizationError` through Mononoke errors, convert to SLAPI `PermissionDenied` for clients. Permission request groups named and typed as `PermissionRequestGroup` (alias for `MononokeIdentity`). Fixtures migrated from `TIER:` to `GROUP:` identities to match production.
- **Mechanism:** `trees` endpoint now uses facet primitives. ACL enforcement outcomes logged to Scuba with `log_tag=per_bookmark_locking_active` pattern. Errors preserve the `request_acl` through the error chain.
- **Scale:** Affects every trees/files batch request to Mononoke SLAPI.
- **Cost:** Large multi-stack effort over ~8 months, significant test fixture churn.

---

## 📊 NanoDag — Non-Linear History for LineLog

**Commits:** `3465d3cb` `7907f81e` `2073672c` `17537e32` `09bec3fd` `f661338e` `43e6c07c` `5d2e069` `d455830a` `bf3d4644` `b7572f5a` `993b6183` `09bec3fd` `372d1f038` `f2c7585f` `372d1f038`

- **Author:** Jun Wu
- **Situation:** LineLog was historically linear — unable to represent even a linear stack with pending changes on top of the middle. This was too restrictive for real workflows.
- **Approach:** Introduce `NanoDag` — a dag optimized for linelog's small draft-stack use case (not general-purpose). Store it on `LineLog`. `calculate_dep_map` now returns `NanoDag` for easier ancestor/descendant queries. Port block-shifting from TypeScript linelog.ts to relax ordering constraints.
- **Mechanism:** Block shifting allows more revisions to be reordered more freely. The `absorb` API used integer line numbers which was incompatible with block shift — a future change may switch to string linelog. Ported insertion optimization, `describeHumanReadableInsDelStacks` (Unicode ASCII art), reorder tests, `remap` and `truncate` APIs.
- **Scale:** Local-only linelog editing operations.
- **Cost:** 496 lines for nanodag.rs alone; extensive test porting from TypeScript.

---

## 🔄 Merge Resolution Override — Typed Enum Migration

**Commits:** `39a7568` `4a73834` `60429c5e` `dd266afd` `1013f604` `e2ade750` `70eb0ad08`

- **Author:** Rajiv Sharma (@rajshar), Jan Mazur (@mzr)
- **Situation:** The per-request `merge_resolution_override` was `Option<bool>` — meaning of `None` was implicit, making it easy to accidentally default to JK behavior. The QE rollout needed three intents: force on, force off, defer to JK.
- **Approach:** Replace with `MergeResolutionOverride { UseJk, ForceOn, ForceOff }` so the choice is named and exhaustive at every callsite. Wire through `land_stack`, then through SLAPI Land handler (parse from pushvar), then through SCS, then through unbundle. Shared parser on the enum itself so D4/D5 become 3-line wirings.
- **Mechanism:** `MERGE_RESOLUTION_OVERRIDE` pushvar parsed via `MergeResolutionOverride::from_pushvar_value`. `sl push --pushvar MERGE_RESOLUTION_OVERRIDE=true` now works end-to-end.
- **Scale:** Push/rebase path across all Mononoke-backed repos.
- **Cost:** 10 files changed in the typed enum migration alone.

---

## 👤 Identity Refactor — Collapsing to MononokeIdentity Newtype

**Commits:** `66c774e` `30c2064e` `4a556e` `2940c66a` `c3c5292d14b`

- **Author:** Jan Mazur
- **Situation:** `MononokeIdentity` was a dual-variant enum (`TypeData | Authenticated`) — but after D105319204 every ingress path already produces an `AuthenticatedIdentity`. The dual variant carried no information the inner thrift struct didn't already carry.
- **Approach:** Collapse to `pub struct MononokeIdentity(pub AuthenticatedIdentity)`. Switch identity extraction across all HTTP and SR Thrift entry points to `get_authn_identities_with_conversion`. Drop `Authenticated;` prefix from `to_typed_string`. Render as `mid://` URI via `authenticated_identity_serializer::serialize` — the same format used by C++ wire envelope.
- **Mechanism:** `to_typed_string` now emits `mid://PROD/USER/foo?agent.id=AGENT%3Adevmate&primary=true`. Falls back to `mid://SERIALIZEERR/TYPE/data` on serialization failure. OSS builds emit `mid://TEST/TYPE/data`.
- **Scale:** Every identity logged in Mononoke Scuba telemetry.
- **Cost:** 16 files changed across the collapse.

---

## 📈 Pipelined Coroutine Inode Enumeration

**Commits:** `956ea43` `122a1cb6` `97f0fe1c`

- **Author:** Spencer Burakowski
- **Situation:** `getChildren` (directory enumeration) was still on the legacy futures pipeline. Wanted coroutine version that preserves parallelism profile.
- **Approach:** `TreeInode::co_getChildren` snapshots directory entries under `lockContentsWrite()`, runs `loadChild()` under the contents lock (for inode-map coordination), defers object-store tree fetches until after lock release. Restricted-directory recheck kept async (`.await` not `.get()`). `VirtualInode::co_getChildren` mirrors: loaded → `TreeInode::co_getChildren()`, unloaded → parallel fetch, file entries → sync resolve, restricted → synthesized restricted VirtualInodes.
- **Mechanism:** Pipelined fanout: each child task resolves its VirtualInode and immediately fetches attributes, joined with `collectAllTryRange`. Regression tests for restricted trees, loaded InodePtr dispatch, file-as-directory errors, mixed unloaded TreePtr children.
- **Scale:** Every directory enumeration through EdenFS.
- **Cost:** 3 files, ~500 lines added across inode layer.

---

## 🐛 LFS Pointer Resolution — Internal Blobstore Path

**Commits:** `8d8a98ba` `f980b239` `afcc3618`

- **Author:** Jan Mazur
- **Situation:** `repo_import` stored LFS pointer text as file content instead of resolving to actual bytes — leaving any LFS-using repo broken after import. Git push had no internal-LFS path, incurring an HTTP/TLS hop to an upstream LFS server even when content was already in the local blobstore.
- **Approach:** `repo_import` gains `--lfs-server URL` and `--internal-lfs` flags. `mononoke_git_service` gains `--internal-lfs` flag. Refactored `GitImportLfsInner` from struct to enum with `Upstream` and `Internal` variants. Internal path uses `filestore::fetch_with_size` with `Alias::Sha256` lookup, skips retries.
- **Mechanism:** Mutual exclusivity enforced by clap. `--internal-lfs` becomes the default when `--lfs-server` is unset.
- **Scale:** Git push and import for LFS-using repos.
- **Cost:** 7 files, ~700 lines added for git_server internal-lfs alone.

---

## 🚀 Perf Counters — Throughput Telemetry

**Commits:** `a457704f` `9ec603ff`

- **Author:** Prashant Pal, Rajiv Sharma
- **Situation:** Existing ODS counters tracked items after processing (`files_served`, `manifests_served`). Missing batch request size metrics for autoscaling decisions.
- **Approach:** Added `files_batch_keys_requested` and `trees_batch_keys_requested` ODS timeseries counters to SLAPI handlers. Per-bookmark locking path gets telemetry for lock acquisition and log ID allocation latency — unsampled (every write logs one row) because infer repo has ~13 writes/hr and sampling would drop to ~0.
- **Mechanism:** `define_stats!` macros, `.unsampled()` Scuba logging with `log_tag=per_bookmark_locking_active`.
- **Scale:** All SLAPI batch requests and bookmark writes.
- **Cost:** Low (single-digit lines per counter).

---

## 📋 Keep-Or-Die Derive-Slice

**Commit:** `c3c5292d`

- **Author:** Gustavo Avena
- **Situation:** `derive-slice` command failed fast on first error. Large backfills became painful when a few changesets have underived dependencies.
- **Approach:** Added `--keep-going` flag that switches to `buffer_unordered + collect`, processing all boundaries/slices and collecting errors along the way. Reports all failures with changeset IDs and error details, then returns a summary error.
- **Mechanism:** Fail-fast unchanged without the flag.
- **Scale:** Large derived-data backfills.
- **Cost:** 202 insertions, 51 deletions.

---

## 📝 LineLog Perf — Cache Hit Testing

**Commit:** `17537e32`

- **Author:** Jun Wu
- **Situation:** The dag cache (ancestors, descendants) adds overhead. Regular sequential `edit_chunk` and `checkout` have fast paths (a_lines_cache to avoid execute, fast is_ancestor check) that should not trigger dag cache builds — but there was no test to verify this.
- **Approach:** Added `PerfStats { cache_hit, execute, dag_cache }` instrumentation and a test that asserts `dag_cache == 0` for sequential operations. Reported stats show `dag_cache: 0` for regular workflows.
- **Mechanism:** Instrumented `LineLog::execute` with `PerfStats` tracking.
- **Scale:** Local editing operations.
- **Cost:** 28 lines added.

---

## 📂 Worktree Direct Copy (Snapshot Mode)

**Commit:** `c9e2c907`

- **Author:** Xiaowei Lu
- **Situation:** `sl worktree add --snapshot` used `sl snapshot create/checkout` shell-outs with network I/O and subprocess spawns.
- **Approach:** Added direct file copy path gated behind `worktree.snapshot-direct-copy` config: compute status via Rust `WorkingCopy::status()`, run `eden clone` to create dest at p1, then copy modified/added/untracked files and remove deleted/removed files from source to dest in parallel using VFS. Skips treestate update if any file operations failed.
- **Mechanism:** Eliminates network I/O and two subprocess spawns. Falls back to old path when config is not set.
- **Scale:** `sl worktree add --snapshot` operations.
- **Cost:** 321 lines across 4 files.

---

## 🔍 ISL — Whitespace-Ignored Diff Comparison

**Commit:** `a286cdd`

- **Author:** Evan Krause (@evangrayk)
- **Situation:** The Sapling ISL comparison view had no "ignore whitespace" option — making certain changes (like wrapping something in an `if`) hard to read.
- **Approach:** Pass an ignore-whitespace flag through to `sl diff`.
- **Mechanism:** 26 lines added to ComparisonView, 5 to test.
- **Scale:** Interactive diff viewing in ISL.
- **Cost:** Minimal.

---

## ⏱️ gclone — Remove `--refresh-index-stats` (Replaced by `--check-stat`)

**Commit:** `23e3792ae`

- **Author:** George Giorgidze
- **Situation:** D105659963 added `--check-stat minimal|default` to `gclone` with `minimal` as default. With `core.checkStat=minimal` and CAF-preserved `mtime`/`size`, first `git status` after CAF download is ~0.61s — within 3% of the ~0.59s from `--refresh-index-stats`. The original flag was scheduled for removal.
- **Approach:** Deleted the 411-line `refresh_index_stats.rs` module, all CLI flag bindings, `Default` initializers, and corresponding BUCK/crate entries. Preserved coverage via existing `--check-stat=default` blocks.
- **Mechanism:** Deleted 40 lines from integration test, removed `sha1` and `rayon` deps from BUCK.
- **Scale:** `sl clone` operations.
- **Cost:** ~400 lines deleted.

---

## 🎛️ EdenFsEventsLogger — Structured Event Wrapper

**Commits:** `cc9cd72` `8ac382c9` `d9189a3f` `53781506` `acfce31d` `f0f99c4b` `7f10308` `ecad0e02` `96c6c450` `a3412ccb` `fdc31173` `92d35b64`

- **Author:** Jane Zhang
- **Situation:** `StructuredLogger` is a header file with multiple logger implementations — couldn't modify it without affecting all loggers. Edenfs events needed dual-write to both `StructuredLogger` (for existing consumers) and XplatLogger (perfpipe) without touching the shared header.
- **Approach:** Created `EdenFsEventsLogger` class that wraps `StructuredLogger` and chooses `XplatLogger` vs `StructuredLogger` based on `enable-xplatlogger-events` config. Follows either/or pattern (not dual-write) — only writes to xplatlogger if object is not null and config is enabled. Centralizes logger writing logic to avoid duplication.
- **Mechanism:** `EdenServer::registerXplatTransforms()` registers `edenfsEventsTransform`. Counters added to `TelemetryStats` for both paths.
- **Scale:** All EdenFS event logging.
- **Cost:** 328 lines added for the logger class and tests.

---

## 🗂️ Per-Bookmark Locking — Unscheduled Telemetry

**Commit:** `9ec603ff`

- **Author:** Rajiv Sharma
- **Situation:** Phase 3 rollout of per-bookmark locking needed production-path telemetry for lock-acquisition and ID-allocation latency. Existing shadow-log path was sampled at infer's rate (~13 writes/hr) and observed events dropped to ~0, leaving the team unable to measure latency.
- **Approach:** Added unsampled telemetry logging after `acquire_bookmark_locks` and `allocate_log_ids` succeed. Fields: `per_bookmark_lock_acquired_us`, `per_bookmark_log_ids_allocated_us`, `per_bookmark_lock_repo_id`, `per_bookmark_lock_entry_count`. Used new `log_tag=per_bookmark_locking_active` (distinct from shadow `per_bookmark_locking_shadow`).
- **Mechanism:** `std::time::Instant` timing, `.unsampled()` Scuba.
- **Scale:** All new-path bookmark writes.
- **Cost:** 18 lines in transaction.rs.