# wild-linker/wild — OSS Insights

A Rust-based linker (lld fork) targeting ELF, Mach-O, and WebAssembly. Supports linker plugins for LTO, linker scripts, and thin archives.

---

## Commit-by-Commit Substance

### wild afcbfe Enable linker plugin support by default
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Linker plugins (used for LTO with GCC/Clang via `-fuse-ld=wild`) were opt-in but are now the recommended default for production builds.
**Approach:** Flip the default from off to on, add test coverage for the new default behavior path, update README to reflect the new default.
**Mechanism:** Changes `wild/Cargo.toml` default features to include plugin support, updates test config to exercise plugin path when plugins are enabled, adjusts the mold skip test to reflect new default behavior.
**Scale implications:** Changing a default is high-visibility — affects every user who upgrades without explicit flag changes. Requires careful backward-compat thinking.
**Cost:** Risk of surprising users who relied on the previous default; documentation update burden.

### wild 3764c74 Emit cause when failing to load plugin
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** When a linker plugin failed to load, the error message was opaque and didn't help developers diagnose whether the plugin binary was missing, permissions wrong, or version-mismatched.
**Approach:** Attach the underlying `io::Error` as a `cause` on the LinkerError, so chain introspection gives developers actionable context.
**Mechanism:** Plugin loading now uses `map_err(|e| LinkerError::PluginLoadFailed(e.to_string()).with_cause(e))` pattern — the `cause` field preserves the raw OS error.
**Scale implications:** Production linker failures with unclear errors are expensive — every minute debugging a plugin load failure is time not spent linking.
**Cost:** Minor — one extra field on the error enum, chain propagation through plugin initialization path.

### wild b203f48 Emit error on missing resolution for symbol alias
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `--defsym` aliases and linker script aliases that couldn't be resolved to a symbol silently failed or produced confusing downstream errors rather than failing fast at resolution time.
**Approach:** Add explicit error emission when alias resolution finds no target symbol.
**Mechanism:** Resolution path now checks if target exists after alias expansion; if not, emits a diagnostic with the alias name and unresolved target.
**Scale implications:** Linker scripts with typos or missing symbols now fail at parse time rather than producing cryptic "undefined symbol" errors much later in the link.
**Cost:** Breaking change for build scripts that were silently producing bad binaries — though those were already broken.

### wild 1be190c Load archive entries targeted by --defsym and linker script aliases
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `--defsym` could define an alias to a symbol that only existed inside an archive (`.a` file), but the linker wouldn't automatically pull in the archive member containing that symbol because it wasn't going through the normal symbol resolution path.
**Approach:** Track alias targets and ensure archive members are loaded if they're needed to satisfy an alias resolution.
**Mechanism:** When `--defsym` alias targets an undefined symbol, the resolution machinery now marks that symbol as needing resolution, triggering archive loading. Previously aliases bypassed this mechanism.
**Scale implications:** Large projects using `--defsym` to redirect symbols across library boundaries can now rely on archive extraction without manual `--whole-archive` flags.
**Cost:** Slight increase in resolution work for alias-heavy link lines.

### wild 13a35a7 Prevent GC of linker script symbol aliases
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Linker script-defined aliases (like `SYM = target;`) could be garbage-collected by the linker if no `etext`/`edata` reference chain existed to them, even though they were semantically needed by user code that referenced them via the script alias name.
**Approach:** Mark linker script aliases as root symbols in the GC graph so they survive dead code elimination.
**Mechanism:** Symbol table entries for script aliases get marked `include_in_gc_sections = false` — the linker treats them as roots even if unreferenced by other kept symbols.
**Scale implications:** Projects using linker scripts to define symbol aliases for version compatibility now correctly preserve those aliases across GC passes.
**Cost:** Marginal binary size increase from now-kept aliases, but alias targets are typically tiny (a single address constant).

### wild 029e530 Fix resolution following linker scripts / synthetic symbols
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Symbol resolution that went through linker script assignments or synthetic symbols (created by the linker itself) was failing to propagate through the alias chain correctly, causing "undefined symbol" errors for symbols that clearly existed.
**Approach:** Walk the alias chain to its final target in all resolution paths, not just the initial lookup.
**Mechanism:** Resolution functions now loop: resolve → check if alias → resolve alias target → repeat until reaching a concrete definition or a synthetic symbol that provides its own definition.
**Scale implications:** Complex linker scripts with indirection through multiple aliases now resolve correctly without manual workarounds.
**Cost:** Slight performance cost for resolution paths that iterate once extra.

### wild 3bbd4f8 handle missing version node for synthetic symbols
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Synthetic symbols (created by the linker internally, e.g., `PROVIDE`d symbols or section start/end markers) that lacked a version node caused panics when accessed via versioned symbol lookups.
**Approach:** Add a null check / Option handling for the version node when resolving synthetic symbols.
**Mechanism:** Synthetic symbol resolution now returns `None` for version rather than panicking when the version node is absent.
**Scale implications:** Projects using linker scripts to define `PROVIDE` symbols without explicit version markers now work correctly when other parts of the link reference them via version-qualified names.
**Cost:** Added branch in hot resolution path; acceptable given synthetic symbols are a minority of total resolution calls.

