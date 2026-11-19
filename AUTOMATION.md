# Autonomous Development Loop

How jpdf gets built with **no human oversight required**. A human *may* read the
weekly digest or pull the kill switch, but nothing waits on a human, except a few
one-time setup steps (§9) and changes to protected files (§5), which can never be on the critical path.

Design principle: **the agent can't be trusted to grade its own work, so machines grade
it.** Automation is safe only when every quality bar is checked mechanically and
can only get stricter, never looser.

---

## 1. Components

```
┌──── Orchestrator: local Claude Code `/loop` on the Mac ───┐
│ thin; reads STATUS + GitHub, decides the next tick,      │
│ spawns agents, merges, sleeps ~2h (≈12 ticks/day)        │
└──────────────┬────────────────────────────────────────────┘
               ▼
┌──── Worker subagent (fresh context, own git worktree) ────┐
│ orient → pick task → branch → spec/test/code →            │
│ local gates (incl. Xcode) → PR → returns summary          │
└──────────────┬────────────────────────────────────────────┘
               ▼
┌──── Reviewer subagent (fresh context, sees only diff) ────┐
│ adversarial review → posts `ai-review` status             │
└──────┬───────────────────────────┬────────────────────────┘
       ▼                           ▼
┌──── GitHub ─────────────┐  ┌──── GitHub Actions CI ────────────────┐
│ Issues = task queue     │  │ PR gates (Linux + macOS runners)      │
│ PRs = units of change   │  │ Nightly: regress corpus, long fuzz,   │
│ Rulesets = guardrails   │  │   bench, oracle diff → files issues   │
│ CODEOWNERS = protected  │  │ Weekly: wide corpus → files issues    │
└─────────────────────────┘  │ Ratchet: baselines only get stricter  │
                             └───────────────────────────────────────┘
```

- **State lives in the repo and GitHub, never in an agent's memory.** Every tick
  starts cold and reconstructs context from §2. That way a crashed or killed tick
  loses at most one in-progress branch.
- **Ticks are short and bounded:** one task, or one PR fix, per tick. Many small
  ticks recover from failure better than one long session.

## 2. Durable state

| Where | What |
|---|---|
| `PLAN.md` | scope + milestones (living; agent may edit except the non-negotiables) |
| `CONSTITUTION.md` | the non-negotiable rules (protected, §5) |
| `ops/STATUS.md` | current milestone, what's in flight, last tick summary, next intended step (≤ 1 page, rewritten each tick) |
| `ops/journal/YYYY-MM-DD.md` | append-only log of each tick: what was tried, result, links |
| `docs/adr/NNNN-*.md` | Architecture Decision Records: every non-trivial decision + why |
| `specs/**` | feature specs (behavior, ISO clauses, acceptance tests) |
| GitHub Issues | task queue. Labels: `m0..m10`, `ready`, `in-progress`, `blocked`, `bug`, `crash`, `perf`, `discovery`, `spec-dispute`, `needs-human` |
| `ops/ratchet/*.json` | metric baselines (render pass %, corpus open %, bench times, fuzz coverage). **Written only by CI on `main`** |

## 3. The tick protocol (what the worker does)

```
0. HALT check: if ops/HALT exists or issue labeled `halt` is open → exit.
1. ORIENT: read STATUS.md, today's journal, CI state of main, open PRs, new nightly issues.
2. PICK (strict priority):
   a. main is red                        → fix it (nothing else matters)
   b. own PR failing CI / review          → fix it (max 3 attempts, then §6)
   c. new `crash` / security issues       → fix (fuzz/corpus findings)
   d. ratchet regression issues           → fix
   e. `ready` task in current milestone   → lowest number, dependencies met
   f. backlog empty for milestone         → run gate check (§7) or PLAN next tasks
   g. every 10th merged PR                → AUDIT tick instead (§8)
3. CLAIM: label issue `in-progress`, assign; branch `t/<issue>-<slug>`.
4. WORK: spec (if missing) → failing tests → implementation → local gates.
5. PR: description links issue, spec, ADRs; CI runs.
6. REVIEW: fresh reviewer subagent (no implementation context) gets
   diff + spec + CONSTITUTION, reviews adversarially for correctness/security/
   rule violations → posts commit status `ai-review` (pass/fail + comments).
7. MERGE: auto-merge when CI + `ai-review` green. Otherwise loop back (b).
8. RECORD: journal entry, update STATUS.md, file issues for discoveries (§4).
9. EXIT. Scheduler starts next tick.
```

