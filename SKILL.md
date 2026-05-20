---
name: oss-insights
description: Use when the user asks to analyze, study, or extract learnings from a set of public OSS repositories — commit by commit, pattern by pattern. Capture style, approach, ordering, failure, insights. Read commit messages to understand the _why_. Produce consolidated engineering principles. Works recursively: re-runs pick up from last processed commit.
---

# OSS Insights

Analyze public OSS repositories commit-by-commit. Extract not just *what* changed — understand the substance: what problem was being solved, what situation existed, how it was addressed, and what it tells you about building at scale.

The goal is to build an intuitive model of how high-quality, long-running projects actually work: what eventually comes to bear, what breaks at scale, what the day-to-day texture of maintaining something with many users looks like.

## Mindset

Before reading any code, read the commit log. The commit log is a history of decisions made under pressure. Ask for each commit:

- What was the situation before this change?
- What was the concrete failure or limitation?
- Why was this approach chosen over alternatives?
- What would a naive solution have looked like? Why didn't that work?
- What did this cost? (complexity, binary size, API surface, cognitive load)
- What does this teach about the shape of the problem?

## Per-Commit Reading (not just scanning)

```bash
# For each meaningful commit:
cd /tmp/target-repo
git log --oneline --since="2025-01-01" | head -N  # get recent commits

# Then for each commit:
git show {sha} --stat                              # what files, how much changed
git log -1 --format="%H%n%an%n%ae%n%s%n%b" {sha} # full message with body
git show {sha} --name-only                         # exact file list
git show {sha} -- pretty=format:"" -U3            # diff context (first 3 lines of each hunk)
```

Skip: version bumps, dependency hash updates, automated bot commits, trivial typo fixes.
Deep-read: performance commits, architectural changes, bugfixes with root cause in the message, multi-commit sequences.

## Substance Format (per commit)

```markdown
### {repo} {sha_short} {title}
**Author:** {author}
**Situation:** {what was broken or insufficient before this change}
**Approach:** {how the problem was diagnosed, what design was chosen, why not alternatives}
**Mechanism:** {what the code actually does — be specific, not vague}
**Scale implications:** {what this tells you about the project at scale — maintenance cost, failure modes, architectural pressure}
**Cost:** {what was paid — binary size, complexity, API surface, regression risk}
```

## What to Look For

### Situation → Response patterns
- What types of problems trigger changes? (perf regression? user bug report? ecosystem pressure?)
- What's the ratio of defensive code to new feature code?
- How do they handle things that are "good enough for now but will be a problem later"?

### Mechanisms of addressing problems
- Do they refactor in-place or layer new code on top?
- How do they handle cross-cutting concerns that touch many layers?
- What's the pattern when a fix in one place requires fixing another?

### What breaks at scale
- Memory patterns: where do blowups happen? (inference depth? cache size? concurrent writes?)
- API surface: what kinds of things become untenable as user count grows?
- Tooling: what part of the dev experience degrades first?

### DEVX and build
- How long does a clean build take? What do they optimize first?
- How is CI structured? What's the feedback loop from "I pushed" to "I know if it broke"?
- How do they handle backwards compatibility in APIs and build flags?
- What's the rollback story when a change goes wrong?

### Performance
- What triggers a perf investigation? (profiling data? user complaint? internal target?)
- How do they measure before and after? (synthetic benchmarks? real workloads? both?)
- Is perf work incremental micro-optimization or structural changes?

### Failure recovery
- How do they handle crashes in production? (crash guards like sigsetjmp? monitoring? both?)
- What's the pattern for "we can't let this crash kill the whole process"?
- How are flaky tests handled? (disabled? fixed? tracked?)

### Architecture under pressure
- What does the code look like when the original design is stressed?
- Where do they add abstraction layers? Where do they refuse to?
- How do they manage the tension between "clean code" and "we need to ship"?

### Long-run project dynamics
- Who owns what? Is there a bus factor risk?
- How do they manage feature flags that were supposed to be temporary?
- What's the pattern for removing something that many users depend on?

## Progress Tracking

```
.oss-insights/
  {repo}/last-processed  # SHA of last analyzed commit
```

On re-run: compare SHA to last-processed and skip already-analyzed commits.

## Output

```
oss-insights/
  {org}/
    {repo}-insights.md    # substance per commit, not just description
  all-principles.md       # synthesized principles about scale, devx, perf
```

## Synthesis: All-Principles.md

After all repos analyzed, write `all-principles.md` that distills the *substance* of what these repos teach:

```markdown
# What High-Scale, Long-Running Projects Actually Teach

## The Situations That Actually Arise
## The Mechanisms That Actually Work at Scale
## What DevX Eventually Costs
## What Performance Work Looks Like in Practice
## What Failure Looks Like and How It's Contained
## Architectural Pressure Points
## DEVX and Build System Realities
## Cross-Cutting Concerns
```

Be direct: tell the reader what they can *do* differently, not just what happened.