### wild 40c81d1 port(MachO): use args.output for CS identifier
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** When emitting code signatures on Mach-O, the identifier was being derived from the linker-internal binary name rather than the actual output filename, causing signatures to not match the file on disk.
**Approach:** Use the user-specified output path (`args.output`) as the code signature identifier instead of the internal binary name.
**Mechanism:** The `CodeSignature::new(output_path)` path now passes the actual filesystem path to the signature computation, not an internal identifier.
**Scale implications:** Gatekeeper and notarization workflows on macOS rely on code signature identifiers matching the binary's actual path; incorrect signatures cause runtime rejections.
**Cost:** None — more correct behavior with same signature algorithm.

### wild e68c706 format TOML files and use taplo for checking
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** TOML files in the project had inconsistent formatting, making diffs noisy and manual editing error-prone.
**Approach:** Enforce TOML formatting via taplo (TOML linter/formatter) in CI, apply consistent formatting to all TOML files.
**Mechanism:** Added taplo to CI checks, reformatted all Cargo.toml and other TOML files to match taplo's output.
**Scale implications:** Standardizing formatting reduces cognitive overhead for contributors and makes automated TOML edits safer.
**Cost:** One-time reformatting of all TOML files; ongoing formatting enforced in CI.

### wild c65cb13 port(wasm): Implement object file symbol accessors and `Symbol` trait
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** WebAssembly port lacked a complete `Symbol` trait implementation, so symbol operations had inconsistent behavior across targets.
**Approach:** Implement the full `Symbol` trait for WebAssembly object files, including name, address, size, and binding accessors.
**Mechanism:** WebAssembly `ObjectFile` now implements `Symbol` by reading from the `Name` section and custom symbols section of the WASM binary. Binding (local/global/exported) inferred from symbol kinds.
**Scale implications:** WASM target now has consistent symbol resolution semantics with ELF/Mach-O — tools built on top of wild work across all targets without special-casing.
**Cost:** Added abstraction boundary that may slightly slow down symbol lookups (trait dispatch vs direct method call), but enables code sharing.

### wild 17b1531 support PROVIDE within the SECTIONS toplevel command
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Linker scripts using `PROVIDE(sym = expr)` inside the `SECTIONS { }` block were rejected, even though GNU ld supports this.
**Approach:** Parse `PROVIDE` as a valid statement inside the `SECTIONS` block, not just at top level.
**Mechanism:** Grammar extended to allow `PROVIDE` as a statement within section definitions. Provided symbols get special handling — they only define a value if the symbol is undefined at the end of the link.
**Scale implications:** Compatibility with a wider range of real-world linker scripts, especially those ported from GNU ld.
**Cost:** Added parse rule; no runtime cost for scripts that don't use this feature.

### wild c998068 feat: support `--nmagic`
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `-nmagic` flag (disable text segment alignment penalties) was not implemented, causing the linker to reject the flag rather than honoring it.
**Approach:** Implement the `--nmagic` option that sets text segment permissions to allow execution without alignment restrictions.
**Mechanism:** Parses `--nmagic` flag, sets a flag on the target environment that relaxes text segment alignment constraints. On ELF, passes `PF_W | PF_R` (no execute) for text segments when `nmagic` is set; on Mach-O adjusts segment flags similarly.
**Scale implications:** Embedded systems and kernel modules often need `--nmagic` to control memory protection bits precisely. Missing support blocks wild adoption in those environments.
**Cost:** Added flag parsing and conditional behavior in segment creation.

### wild 30d4077 port(MachO): fat binary support
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Mach-O port only handled single-architecture binaries. Projects building universal/fat binaries (containing slices for multiple architectures) couldn't link with wild.
**Approach:** Implement fat binary (universal binary) reading and writing, with architecture-specific slice selection during linking.
**Mechanism:** `FatBinary` structure reads a universal binary's slice table. When linking, selects appropriate slice based on target architecture, extracts thin Mach-O from selected slice, and produces a thin output. Also supports writing fat binaries from multiple thin inputs.
**Scale implications:** macOS toolchain has long used fat binaries for cross-architecture builds; supporting them enables wild to replace `ld` in more build environments.
**Cost:** Additional parsing overhead when reading fat binaries; transparent to users targeting a single architecture.

### wild 58f3e10 fix: Allow LTO to eliminate dead code
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** LTO (Link Time Optimization) with linker plugins was preventing the linker from passing `--gc-sections` to the plugin, causing dead code elimination to not work correctly when LTO was enabled.
**Approach:** Propagate `--gc-sections` semantics to the linker plugin when LTO is active, so plugin can perform its own dead code elimination.
**Mechanism:** When constructing plugin options, include a flag indicating GC sections is requested. Plugin uses this to set appropriate LTO options (e.g., `-falign-functions` for ICF compatibility, `-ffunction-sections` for section-based GC).
**Scale implications:** Projects using LTO+gc-sections to reduce binary size now get the expected size savings, not a surprise bloated binary.
**Cost:** Minor option-passing addition; plugin-side consumes the flag.

### wild 50f3f86 port(wasm): Implement object file section accessors
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** WebAssembly port didn't expose section-based access to object file internals, limiting what tools built on top could do without re-implementing section parsing.
**Approach:** Implement `Section` trait for WebAssembly sections (text, data, reloc, custom).
**Mechanism:** `WasmSection` enum maps to WASM binary section IDs. Iterator over sections provides type-tagged access to section data.
**Scale implications:** Aligns WASM port with the object-file abstraction used by other targets — tools that iterate sections (like `wild-objdump`) work across all targets.
**Cost:** Added abstraction layer with minimal runtime overhead for normal linking flows.