## 4. Handling discoveries

Anything learned mid-task that changes the plan is filed and then handled with **fixed
rules**, so it never needs a human:

| Discovery type | Action |
|---|---|
| Extra work needed | new issue `discovery` + milestone label; current task continues or is split |
| Spec ambiguity | decide with this rule order: ① ISO 32000-2 text → ② errata / PDF Association clarifications → ③ consensus of reference engines (oracle) → ④ the **safer** behavior. Record in an ADR. |
| Reference engines disagree | `spec-dispute` issue; resolved with the same rule order; the page is moved to the disputed list with a note |
| Design flaw found | ADR proposing the change + refactor issue(s); priority above new features |
| Backend crate inadequate (bugs, unsafe, license) | ADR; swap implementation behind trait or write our own; trait conformance suite must pass |
| Milestone order wrong | edit PLAN.md + ADR (allowed; non-negotiables are not) |
| Wants a new dependency | automated admission (§5.3); if it passes, no human needed |
| Would need to break a non-negotiable | not allowed. Find an alternative; if none, label `needs-human` and **move on to other work** |

## 5. Guardrails (so automation can't game itself)

### 5.1 Protected paths: CODEOWNERS + ruleset require human review
`CONSTITUTION.md`, `.github/**`, `ops/ratchet/**`, `deny.toml`, `supply-chain/**`
(cargo-vet), `.claude/settings.json`, `.claude/hooks/**`, the `Limits` defaults file.
PRs touching these **never auto-merge**. They are labeled `needs-human`, and the agent
moves on. Normal work never needs to touch them.

### 5.2 Ratchets: metrics only go up
CI on `main` updates `ops/ratchet/*.json` after nightly runs. PR CI compares against them:
- render-consensus pass %, corpus open %, text-extraction score: **may not decrease**
- panics / timeouts / limit escapes on corpus tiers: **must stay 0**
- bench: ≤ 5% regression per metric
- test count: may not decrease; `#[ignore]` / `skip` requires a linked open issue
- fuzz coverage (edges): may not decrease by more than noise threshold
- `unsafe` count (geiger): must stay 0 in first-party crates

### 5.3 Automated dependency admission
CI job `dep-admission` runs on any `Cargo.lock` change: license allowlist,
`cargo geiger` (no new unsafe outside allowlist), `cargo vet` (an imported audit is required),
crate age/maintenance heuristics, and our trait conformance + fuzz suite. If any check
fails, the PR fails, and the agent picks a different crate or writes its own.

### 5.4 Session-level hooks (`.claude/hooks`)
- `PreToolUse` on Edit/Write: block edits to protected paths; block adding `unsafe`,
  `#[allow(unsafe_code)]`, removing `forbid(unsafe_code)`, or `#[ignore]` without an issue link.
- `PreToolUse` on Bash: block `git push --force`, pushes to `main`, `gh pr merge --admin`,
  deleting branches other than the agent's own, and network calls outside an allowlist.
- These are a first layer only. The real enforcement is CI + rulesets (§5.1–5.2), which
  the agent cannot modify.

### 5.5 Independent review
The reviewer is a separate agent with no implementation context. Its checklist:
CONSTITUTION violations, missing spec references, missing tests for new behavior,
input that could panic, unchecked arithmetic on input-derived values, limits bypass,
logic added to the native layer, and code that looks copied from a known engine.

## 6. Stuck handling

- **3 failed attempts** on the same task or PR → stop. Write the hypotheses and evidence to
  the issue, label `blocked`, and pick other work.
- **Blocked issues are revisited** in audit ticks with fresh context. The usual moves are to
  split the task, change approach via an ADR, or write a minimal repro.
- **If main has been red for 3 ticks** → revert the offending merge (`git revert`, via PR) and
  reopen its issue.
