---
name: oss-insights
description: Use when the user asks to analyze, study, or extract learnings from a set of public OSS repositories — commit by commit, pattern by pattern. Capture style, approach, ordering, failure, insights. Read commit messages to understand the _why_. Produce consolidated engineering principles. Works recursively: re-runs pick up from last processed commit.
---

# OSS Insights

Analyze public OSS repositories commit-by-commit and extract actionable engineering principles.

## Core Loop

```text
For each assigned repo:
  1. Clone or pull latest
  2. Get last N commits (--depth flag or --since)
  3. For each meaningful commit:
     a. git log --oneline {sha}
     b. git show --stat {sha}
     c. git log -1 --format="%H%n%an%n%ae%n%s%n%b" {sha}
     d. git show {sha} --pretty=format:"%H" --name-only
  4. Extract structured insight per commit
  5. Track last-processed SHA in .oss-insights/
  6. Append to insights file
```

## Insight Format (per commit)

```markdown
### [{repo}] {sha_short} {commit_title}
**Author:** {author} | **Date:** {date}
**Files:** {file list}
**Type:** [refactor|feature|fix|perf|test|docs|infra|architecture|breaking|experimental]
**What:** {1-2 sentence summary of the concrete change}
**Why:** {inferred reason — what problem does it solve? what motivated it?}
**Insights:**
- {structural pattern observed}
- {approach choice made}
- {failure/recovery pattern if any}
- {scale concern if any}
```

## Categories to Look For

- **Refactor patterns**: How do they restructure code? What triggers it?
- **API evolution**: How do they add/change/deprecate APIs?
- **Test strategy**: How are tests structured? What kinds are added?
- **Breaking changes**: How are they communicated? Handled?
- **Architecture**: Big shifts? What preceded them?
- **Performance**: How are perf improvements approached?
- **Failure patterns**: What breaks at scale? How do they recover?
- **Commit hygiene**: How are messages structured? What discipline?
- **Dependency management**: How are deps handled?
- **Contributor patterns**: Who makes what kinds of changes?
- **Build systems**: Special considerations for the build tool
- **Rust idioms**: If Rust project — specific patterns, trait design, error handling

## Output Structure

```
oss-insights/
  {org}/
    {repo}-insights.md    # per-repo insights from this run
  all-principles.md       # synthesized cross-repo principles
```

## Progress Tracking

Store last-processed SHA per repo:
```
.oss-insights/
  {repo}/last-processed  # contains the last SHA
```

On re-run: skip commits already processed (compare SHA to stored last-processed).

## Runtime Caps

- `--max-commits <n>`: stop after N commits per repo
- `--max-tokens <n>`: abort when token budget exhausted
- `--since <date>`: only commits after date

## Synthesis

After all repos processed, write `all-principles.md`:

```markdown
# Engineering Principles — {date}

## Core Lessons
## Scale Challenges
## Refactor Patterns
## Failure Handling
## Architecture Decisions
## Commit Discipline
## Cross-cutting Patterns
```

Be analytical — tell the reader WHAT they can learn and WHY this approach was taken.