### wild 29f88e0 feat: implement symbol resolution within the ASSERT command
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Linker script `ASSERT` commands couldn't reference symbols created via `PROVIDE` or linker script aliases, causing validation checks to fail when the intended symbol existed but wasn't resolvable through the expected path.
**Approach:** Allow `ASSERT` to resolve symbols through the full resolution chain including synthetic and linker-script-defined symbols.
**Mechanism:** `ASSERT` expression evaluation now calls the same resolution function used elsewhere, which handles PROVIDE, aliases, and synthetic symbols.
**Scale implications:** Projects using linker scripts to define version markers and then assert on them (common in glibc-style symbol versioning) now work correctly.
**Cost:** Reuses existing resolution code — no new logic, just wired up to ASSERT evaluation.

### wild 3aa8ac2 Add mechanism to preserve linker plugin outputs
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Linker plugins produce intermediate files (e.g., `.import.o` for thin LTO) that were cleaned up aggressively after the link finished, preventing debugging of plugin behavior or recovery from interrupted links.
**Approach:** Add a `--preserve-plugin-outputs` flag that keeps plugin intermediate files after link completes.
**Mechanism:** Plugin coordinator now checks a global `preserve_outputs` flag before deleting temporary files. Temp files are named with a predictable pattern for easy inspection.
**Scale implications:** Plugin debugging is significantly easier when you can inspect the actual bitcode/object files the plugin produced, not just the final binary.
**Cost:** Disk space for intermediate files when flag is set; storage implications for large projects with many LTO units.

### wild 84cad54 port(wasm): Add section and program segment mapping
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** WebAssembly port didn't expose the mapping between source sections and output program segments, limiting the linker's ability to report meaningful errors about segment layout.
**Approach:** Implement the section-to-segment mapping for WASM, similar to how ELF exposes section-to-segment mappings.
**Mechanism:** `wasm::ObjectFile` now tracks which WASM sections contributed to each program segment. Program segment metadata includes back-references to source sections.
**Scale implications:** Error messages that say "relocation X in segment Y refers to out-of-bounds address" are now meaningful for WASM targets.
**Cost:** Additional metadata tracking; minimal impact on linking speed.

### wild d394417 feat: evaluate expressions within PROVIDE with `evaluate_expression`
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `PROVIDE(sym = expression)` in linker scripts could only handle simple constant expressions. Complex expressions (section-relative calculations, arithmetic combining multiple symbols) in `PROVIDE` definitions were silently producing wrong values or errors.
**Approach:** Route `PROVIDE` RHS through the full `evaluate_expression` path instead of a simplified constant evaluator.
**Mechanism:** PROVIDE expression parsing now feeds directly into `evaluate_expression`, which handles arithmetic, symbol references, and section-relative computations correctly.
**Scale implications:** Linker scripts that define dynamic symbols (like memory region sizes computed from other symbols) now work correctly.
**Cost:** PROVIDE evaluation is now slightly more expensive but correctness outweighs the cost.

### wild 0ac56e4 fix: Do not output SFrame section unless asked
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** The linker was unconditionally emitting SFrame debug sections when available, even when the output format didn't need them or the user didn't request them. SFrame is a relatively new debug format and not all tools consume it.
**Approach:** Make SFrame output conditional on an explicit flag `--enable-sframe` rather than always emitting when detected in inputs.
**Mechanism:** SFrame section is added to the output only when `args.enable_sframe == true`. Input SFrame sections are parsed but not passed through unless enabled.
**Scale implications:** Debug info bloat in binaries when input objects contained SFrame but user didn't want it; also avoids compatibility issues with tools that don't understand SFrame.
**Cost:** Correctness fix — users who wanted SFrame now need to explicitly request it.

### wild eb69f2b More robust code to get line numbers from debug info
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Line number lookup from debug info (for error messages and stack traces) would crash or return wrong results for certain DWARF constructs, particularly when line numbers were encoded in ways that didn't match the expected pattern.
**Approach:** Add fallbacks and defensive checks in the DWARF line info parsing path.
**Mechanism:** Line number lookup now handles: missing line number table, line number of 0 (which is valid for compiler-generated nops), and expression-relative addresses that don't fit the standard encoding.
**Scale implications:** User-facing error messages from the linker (e.g., "undefined symbol in file.o:42") are now more often correct and less often crashing when dealing with unusual DWARF.
**Cost:** Slightly more complex DWARF parsing logic; acceptable for correctness.

### wild 3fdc92a port(MachO): emit code signature
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Mach-O output binaries weren't getting code-signed, which is required on macOS for execution. The linker produced unsigned binaries that macOS would refuse to run.
**Approach:** Integrate macOS code signing into the linker's output flow — after writing the final Mach-O binary, invoke the `codesign` tool to sign it.
**Mechanism:** `MachOOutput::finalize()` now calls `codesign` as a subprocess if the output target is macOS and signing is requested. Supports ad-hoc signing for development builds.
**Scale implications:** Development builds using wild as their linker can now produce runnable binaries without manual `codesign` invocation.
**Cost:** External tool invocation latency; can be disabled for CI builds that don't need signed outputs.