- **Thrash detector:** if the same file churns in more than 5 PRs per day, or the same test
  flips back and forth, the next tick is an audit tick focused on it.

## 7. Milestone gates (automated)

`tools/gate <milestone>` mechanically evaluates the exit criteria in PLAN.md §8 on
nightly data. When it passes, the agent:
1. tags `mN`
2. writes `docs/milestones/mN.md` (what was built, metrics, known limits, ADRs)
3. generates the next milestone's issue backlog from PLAN.md + specs
4. posts the digest (§9)

## 8. Audit ticks (self-correction against drift)

Every 10 merged PRs, or weekly, a tick does no feature work. Instead it:
- runs a security review and a code review of everything merged since the last audit, with fresh agents
- reconciles PLAN.md / STATUS.md / issues against reality (stale issues, unplanned work)
- revisits `blocked` issues
- checks for duplicated code across crates and dead code
- refreshes backend crate versions through dep-admission

## 9. Human touchpoints

**Required, one time only:**
1. Create a **public GitHub repo** (public = free macOS CI runners) and push.
2. Enable rulesets + CODEOWNERS + auto-merge (the agent can prepare the config).
3. Start the local loop (§10) and keep the Mac powered (it is allowed to lock).
4. Later, for release only: Apple Developer signing/notarization certificates and a web hosting account.

**Optional:**
- **Weekly digest:** progress, metrics trends, ADRs made, `needs-human` items, cost.
- **Kill switch:** create `ops/HALT` or open an issue labeled `halt`.
- **Reviewing `needs-human` PRs:** never blocks progress. They just stay open.

## 10. Execution: local loop on the Mac (chosen: option C)

**Setup (once):**
```sh
brew install tmux qpdf poppler mupdf-tools   # + pdfium binaries, fetched by tools/oracle
tmux new -s jpdf
caffeinate -dimsu &                          # keep the Mac awake while the loop runs
claude                                       # in the repo, auto mode on
> /loop Run one tick per AUTOMATION.md §3
```
`/loop` with no interval paces itself, with about 2 h between ticks (≈12/day). Each wake
spawns a fresh worker subagent in its own git worktree, so the orchestrator's context
stays small.

**Why the orchestrator/worker split:** the `/loop` session lives for days. If it did the
work itself, its context would fill with code, and a compaction could lose details. Instead
the workers start fresh each tick, and the orchestrator holds only small summaries.

**Resilience:**
- **Mac restart, terminal closed, or crash:** rerun the `/loop` command. All state is in
  the repo and GitHub, and an orphaned branch is either resumed or discarded by the next tick.
- **Optional:** a `launchd` agent that relaunches the tmux session at login (set up in M0).
- **Unattended permissions:** the loop needs auto mode or a permission allowlist so it
  never stops at a prompt. `.claude/settings.json` (protected) holds the allowlist and
  the guard hooks from §5.4.

**What runs where:**
| Work | Local Mac | GitHub Actions |
|---|---|---|
| Worker ticks, Xcode builds, XCUITest, Playwright | ✅ | — |
| PR gates (authoritative, so the agent can't fake them) | quick pre-check | ✅ Linux + macOS runners |
| Nightly: `regress` corpus, long fuzz, bench, oracle | — | ✅ (with an optional local mirror) |
| Weekly: `wide` corpus (too big for CI) | ✅ via a launchd job at night; results uploaded as issues | — |
| Ratchet baseline updates | — | ✅ only CI on `main` writes them |

**Bench note:** benchmark ratchets use the GitHub runner's numbers (stable hardware
class). Local timings are informational only.

**Concurrency:** start with 1 worker. Once M1 is stable, the orchestrator may run 2–3
workers in parallel worktrees on distinct crate areas, each holding its issue's
`in-progress` claim as a lock.

**Disk:** this Mac has ~196 GB free, so `wide` uses **rotating shards**. Each weekly run
downloads a new ~50 GB shard (~50–100k PDFs) of SafeDocs/GovDocs1 into `~/jpdf-corpus`,
tests it, keeps only the PDFs that failed (minimized), then deletes the shard. Over
months this covers millions of files within a fixed disk budget (cap: 80 GB, configurable).
