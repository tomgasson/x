# Relay — Group A: Build / Infra / Deploy / TypeScript

## facebook/relay 92405070 — Support named exports with nodejs ESM
**Author:** Rob Richard <rob@1stdibs.com>
**Situation:** Named imports like `import { fetchQuery } from 'relay-runtime'` threw `SyntaxError: Named export 'fetchQuery' not found` in Node.js ESM files. Node.js's `cjs-module-lexer` couldn't detect named exports from Relay's CJS entry points.
**Approach:** Changed the export pattern from `module.exports = { fetchQuery: RelayRuntime.fetchQuery }` (which `cjs-module-lexer` doesn't detect) to `const { fetchQuery } = RelayRuntime; module.exports = { fetchQuery }` (which it does).
**Mechanism:** Entry point refactor in react-relay and relay-runtime. Test added to the Gulp build pipeline since the issue only manifests in the final transpiled output.
**Scale implications:** Affects every Relay user on Node.js ESM — a common pattern.
**Cost:** Backwards-compatible — just fixes a broken use case without changing API.

---

## facebook/relay 548434a5 — Include docs in NPM package for LLM/agent access
**Author:** Jordan Eldredge <jeldredge@meta.com>
**Situation:** LLMs and agents working in Relay codebases couldn't access documentation without network access.
**Approach:** Added `copyDocs` gulp task that copies `website/docs/**/*.mdx` into `dist/relay-runtime/llm-docs/` during build. Each entrypoint file gets a docblock pointing agents to `node_modules/relay-runtime/llm-docs/`.
**Mechanism:** 129 doc files now ship in the NPM package. Excludes `FbFakeContent.mdx` (internal placeholder) and versioned docs.
**Scale implications:** Makes Relay more agent-friendly — self-hosted models and air-gapped environments now get full docs from the installed package.
**Cost:** Small build overhead, negligible package size increase.

---

## facebook/relay 2ae7e5a9 + c0fe61a5 + b1b31af9 — Compiler output check CI fixes
**Author:** Jordan Eldredge <jeldredge@meta.com>
**Situation:** The "Compiler output check" CI job validates that generated files match what the compiler produces. When feature flags changed compiler output (e.g., `emit_nogrep_annotation` default, casting syntax change), generated test-project files weren't regenerated, causing CI to fail.
**Approach:** Regenerate the test-project generated files to match current compiler output. Three separate commits for three separate flag/feature changes.
**Mechanism:** Re-run compiler on test-project, commit the new generated files. CI job uses `check-git-status.sh` to detect any diff.
**Scale implications:** Every compiler flag change now requires updating generated test files — this is a recurring tax on feature flag changes.
**Cost:** Mechanical updates — easy to review, low risk.

---

## facebook/relay 3cb70f33 — Fix Windows file URI construction in relay-lsp
**Author:** (not attributed in commit)
**Situation:** File URIs on Windows need the `file:///` prefix with correct authority handling. Standard URI construction produced wrong paths on Windows, breaking LSP file references.
**Approach:** Fixed URI construction to use the correct Windows file URI format.
**Scale implications:** Only Windows users affected — typically invisible to macOS/Linux devs.
**Cost:** Small, targeted fix.

---

## facebook/relay d8bdb2d5 — Don't generate GraphQL schemas in daemon
**Author:** Evan Yeung <evanyeung@meta.com>
**Situation:** GraphQL schema generation was running inside the daemon process, coupling the compiler to schema generation lifecycle and causing issues during daemon restarts.
**Approach:** Moved schema generation outside the daemon — daemon restarts no longer trigger schema regeneration.
**Scale implications:** Reduces daemon coupling and restart flakiness.
**Cost:** Architectural change to daemon lifecycle.

---

## facebook/relay 86d27928 + 49f143b9 — Add TypeScript type defs to react-relay + relay-runtime
**Author:** (not attributed in commits)
**Situation:** TypeScript users were missing type declarations for some React hooks and runtime APIs.
**Approach:** Added missing `.d.ts` files for the affected packages.
**Scale implications:** TypeScript users get full type coverage without resorting to `as any`.
**Cost:** Pure typing work, low risk.

---

## facebook/relay ca723c82 — Add markdown-driven e2e test suite
**Author:** (not attributed in commit)
**Situation:** Relay needed an e2e testing approach that was easy to write and maintain, accessible to contributors who aren't compiler experts.
**Approach:** Markdown-driven e2e test suite — tests written in markdown, executed by the test harness.
**Scale implications:** Lowers the barrier for integration testing. More tests = fewer regressions reaching users.
**Cost:** Infrastructure investment upfront.

---

## facebook/relay ae1eb962 — Add missing TS type declarations for RelayFeatureFlags and Store log option
**Author:** Alison Lee <alis0n@meta.com>
**Situation:** Flow types existed (`RelayFeatureFlags.js`, `RelayModernStore.js`) but the TypeScript declaration files had gaps.
**Approach:** Added missing entries to `.d.ts` files: `ENABLE_READER_FLAGS_LOGGING` flag and `log? : LogFunction` on Store constructor.
**Scale implications:** TypeScript users get complete type coverage for these APIs.
**Cost:** Low-risk typing addition.