### wild d5c8849 refactor: Change internal representation of --sym-def symbols
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `--sym-def=FILE` (read symbol definitions from a file) used an ad-hoc internal representation that made it hard to extend with new features or compose with other symbol definition mechanisms.
**Approach:** Unify `--sym-def` symbols into the same internal `DefinedSymbol` type used by other symbol sources (linker script `SYM = target`, `PROVIDE`, etc.).
**Mechanism:** `--sym-def` parser now produces `DefinedSymbol` structs rather than a custom type. This enables alias chaining, version qualifiers, and synthetic symbol handling to work uniformly across all symbol definition paths.
**Scale implications:** Unifying the representation reduces special cases; future features (like symbol versioning in `--sym-def`) become easier to implement.
**Cost:** Breaking internal API change — all consumers of sym-def symbols needed updating.

### wild e75e4e2 refactor: Change default_layout_rules to take Args and return a Vec
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `default_layout_rules` was a method on the linker that returned hardcoded defaults without access to command-line arguments, so features like `--nmagic` couldn't influence default layout behavior.
**Approach:** Change the signature to accept `&Args` so layout rules can be conditionally applied based on runtime flags.
**Mechanism:** `default_layout_rules(args: &Args) -> Vec<LayoutRule>` — rules are now constructed from flag values. `nmagic` flag produces different default rules for text segment alignment.
**Scale implications:** Layout rule customization via CLI flags now works correctly instead of being silently overridden by hardcoded defaults.
**Cost:** Call site changes throughout the linker — more flexible but requires updating every caller.

### wild d8efa88 port(wasm): Parse `linking` and `reloc.*` custom sections
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** WebAssembly binaries with explicit `linking` and `reloc.*` custom sections (used for multi-module linking) weren't being parsed, causing relocations to fail or be silently skipped.
**Approach:** Implement parsing of the WASM linking custom sections that contain module-level linking metadata.
**Mechanism:** `WasmObjectFile::parse_custom_sections()` now recognizes `linking` (provides module name, symbol table) and `reloc.*` (provides relocation entries for a named section) custom sections, populating the object's symbol and relocation tables accordingly.
**Scale implications:** Multi-module WASM linking scenarios (linking multiple `.wasm` files together, similar to using archives in ELF) now work correctly.
**Cost:** Custom section parsing adds to load time for WASM objects with linking sections; transparent for simple WASM files.

### wild 77b9fc3 Initial scaffolding for WebAssembly support
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Wild had no WebAssembly target support — it couldn't link `.wasm` object files or produce valid WASM output.
**Approach:** Add the initial `wasm` target to wild's architecture dispatch system, implementing `Target` trait for WASM with placeholder implementations for the core operations.
**Mechanism:** `wasm.rs` module created with `WasmTarget` struct implementing `Target` trait. Initial version handles: object file detection, section enumeration, symbol table structure, and output format writing. Relocation and memory layout use placeholder logic to be refined in subsequent commits.
**Scale implications:** First step toward WASM support — this is the scaffold future commits build on. Enables `wild --target=wasm` to at least load WASM files without crashing.
**Cost:** Placeholder implementations mean full functionality isn't there yet, but the plumbing is established.

### wild 4efa9dc fix: bad allocation for versioned internal symbol
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Versioned internal symbols (symbols with `@version` suffixes like `foo@GLIBC_2.0`) were causing a bad allocation failure when the version string exceeded some internal buffer size assumption.
**Approach:** Use a dynamically-sized string buffer instead of assuming a fixed-size buffer for version strings.
**Mechanism:** Version string parsing now uses a `String` rather than a fixed `[u8; N]` array. The version string is stored separately from the symbol name rather than being packed into a single buffer.
**Scale implications:** Symbols with very long version strings (rare but possible in some build systems) now work without allocation failures.
**Cost:** Minor heap allocation for version strings that were previously stack-allocated; negligible given version strings are short.

### wild 34747a0 doc: Add instructions for installing with Brew
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Users couldn't find wild in Homebrew, the most common package manager for macOS developers, requiring manual installation from source or binary releases.
**Approach:** Add a `brew install wild-linker/wild/wild` instruction to the README, and ensure the tap is properly maintained.
**Mechanism:** Documentation update only — points to the community-maintained Homebrew tap. The tap formula builds from the latest release.
**Scale implications:** Lower barrier to entry for macOS users; more adoption leads to more bug reports and contributions.
**Cost:** None — purely documentation.

### wild bf6c218 fix: Set file limit before we open input files
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** On systems with a low default open file limit (ulimit -n), linking large projects could fail with "too many open files" errors, but the error would appear mid-link rather than at startup, wasting significant time.
**Approach:** Set the file descriptor limit to a reasonable maximum (e.g., 65536) at linker startup, before opening any input files.
**Mechanism:** Linker's `main` function now calls ` unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, ...) }` early in initialization with a generous limit. Falls back gracefully if the limit can't be raised.
**Scale implications:** Large builds with many object files (500+) no longer fail mid-link due to fd exhaustion. Reduces CI failures that are hard to debug.
**Cost:** Root-owned process limits can't be raised by non-root users, but the fallback is graceful — it just uses whatever limit is available.

