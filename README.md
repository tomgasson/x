# oss-insights

Recursive OSS repository archaeology — commit by commit, pattern by pattern.

## What This Is

A skill for analyzing public OSS repositories at the commit level. It extracts not just *what* changed, but *why* — the structural patterns, engineering decisions, failure modes, and lessons that can only be seen by reading commits in sequence.

## The Skill

`SKILL.md` — tells the agent how to run the analysis loop.

## What We Found

`PRINCIPLES.md` — 15 core engineering principles distilled from 9 Meta/frontend OSS repos:

| Principle | Key Takeaway |
|---|---|
| Numbered refactors | Large refactors are tracked, bounded, self-contained |
| Hard flag removal | Flags have timelines; dead flags are technical debt |
| Micro perf | One specific fix, one commit, one measurement cited |
| Instrumented architecture | Big changes land as staged experiments, not big bangs |
| Reverts are first-class | Failure is normal; revert discipline is a competitive advantage |
| Co-located tests | Test names describe features, not frameworks |
| AI disclosure | Human review chain is explicit, not hidden |
| Rust idioms | Pre-allocation, ArcStr, node-type restriction, Once guards |
| Cross-layer changes | Architecture first, then implementation in dependency order |
| Contributor concentration | >70% in one person = bus factor risk, not celebration |
| Bot-batched deps | Automated, batched, separated from human-authored commits |
| Commit hygiene | Message = contract; future bisectors depend on it |
| Incremental scale | Fix the specific O(n²), not the hardware |
| Isolated rewrites | Keep experimental components separate until proven |
| Breaking changes | 2-release minimum: flag, then remove |

## Source Repos Analyzed

- facebook/hhvm — C++/Hack/Rust VM (400 commits)
- facebook/hermes — JS engine (400 commits)
- facebook/flow — TypeScript type checker (400 commits)
- facebook/relay — GraphQL client (400 commits)
- facebook/pyrefly — Python type checker in Rust (400 commits)
- facebook/sapling — Git-compatible VCS on Mercurial (400 commits)
- facebook/buck2 — Build system with Starlark (400 commits)
- oxc-project/oxc — Rust JS tooling suite, 30+ crates (500 commits)
- rolldown/rolldown — Rust bundler (500 commits)
- wild-linker/wild — WASM linker (500 commits)

## Re-Running This Skill

```bash
# On a new repo or fresh analysis
git clone https://github.com/<target>.git /tmp/target-repo
cd /tmp/target-repo
git log --oneline --max-count=500 | head -500

# Per commit:
git show --stat <sha>
git log -1 --format="%H%n%an%n%ae%n%s%n%b" <sha>
git show <sha> --pretty=format:"%H" --name-only

# Store progress
echo <last-sha> > .oss-insights/<repo>/last-processed
```

## Extending

Add new repos to the analysis by cloning, then iterating commits with the format in `SKILL.md`. Update `PRINCIPLES.md` with new patterns discovered.

## GHHF Integration

This skill pairs with GNHF (Good Night, Have Fun) — use `oss-insights` to study how others evolve their codebases, then use GNHF to apply those patterns to your own.