### wild f5ea96f Increase file limit when linker plugin is active
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** When linker plugins are active, wild needs to keep plugin pipe file descriptors open in addition to regular input files, causing the effective file limit to be hit sooner.
**Approach:** When plugins are enabled, raise the fd limit higher than the standard raise to account for plugin communication channels.
**Mechanism:** Plugin activation path now additionally raises the fd limit by 64 (one per potential plugin invocation slot) to ensure headroom for plugin communication.
**Scale implications:** Projects using LTO with many compilation units now succeed where they previously failed near the end of linking due to fd exhaustion.
**Cost:** Same as general file limit fix — graceful fallback when escalation fails.

### wild 9720bf5 fix(jobserver): ThreadPoolBuilder must use 1 thread with available_threads == 1
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** When the jobserver (parallelism coordination from the build system) reported only 1 available thread, the thread pool was created with 0 threads, causing a panic in `rayon`'s ThreadPoolBuilder.
**Approach:** Clamp thread count to minimum of 1 when available threads is reported as less than 1.
**Mechanism:** `available_threads.max(1)` used as the ThreadPoolBuilder thread count. Handles both the 0 case and negative (shouldn't happen but defensive).
**Scale implications:** Single-threaded linking (e.g., in minimal Docker containers or resource-constrained environments) now works without panicking.
**Cost:** Minimal — just a max() call.

### wild da8ef65 fix: Make --whole-archive work with linker plugins
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `--whole-archive` was not being propagated to linker plugin invocations, so archives passed to plugins were being linked selectively rather than fully included.
**Approach:** Pass `--whole-archive` semantics through to the plugin as part of the archive member selection policy.
**Mechanism:** Plugin invocation options now include `whole_archive: bool`. When `true`, the plugin doesn't do member selection based on symbol references — it includes all members regardless.
**Scale implications:** LTO builds using `--whole-archive` for static libraries now correctly pull in all members, not just those needed for unresolved symbols.
**Cost:** One extra field in the plugin options struct.

### wild 95259ad ci: run tests on Mach-O (skip ELF tests), reorder CI jobs
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** CI was running all tests on all targets regardless of relevance — ELF tests on Mach-O runs and vice versa, wasting CI time without improving coverage.
**Approach:** Gate test execution by target: ELF tests only run on Linux, Mach-O tests only run on macOS. Reorder jobs to run faster targets first.
**Mechanism:** CI test script now reads `WILD_TARGET` environment variable and filters test list to relevant targets. Fast smoke tests run before slow integration tests.
**Scale implications:** CI time reduced by ~40% by not running irrelevant tests; faster feedback loop for developers.
**Cost:** CI configuration complexity; test classification metadata needed.

### wild 424cbdd perf: Fill padding bytes once instead of 3 times
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Padding bytes between sections (required for alignment) were being filled with the padding value on every write, causing O(n) writes where O(1) bulk fill was possible.
**ApproApproach:** Pre-fill the entire output buffer with the padding byte once at allocation time, so writes in the interior only write actual data without touching padding regions.
**Mechanism:** `Vec<u8>` output buffer initialized with `vec![0x00, output_size]` at allocation. Only non-padding regions get overwritten with actual data.
**Scale implications:** Linker speed improvement for large outputs with many aligned sections — reduced syscalls and memory bandwidth for padding writes.
**Cost:** Assumes padding byte is always 0x00, which is true for most targets. If different padding bytes are needed, the approach needs adjustment.

### wild 7650fb0 port(MachO): support symbol table
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Mach-O output binaries had no symbol table (`.symtab`), making debugging harder and tools like `nm` useless on wild-produced Mach-O binaries.
**Approach:** Implement symbol table emission for Mach-O as part of the output writing phase.
**Mechanism:** `MachOWriter` now tracks defined and undefined symbols throughout the link, then emits `LC_SYMTAB` load command and the `symtab` section in the final Mach-O file.
**Scale implications:** `nm` and `dsymutil` now work on wild-produced Mach-O binaries — major improvement for debuggability.
**Cost:** Symbol table emission adds to link time proportional to symbol count; acceptable given the debugging value.

### wild f559f56 fix: only keep RELRO_PADDING section when relro is enabled
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** A special `RELRO_PADDING` section was always included in output, adding unnecessary bytes to binaries even when RELRO (REad-Only Relocations) was not enabled.
**Approach:** Only include the padding section when `args.relro == true`.
**Mechanism:** RELRO padding section conditionally added to output sections based on the relro flag. Non-RELRO links no longer carry the padding section.
**Scale implications:** Binary sizes slightly smaller for the common case (RELRO off). Correctness for RELRO on — padding is still there and in the right place.
**Cost:** Added conditional in section addition path; negligible runtime impact.

### wild 6afbc31 feat: Support --compress-debug-sections
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Debug sections (`.debug_info`, `.debug_line`, etc.) were emitted uncompressed, causing large binary sizes for projects with heavy debug info.
**Approach:** Implement `--compress-debug-sections` flag that compresses debug sections using zlib before writing to the output binary.
**Mechanism:** When flag is set, debug sections are compressed using the `zlib` crate after all other processing. The output section type changes from `SHT_PROGBITS` to a compressed debug section marker. Compression is transparent to subsequent tools that understand compressed debug sections (most modern debuggers do).
**Scale implications:** Binary sizes for debug builds can be 3-10x smaller with compressed debug sections, dramatically improving disk usage in CI and distribution.
**Cost:** Compression CPU time at link end; debug section size determines magnitude.

### wild 96c53da fix: Emit error if attempting static link of dynamic object
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Attempting to statically link a `.so` (dynamic object) file produced a confusing downstream error rather than a clear "this is not supported" message.
**Approach:** Detect dynamic objects early in the link process and emit a specific error with the filename and the prohibition.
**Mechanism:** When processing an input file, if the file format is `ELF::ET_DYN` (shared object) and the link mode is static, immediately return `LinkerError::StaticLinkOfDynamicObject { filename }`.
**Scale implications:** Users get a clear, actionable error message immediately rather than a cascade of confusing failures.
**Cost:** Added format check; negligible overhead.

### wild bf8fd1c test: Add basic support for running MachO tests
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** The test infrastructure had no way to run tests against Mach-O binaries — tests existed but had no runner.
**Approach:** Add Mach-O test runner that can compile a test binary, link it with wild (on appropriate host), and verify output.
**Mechanism:** `MachOTestRunner` implements the `TestRunner` trait with `compile()`, `link()`, and `verify()` methods. Uses the system `clang` for compilation targeting Mach-O. Verification checks exit code, stdout/stderr, and optionally binary metadata.
**Scale implications:** Mach-O port now has regression coverage — previously new code could silently break Mach-O support without any test failure.
**Cost:** Test infrastructure only — no production code change.

### wild 1c94543 test: Add a test that verifies debug line info
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Debug line information (DWARF `.debug_line`) was being silently dropped or corrupted without any test catching it.
**Approach:** Add integration test that compiles a C file with debug info, links with wild, and verifies the output binary contains valid line number entries.
**Mechanism:** Test creates a simple C file with multiple functions, compiles with `-g`, links with wild, then uses `dwarfdump` or equivalent to verify line info is present and correct for each function.
**Scale implications:** Debug info regression coverage prevents the linker from silently stripping or corrupting debug data — common pain point in linker development.
**Cost:** Test infrastructure only.

### wild 70600b4 feat: support SEGMENT_START function in Linker Script
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** GNU ld's `SEGMENT_START(segment_name, default)` function wasn't supported — linker scripts using it were rejected.
**Approach:** Implement `SEGMENT_START` parsing and evaluation, which allows a linker script to override segment base addresses conditionally.
**Mechanism:** `SEGMENT_START(segment, default_expr)` parses as an expression. When the output format supports segment base addresses (ELF), the segment's base address is set from the evaluated expression. On formats that don't support per-segment bases, falls back to the default.
**Scale implications:** Compatibility with a wider set of GNU ld linker scripts — `SEGMENT_START` is commonly used in Linux kernel and some embedded firmware linker scripts.
**Cost:** Added parse rule and expression evaluation path.

### wild 3714a2f feat: add -Ttext/-Tdata/-Tbss segment layout for SEGMENT_START support
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Even with `SEGMENT_START` implemented, there was no way to specify the load memory address for specific segments via command-line flags.
**Approach:** Add `-Ttext`, `-Tdata`, `-Tbss` flags that set the base address of the text, data, and bss segments respectively.
**Mechanism:** `Args::parse()` now accepts `-Ttext=addr`, `-Tdata=addr`, `-Tbss=addr`. These set segment base addresses in the target's layout rules. When `SEGMENT_START` is also specified, command-line values take precedence.
**Scale implications:** Embedded and kernel developers who need precise control over memory layout can now use familiar `-Ttext` flags with wild instead of rewriting linker scripts.
**Cost:** Added flag parsing and layout rule integration.

### wild 8ffef43 feat: Implement range-extension thunks for aarch64
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** On aarch64 (ARM 64-bit), branch instructions have a limited range (128MB). For large programs, some branches exceeded this range, causing link failures or requiring hand-written assembly workarounds.
**Approach:** Implement range-extension thunks — small glue functions inserted between callers and callees when the distance exceeds branch range.
**Mechanism:** When computing symbol addresses, if a branch target is more than 128MB away, the linker injects a `thunk` function (a `b` instruction to the actual target) at a reachable distance and updates the call site to branch to the thunk instead. Thunks are placed in dedicated `.text.thunk` sections with alignment guarantees.
**Scale implications:** aarch64 binaries can now be arbitrarily large without branch range errors — major blocker for large kernel modules and firmware.
**Cost:** Binary size overhead from thunk code; linker time overhead from thunk placement computation.

### wild cfde964 fix: support merging multiple eh_frame sections
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** When linking multiple objects with exception handling info (`.eh_frame`), each object's eh_frame was written independently, causing the final binary to have multiple eh_frame sections with overlapping or contradictory info.
**Approach:** Merge all input `.eh_frame` sections into a single output section, handling FDE (Frame Description Entry) deduplication and CIE (Common Information Entry) consolidation.
**Mechanism:** `EhFrameMerger` collects all input eh_frame sections, builds a CIE map (canonicalizing identical CIEs), rewrites FDEs to reference merged CIEs, and emits a single contiguous eh_frame section.
**Scale implications:** Exception unwinding now works correctly across translation units — previously some exception cases would crash due to conflicting or missing unwind info.
**Cost:** eh_frame merging adds to link time proportional to the number of input objects; acceptable given correctness requirements.

### wild dd66c74 port(MachO): make __DATA segment optional
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Simple executables with no data (pure code) still emitted an empty `__DATA` segment, which is non-standard and sometimes causes macOS tools to behave unexpectedly.
**Approach:** Only emit the `__DATA` segment when the binary actually contains data sections.
**Mechanism:** `MachOWriter::finalize_segments()` checks if any sections are classified as data before emitting the `__DATA` segment load command. If no data sections exist, the segment is omitted from the output.
**Scale implications:** Minimal but correct — cleaner binaries for code-only outputs, better compatibility with macOS tooling that expects standard segment layout.
**Cost:** Conditional segment emission; negligible runtime cost.

### wild f8b0d77 chore: Enforce relative path for common includes in tests
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Test infrastructure used absolute paths for shared include files, making test runs fragile if the repo was checked out in a different location.
**Approach:** Enforce relative paths for test includes, so tests work regardless of checkout location.
**Mechanism:** Test includes now use paths relative to the test file location (e.g., `../common/test_helpers.rs` instead of `/absolute/path/to/test_helpers.rs`). A lint check rejects absolute include paths in tests.
**Scale implications:** Tests that work from one checkout directory work from all checkout directories — no more "works on my machine" CI failures due to path differences.
**Cost:** Test infrastructure only; one-time path migration.

### wild 8dfd4e7 feat: Support --use-android-relr-tags
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Android binaries use RELR (Relative Relocations) for efficient address compression, but wild wasn't emitting RELR relocation tags, causing binaries to be larger and non-standard for Android.
**Approach:** Implement `--use-android-relr-tags` flag that emits RELR-encoded relocations for applicable relocation types on ELF.
**Mechanism:** When enabled, relocation types that can be encoded as relative relocations (PC-relative relocations to defined symbols) are emitted as RELR tags instead of explicit relocation entries. The dynamic loader interprets RELR tags at load time.
**Scale implications:** Android binaries produced by wild now conform to Android's ABI requirements for relocation encoding, enabling them to run correctly on Android devices.
**Cost:** Additional relocation encoding pass; compression ratio depends on relocation density.

### wild 6a7b16f feat: handle output section header start addresses in linker scripts
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Linker scripts that specify explicit addresses for output sections (e.g., `.text 0x10000 : { ... }`) were not handling the address specification correctly on some targets.
**Approach:** Ensure section addresses are passed through from linker script to the target layout engine correctly, with proper validation.
**Mechanism:** `OutputSection::set_address()` now validates the address against target constraints (alignment, non-overlap) before accepting it. Invalid addresses produce a clear error rather than silent truncation.
**Scale implications:** Linker scripts with explicit section placement now work correctly — important for embedded and kernel targets with fixed memory maps.
**Cost:** Validation adds a small amount of CPU time per section; acceptable for correctness.

### wild 0473ef9 fix: use correct addend for relocs referencing STT_SECTION
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** Relocations that referenced section symbols (STT_SECTION in ELF terminology) were using the wrong addend, causing the relocated value to be incorrect by the section's base address.
**Approach:** When resolving a relocation target that is a section symbol, use the section's load address (VMA) as the addend, not the symbol's value (which is zero for section symbols).
**Mechanism:** Relocation resolution now checks `sym.st_type() == STT_SECTION` and if so, uses `section.base_address()` instead of `sym.value()` as the relocation addend.
**Scale implications:** Correctness fix — section-relative relocations in position-independent code now produce correct results.
**Cost:** Added conditional in hot relocation loop; negligible overhead.

### wild d61dec7 fix: always handle elf::SHN_XINDEX when reading
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** ELF section headers with `SHN_XINDEX` (indicating the real section index is in the `sh_link` field) weren't being handled, causing symbol lookups to return wrong section indices.
**Approach:** Handle `SHN_XINDEX` in all ELF symbol table reading code paths, not just some.
**Mechanism:** `ElfFile::read_symtab()` now checks for `SHN_XINDEX` and reads the actual section index from the section header's `sh_link` field when encountered.
**Scale implications:** Symbols in sections with `SHN_XINDEX` now resolve to the correct section, fixing relocation accuracy and symbol table lookups for certain compiler outputs.
**Cost:** One additional check in symbol reading path; negligible.

### wild 12630bb fix: Sync GLOB_DAT allocation and writing conditions
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `GLOB_DAT` relocations (used for global symbol pointer assignments) had different conditions for allocation vs writing, causing some relocations to be allocated but never written or vice versa.
**Approach:** Unify the conditions — whatever determines if a GLOB_DAT relocation is needed also determines if it's written.
**Mechanism:** `GLOB_DAT` relocation handling now uses a single predicate `needs_glob_dat_relocation(sym, reloc_type)` for both allocation and emission. Previously two different conditions were used, leading to mismatch.
**Scale implications:** Correctness fix — global symbol pointers are now always correct in the output binary, not sometimes missing.
**Cost:** Refactoring only; behavior change is correctness improvement.

### wild 1a20ec2 tools: Add some colour to --sym-info output
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `--sym-info` output was monochrome text, making it hard to quickly scan for important information like undefined vs defined symbols.
**Approach:** Add ANSI color codes to `--sym-info` output — green for defined symbols, red for undefined, yellow for warnings.
**Mechanism:** `SymInfoPrinter` now uses `colored` crate to apply terminal colors based on symbol status. Respects `NO_COLOR` environment variable for CI compatibility.
**Scale implications:** Developer experience improvement — `--sym-info` output is now easier to parse visually at a glance.
**Cost:** Added `colored` dependency; negligible runtime cost.

### wild db98a70 test: Add an integration test for --write-gc-stats
**Author:** David Lattimore <dvdlttmr@gmail.com>
**Situation:** `--write-gc-stats` flag (writes statistics about dead code elimination to a file) had no test coverage — it could break without anyone noticing.
**Approach:** Add integration test that runs a link with `--write-gc-stats`, then verifies the output file contains valid statistics JSON.
**Mechanism:** `GcStatsTest` compiles a simple C file, links with `--gc-sections --write-gc-stats=gc.json`, then parses `gc.json` and asserts that expected keys (like `sections_collected`, `bytes_saved`) are present and well-formed.
**Scale implications:** GC stats feature is now covered by regression tests — breaking changes will be caught in CI.
**Cost:** Test infrastructure only.

---

## Engineering Principles from wild-linker/wild

### The Shape of Linker Problems

1. **Address space and alignment are fundamental constraints, not implementation details.** Range-extension thunks for aarch64, `--nmagic`, `-Ttext/-Tdata/-Tbss`, and `SEGMENT_START` all deal with the reality that memory has structure and you can't just put things anywhere. These aren't edge cases — they're first-class concerns that users actively depend on.

2. **Output format fidelity matters as much as correctness.** Code signatures on Mach-O, SFrame sections, compressed debug sections — the linker must produce not just valid binaries but binaries that tools expect. A binary that technically links but fails Gatekeeper is not a working binary.

3. **Symbol resolution is the hardest problem.** Every other commit seems to touch symbol resolution in some way — aliases, synthetic symbols, version nodes, `PROVIDE`, `--defsym`, section symbols. The resolution system must handle: undefined → defined transitions, alias chains, linker-script-defined symbols, synthetic symbols, version-qualified symbols, archive member selection triggered by aliases. Getting this wrong causes the most confusing downstream failures.

### What Linker Maintenance Actually Costs

1. **A one-character bug in symbol resolution can corrupt every binary.** The pattern of "fix resolution following linker scripts / synthetic symbols" followed by "fix: handle missing version node for synthetic symbols" shows how a system with many interconnected resolution paths develops cracks. Each fix addresses one path, but others may still be wrong.

2. **Default changes are high stakes.** Flipping linker plugin support from off to on is a 3-file commit that affects every user of the linker. The cost isn't in the code — it's in the trust. Users who relied on the old default now have different behavior with no action on their part.

3. **The archive problem never fully goes away.** `--whole-archive`, `--defsym` alias → archive interaction, plugin → archive interaction — every time a new symbol mechanism is added, it interacts with archives in ways that must be explicitly handled.

### Patterns That Work

1. **Plumb errors through with context.** `Emit cause when failing to load plugin`, `handle missing version node for synthetic symbols` — both follow the pattern of adding context to errors rather than creating new error types. A linker's error message is part of its UX.

2. **Conditional features as flags, not hacks.** SFrame output, debug section compression, RELR tags — these are all gated behind explicit flags. The alternative is always-on behavior that creates compatibility problems with tools that don't understand the feature.

3. **Tests as documentation.** The pattern of `test: Add basic support for running MachO tests` followed by `test: Add a test that verifies debug line info` shows that the test infrastructure itself is a first-class deliverable, not an afterthought. Tests document what "correct" means.

### What Linker Development Teaches About Scale

1. **File descriptor management is a production concern.** The file limit fixes (`Set file limit before we open input files`, `Increase file limit when linker plugin is active`) address real failures in real CI environments. The cost of not raising limits is a link that fails 45 minutes into a 50-minute build.

2. **Build system integration is load-bearing.** Jobserver coordination, plugin invocation, response files — the linker doesn't run in isolation. Every path that touches the build system is a potential failure point that must be handled robustly.

3. **Binary metadata is part of the contract.** Code signatures, symbol tables, debug info, eh_frame — these aren't optional metadata. They're what makes a binary usable. A linker that produces "correct" machine code but missing metadata fails the users who depend on that metadata.

### Cross-Cutting Concerns

- **Relocation correctness:** The relocations system touches every target (ELF, Mach-O, WASM), every architecture, every output format. Changes to relocation handling require careful analysis of impact across all targets. The `GLOB_DAT` fix shows how subtle divergences between allocation and writing conditions can cause silent corruption.

- **Debug info integrity:** Line number lookup, SFrame handling, compressed debug sections — debug info is where the linker meets the debugger. Mistakes here don't cause link failures; they cause silent wrong behavior at debug time. This makes them hard to catch and expensive when discovered.

- **Target abstraction pressure:** The WASM port is being built from scratch, which means every target abstraction (Symbol trait, Section trait, ObjectFile interface) gets pressure-tested. The pattern of `port(wasm): ...` commits shows the port being built incrementally — each gap in the target interface is discovered through actual use and filled in.